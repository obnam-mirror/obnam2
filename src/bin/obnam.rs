use log::{debug, error, info, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use obnam::client::ClientConfig;
use obnam::cmd::{backup, get_chunk, list, list_files, restore, show_config, show_generation};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let config_file = match opt.config {
        None => default_config(),
        Some(ref path) => path.to_path_buf(),
    };
    let config = ClientConfig::read_config(&config_file)?;
    setup_logging(&config.log)?;

    info!("client starts");
    debug!("{:?}", opt);

    let result = match opt.cmd {
        Command::Backup => backup(&config),
        Command::List => list(&config),
        Command::ShowGeneration { gen_id } => show_generation(&config, &gen_id),
        Command::ListFiles { gen_id } => list_files(&config, &gen_id),
        Command::Restore { gen_id, to } => restore(&config, &gen_id, &to),
        Command::GetChunk { chunk_id } => get_chunk(&config, &chunk_id),
        Command::Config => show_config(&config),
    };

    if let Err(ref e) = result {
        error!("{}", e);
        eprintln!("ERROR: {}", e);
        result?
    }

    info!("client ends successfully");
    Ok(())
}

fn default_config() -> PathBuf {
    if let Some(path) = dirs::config_dir() {
        path.join("obnam").join("obnam.yaml")
    } else if let Some(path) = dirs::home_dir() {
        path.join(".config").join("obnam").join("obnam.yaml")
    } else {
        panic!("can't find config dir or home dir");
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(long, short, parse(from_os_str))]
    config: Option<PathBuf>,

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
    ShowGeneration {
        #[structopt(default_value = "latest")]
        gen_id: String,
    },
    GetChunk {
        #[structopt()]
        chunk_id: String,
    },
    Config,
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
