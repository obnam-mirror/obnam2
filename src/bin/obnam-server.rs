use anyhow::Context;
use clap::Parser;
use log::{debug, error, info};
use obnam::chunkid::ChunkId;
use obnam::chunkmeta::ChunkMeta;
use obnam::chunkstore::ChunkStore;
use obnam::label::Label;
use obnam::server::{ServerConfig, ServerConfigError};
use serde::Serialize;
use std::collections::HashMap;
use std::default::Default;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::http::StatusCode;
use warp::hyper::body::Bytes;
use warp::Filter;

#[derive(Debug, Parser)]
#[clap(name = "obnam2-server", about = "Backup server")]
struct Opt {
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init_custom_env("OBNAM_SERVER_LOG");

    let opt = Opt::parse();
    let config = load_config(&opt.config)?;

    let addresses: Vec<SocketAddr> = config.address.to_socket_addrs()?.collect();
    if addresses.is_empty() {
        error!("specified address is empty set: {:?}", addresses);
        eprintln!("ERROR: server address is empty: {:?}", addresses);
        return Err(ServerConfigError::BadServerAddress.into());
    }

    let store = ChunkStore::local(&config.chunks)?;
    let store = Arc::new(Mutex::new(store));
    let store = warp::any().map(move || Arc::clone(&store));

    info!("Obnam server starting up");
    debug!("opt: {:#?}", opt);
    debug!("Configuration: {:#?}", config);

    let create = warp::post()
        .and(warp::path("v1"))
        .and(warp::path("chunks"))
        .and(warp::path::end())
        .and(store.clone())
        .and(warp::header("chunk-meta"))
        .and(warp::filters::body::bytes())
        .and_then(create_chunk);

    let fetch = warp::get()
        .and(warp::path("v1"))
        .and(warp::path("chunks"))
        .and(warp::path::param())
        .and(warp::path::end())
        .and(store.clone())
        .and_then(fetch_chunk);

    let search = warp::get()
        .and(warp::path("v1"))
        .and(warp::path("chunks"))
        .and(warp::path::end())
        .and(warp::query::<HashMap<String, String>>())
        .and(store.clone())
        .and_then(search_chunks);

    let log = warp::log("obnam");
    let webroot = create.or(fetch).or(search).with(log);

    debug!("starting warp");
    warp::serve(webroot)
        .tls()
        .key_path(config.tls_key)
        .cert_path(config.tls_cert)
        .run(addresses[0])
        .await;
    Ok(())
}

fn load_config(filename: &Path) -> Result<ServerConfig, anyhow::Error> {
    let config = ServerConfig::read_config(filename).with_context(|| {
        format!(
            "Couldn't read default configuration file {}",
            filename.display()
        )
    })?;
    Ok(config)
}

pub async fn create_chunk(
    store: Arc<Mutex<ChunkStore>>,
    meta: String,
    data: Bytes,
) -> Result<impl warp::Reply, warp::Rejection> {
    let store = store.lock().await;

    let meta: ChunkMeta = match meta.parse() {
        Ok(s) => s,
        Err(e) => {
            error!("chunk-meta header is bad: {}", e);
            return Ok(ChunkResult::BadRequest);
        }
    };

    let id = match store.put(data.to_vec(), &meta).await {
        Ok(id) => id,
        Err(e) => {
            error!("couldn't save: {}", e);
            return Ok(ChunkResult::InternalServerError);
        }
    };

    info!("created chunk {}", id);
    Ok(ChunkResult::Created(id))
}

pub async fn fetch_chunk(
    id: String,
    store: Arc<Mutex<ChunkStore>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let store = store.lock().await;
    let id: ChunkId = id.parse().unwrap();
    match store.get(&id).await {
        Ok((data, meta)) => {
            info!("found chunk {}: {:?}", id, meta);
            Ok(ChunkResult::Fetched(meta, data))
        }
        Err(e) => {
            error!("chunk not found: {}: {:?}", id, e);
            Ok(ChunkResult::NotFound)
        }
    }
}

