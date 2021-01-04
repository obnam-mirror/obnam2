use log::{debug, error, info, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use obnam::client::ClientConfig;
use obnam::cmd::{backup, get_chunk, list, list_files, restore};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

const BUFFER_SIZE: usize = 1024 * 1024;

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let config = ClientConfig::read_config(&opt.config)?;
    if let Some(ref log) = config.log {
        setup_logging(&log)?;
    }

    info!("client starts");
    debug!("{:?}", opt);

    let result = match opt.cmd {
        Command::Backup => backup(&config, BUFFER_SIZE),
        Command::List => list(&config),
        Command::ListFiles { gen_id } => list_files(&config, &gen_id),
        Command::Restore { gen_id, to } => restore(&config, &gen_id, &to),
        Command::GetChunk { chunk_id } => get_chunk(&config, &chunk_id),
    };

    if let Err(ref e) = result {
        error!("{}", e);
        eprintln!("ERROR: {}", e);
        return result;
    }

    info!("client ends successfully");
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(long, short, parse(from_os_str))]
    config: PathBuf,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    Backup,
    List,
    ListFiles {
        #[structopt(default_value = "latest")]
        gen_id: String,
    },
    Restore {
        #[structopt()]
        gen_id: String,

        #[structopt(parse(from_os_str))]
        to: PathBuf,
    },
    GetChunk {
        #[structopt()]
        chunk_id: String,
    },
}

fn setup_logging(filename: &Path) -> anyhow::Result<()> {
    let logfile = FileAppender::builder().build(filename)?;

    let config = Config::builder()
        .appender(Appender::builder().build("obnam", Box::new(logfile)))
        .logger(Logger::builder().build("obnam", LevelFilter::Debug))
        .build(Root::builder().appender("obnam").build(LevelFilter::Debug))?;

    log4rs::init_config(config)?;

    Ok(())
}
