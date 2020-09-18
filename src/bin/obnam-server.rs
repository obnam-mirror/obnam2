use bytes::Bytes;
use obnam::{chunk::Chunk, chunkid::ChunkId, chunkmeta::ChunkMeta, index::Index, store::Store};
use serde::{Deserialize, Serialize};
use std::default::Default;
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
    let opt = Opt::from_args();
    let config = Config::read_config(&opt.config).unwrap();
    let config_bare = config.clone();
    let config = Arc::new(Mutex::new(config));
    let config = warp::any().map(move || Arc::clone(&config));

    let index = Arc::new(Mutex::new(Index::default()));
    let index = warp::any().map(move || Arc::clone(&index));

    let create = warp::post()
        .and(warp::path("chunks"))
        .and(config.clone())
        .and(index.clone())
        .and(warp::header("chunk-meta"))
        .and(warp::filters::body::bytes())
        .and_then(create_chunk);

    let fetch = warp::get()
        .and(warp::path("chunks"))
        .and(warp::path::param())
        .and(config.clone())
        .and_then(fetch_chunk);

    // let search = warp::get()
    //     .and(warp::path("chunks"))
    //     .and(warp::query::<HashMap<String, String>>())
    //     .and(config.clone())
    //     .and(index.clone())
    //     .and_then(obnam::routes::search::search_chunks);

    //    let webroot = create.or(fetch).or(search);
    let webroot = create.or(fetch);
    warp::serve(webroot)
        .tls()
        .key_path(config_bare.tls_key)
        .cert_path(config_bare.tls_cert)
        .run(([127, 0, 0, 1], config_bare.port))
        .await;
    Ok(())
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub chunks: PathBuf,
    pub port: u16,
    pub tls_key: PathBuf,
    pub tls_cert: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("Port number {0} too small, would require running as root")]
    PortTooSmall(u16),

    #[error("Directory for chunks {0} does not exist")]
    ChunksDirNotFound(PathBuf),

    #[error("TLS certificate {0} does not exist")]
    TlsCertNotFound(PathBuf),

    #[error("TLS key {0} does not exist")]
    TlsKeyNotFound(PathBuf),
}

impl Config {
    pub fn read_config(filename: &Path) -> anyhow::Result<Config> {
        let config = std::fs::read_to_string(filename)?;
        let config: Config = serde_yaml::from_str(&config)?;
        config.check()?;
        Ok(config)
    }

    pub fn check(&self) -> anyhow::Result<()> {
        if self.port < 1024 {
            return Err(ConfigError::PortTooSmall(self.port).into());
        }
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
    config: Arc<Mutex<Config>>,
    index: Arc<Mutex<Index>>,
    meta: String,
    data: Bytes,
) -> Result<impl warp::Reply, warp::Rejection> {
    let id = ChunkId::new();
    let config = config.lock().await;
    let store = Store::new(&config.chunks);

    let meta: ChunkMeta = match meta.parse() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("bad meta");
            return Ok(ChunkResult::BadRequest);
        }
    };

    let chunk = Chunk::new(meta.clone(), data.to_vec());

    match store.save(&id, &chunk) {
        Ok(_) => (),
        Err(_) => {
            eprintln!("no meta file");
            return Ok(ChunkResult::InternalServerError);
        }
    }

    let mut index = index.lock().await;
    index.insert(id.clone(), "sha256", meta.sha256());
    if meta.is_generation() {
        index.insert_generation(id.clone());
    }

    Ok(ChunkResult::Created(id))
}

pub async fn fetch_chunk(
    id: String,
    config: Arc<Mutex<Config>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let config = config.lock().await;
    let store = Store::new(&config.chunks);
    let id: ChunkId = id.parse().unwrap();
    match store.load(&id) {
        Ok(chunk) => Ok(ChunkResult::Fetched(chunk)),
        Err(_) => Err(warp::reject::not_found()),
    }
}

enum ChunkResult {
    Created(ChunkId),
    Fetched(Chunk),
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
                let mut r = warp::reply::Response::new(body.into());
                r.headers_mut().insert(
                    warp::http::header::CONTENT_TYPE,
                    warp::http::header::HeaderValue::from_static("application/json"),
                );
                *r.status_mut() = StatusCode::CREATED;
                r
            }
            ChunkResult::Fetched(chunk) => {
                let mut r = warp::reply::Response::new(chunk.data().to_vec().into());
                r.headers_mut().insert(
                    warp::http::header::CONTENT_TYPE,
                    warp::http::header::HeaderValue::from_static("application/octet-stream"),
                );
                r.headers_mut().insert(
                    "chunk-meta",
                    warp::http::header::HeaderValue::from_str(
                        &serde_json::to_string(&chunk.meta()).unwrap(),
                    )
                    .unwrap(),
                );
                *r.status_mut() = StatusCode::OK;
                r
            }
            ChunkResult::BadRequest => {
                let mut r = warp::reply::Response::new("".into());
                r.headers_mut().insert(
                    warp::http::header::CONTENT_TYPE,
                    warp::http::header::HeaderValue::from_static("application/json"),
                );
                *r.status_mut() = StatusCode::BAD_REQUEST;
                r
            }
            ChunkResult::InternalServerError => {
                let mut r = warp::reply::Response::new("".into());
                r.headers_mut().insert(
                    warp::http::header::CONTENT_TYPE,
                    warp::http::header::HeaderValue::from_static("application/json"),
                );
                *r.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                r
            }
        }
    }
}