pub async fn search_chunks(
    query: HashMap<String, String>,
    store: Arc<Mutex<ChunkStore>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let store = store.lock().await;

    let mut query = query.iter();
    let found = if let Some((key, value)) = query.next() {
        if query.next().is_some() {
            error!("search has more than one key to search for");
            return Ok(ChunkResult::BadRequest);
        }
        if key == "label" {
            let label = Label::deserialize(value).unwrap();
            let label = ChunkMeta::new(&label);
            store
                .find_by_label(&label)
                .await
                .expect("SQL lookup failed")
        } else {
            error!("unknown search key {:?}", key);
            return Ok(ChunkResult::BadRequest);
        }
    } else {
        error!("search has no key to search for");
        return Ok(ChunkResult::BadRequest);
    };

    let mut hits = SearchHits::default();
    for chunk_id in found {
        let (_, meta) = match store.get(&chunk_id).await {
            Ok(meta) => {
                info!("search found chunk {}", chunk_id);
                meta
            }
            Err(err) => {
                error!(
                    "search found chunk {} in index, but but not on disk: {}",
                    chunk_id, err
                );
                return Ok(ChunkResult::InternalServerError);
            }
        };
        hits.insert(&chunk_id, meta);
    }

    info!("search found {} hits", hits.len());
    Ok(ChunkResult::Found(hits))
}

#[derive(Default, Clone, Serialize)]
struct SearchHits {
    map: HashMap<String, ChunkMeta>,
}

impl SearchHits {
    fn insert(&mut self, chunk_id: &ChunkId, meta: ChunkMeta) {
        self.map.insert(chunk_id.to_string(), meta);
    }

    fn to_json(&self) -> String {
        serde_json::to_string(&self.map).unwrap()
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}

enum ChunkResult {
    Created(ChunkId),
    Fetched(ChunkMeta, Vec<u8>),
    Found(SearchHits),
    NotFound,
    BadRequest,
    InternalServerError,
}

#[derive(Debug, Serialize)]
struct CreatedBody {
    chunk_id: String,
}

impl warp::Reply for ChunkResult {
    fn into_response(self) -> warp::reply::Response {
        match self {
            ChunkResult::Created(id) => {
                let body = CreatedBody {
                    chunk_id: id.to_string(),
                };
                let body = serde_json::to_string(&body).unwrap();
                json_response(StatusCode::CREATED, body, None)
            }
            ChunkResult::Fetched(meta, chunk) => {
                let mut headers = HashMap::new();
                headers.insert(
                    "chunk-meta".to_string(),
                    serde_json::to_string(&meta).unwrap(),
                );
                into_response(
                    StatusCode::OK,
                    &chunk,
                    "application/octet-stream",
                    Some(headers),
                )
            }
            ChunkResult::Found(hits) => json_response(StatusCode::OK, hits.to_json(), None),
            ChunkResult::BadRequest => status_response(StatusCode::BAD_REQUEST),
            ChunkResult::NotFound => status_response(StatusCode::NOT_FOUND),
            ChunkResult::InternalServerError => status_response(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

// Construct a response with a JSON and maybe some extra headers.
fn json_response(
    status: StatusCode,
    json: String,
    headers: Option<HashMap<String, String>>,
) -> warp::reply::Response {
    into_response(status, json.as_bytes(), "application/json", headers)
}

// Construct a body-less response with just a status.
fn status_response(status: StatusCode) -> warp::reply::Response {
    into_response(status, b"", "text/json", None)
}

// Construct a custom HTTP response.
//
// If constructing the response fails, return an internal server
// error. If constructing that response also fails, panic.
fn into_response(
    status: StatusCode,
    body: &[u8],
    content_type: &str,
    headers: Option<HashMap<String, String>>,
) -> warp::reply::Response {
    match response(status, body, content_type, headers) {
        Ok(x) => x,
        Err(_) => response(StatusCode::INTERNAL_SERVER_ERROR, b"", "text/plain", None).unwrap(),
    }
}

// Construct a warp::reply::Response if possible.
//
// Note that this can fail. If so the caller needs to handle that in some way.
fn response(
    status: StatusCode,
    body: &[u8],
    content_type: &str,
    headers: Option<HashMap<String, String>>,
) -> anyhow::Result<warp::reply::Response> {
    // Create a new Response, using the generic body we've been given.
    let mut r = warp::reply::Response::new(body.to_vec().into());

    // Insert the content-type header.
    r.headers_mut().insert(
        warp::http::header::CONTENT_TYPE,
        warp::http::header::HeaderValue::from_str(content_type)?,
    );

    // Insert custom headers, if any.
    if let Some(h) = headers {
        for (h, v) in h.iter() {
            r.headers_mut().insert(
                warp::http::header::HeaderName::from_lowercase(h.as_bytes())?,
                warp::http::header::HeaderValue::from_str(v)?,
            );
        }
    }

    // Set the HTTP status code.
    *r.status_mut() = status;

    // Everything went well.
    Ok(r)
}
