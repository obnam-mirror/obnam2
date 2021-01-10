use bytes::Bytes;
use log::{debug, error, info};
use obnam::chunk::DataChunk;
use obnam::chunkid::ChunkId;
use obnam::chunkmeta::ChunkMeta;
use obnam::indexedstore::IndexedStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use structopt::StructOpt;
use tokio::sync::Mutex;
use warp::http::StatusCode;
use warp::Filter;

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam2-server", about = "Backup server")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let config = Config::read_config(&opt.config).unwrap();

    let addresses: Vec<SocketAddr> = config.address.to_socket_addrs()?.collect();
    if addresses.is_empty() {
        error!("specified address is empty set: {:?}", addresses);
        eprintln!("ERROR: server address is empty: {:?}", addresses);
        return Err(ConfigError::BadServerAddress.into());
    }

    let store = IndexedStore::new(&config.chunks)?;
    let store = Arc::new(Mutex::new(store));
    let store = warp::any().map(move || Arc::clone(&store));

    info!("Obnam server starting up");
    debug!("opt: {:#?}", opt);
    debug!("Configuration: {:#?}", config);

    let create = warp::post()
        .and(warp::path("chunks"))
        .and(store.clone())
        .and(warp::header("chunk-meta"))
        .and(warp::filters::body::bytes())
        .and_then(create_chunk);

    let fetch = warp::get()
        .and(warp::path("chunks"))
        .and(warp::path::param())
        .and(store.clone())
        .and_then(fetch_chunk);

    let search = warp::get()
        .and(warp::path("chunks"))
        .and(warp::query::<HashMap<String, String>>())
        .and(store.clone())
        .and_then(search_chunks);

    let delete = warp::delete()
        .and(warp::path("chunks"))
        .and(warp::path::param())
        .and(store.clone())
        .and_then(delete_chunk);

    let log = warp::log("obnam");
    let webroot = create.or(fetch).or(search).or(delete).with(log);

    debug!("starting warp");
    warp::serve(webroot)
        .tls()
        .key_path(config.tls_key)
        .cert_path(config.tls_cert)
        .run(addresses[0])
        .await;
    Ok(())
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub chunks: PathBuf,
    pub address: String,
    pub tls_key: PathBuf,
    pub tls_cert: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("Directory for chunks {0} does not exist")]
    ChunksDirNotFound(PathBuf),

    #[error("TLS certificate {0} does not exist")]
    TlsCertNotFound(PathBuf),

    #[error("TLS key {0} does not exist")]
    TlsKeyNotFound(PathBuf),

    #[error("server address can't be resolved")]
    BadServerAddress,
}

impl Config {
    pub fn read_config(filename: &Path) -> anyhow::Result<Config> {
        let config = std::fs::read_to_string(filename)?;
        let config: Config = serde_yaml::from_str(&config)?;
        config.check()?;
        Ok(config)
    }

    pub fn check(&self) -> anyhow::Result<()> {
        if !self.chunks.exists() {
            return Err(ConfigError::ChunksDirNotFound(self.chunks.clone()).into());
        }
        if !self.tls_cert.exists() {
            return Err(ConfigError::TlsCertNotFound(self.tls_cert.clone()).into());
        }
        if !self.tls_key.exists() {
            return Err(ConfigError::TlsKeyNotFound(self.tls_key.clone()).into());
        }
        Ok(())
    }
}

pub async fn create_chunk(
    store: Arc<Mutex<IndexedStore>>,
    meta: String,
    data: Bytes,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut store = store.lock().await;

    let meta: ChunkMeta = match meta.parse() {
        Ok(s) => s,
        Err(e) => {
            error!("chunk-meta header is bad: {}", e);
            return Ok(ChunkResult::BadRequest);
        }
    };

    let chunk = DataChunk::new(data.to_vec());

    let id = match store.save(&meta, &chunk) {
        Ok(id) => id,
        Err(e) => {
            error!("couldn't save: {}", e);
            return Ok(ChunkResult::InternalServerError);
        }
    };

    info!("created chunk {}: {:?}", id, meta);
    Ok(ChunkResult::Created(id))
}

pub async fn fetch_chunk(
    id: String,
    store: Arc<Mutex<IndexedStore>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let store = store.lock().await;
    let id: ChunkId = id.parse().unwrap();
    match store.load(&id) {
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
    store: Arc<Mutex<IndexedStore>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let store = store.lock().await;

    let mut query = query.iter();
    let found = if let Some((key, value)) = query.next() {
        if query.next() != None {
            error!("search has more than one key to search for");
            return Ok(ChunkResult::BadRequest);
        }
        if key == "generation" && value == "true" {
            store.find_generations().expect("SQL lookup failed")
        } else if key == "sha256" {
            store.find_by_sha256(value).expect("SQL lookup failed")
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
        let meta = match store.load_meta(&chunk_id) {
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

pub async fn delete_chunk(
    id: String,
    store: Arc<Mutex<IndexedStore>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut store = store.lock().await;
    let id: ChunkId = id.parse().unwrap();

    match store.remove(&id) {
        Ok(_) => {
            info!("chunk deleted: {}", id);
            Ok(ChunkResult::Deleted)
        }
        Err(e) => {
            error!("could not delete chunk {}: {:?}", id, e);
            Ok(ChunkResult::NotFound)
        }
    }
}

enum ChunkResult {
    Created(ChunkId),
    Fetched(ChunkMeta, DataChunk),
    Found(SearchHits),
    Deleted,
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
                    chunk.data(),
                    "application/octet-stream",
                    Some(headers),
                )
            }
            ChunkResult::Found(hits) => json_response(StatusCode::OK, hits.to_json(), None),
            ChunkResult::Deleted => status_response(StatusCode::OK),
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
