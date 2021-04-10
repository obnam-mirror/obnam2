use directories_next::ProjectDirs;
use log::{debug, error, info, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use obnam::cmd::{
    backup, get_chunk, init, list, list_files, restore, show_config, show_generation,
};
use obnam::config::ClientConfig;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

const QUALIFIER: &str = "";
const ORG: &str = "";
const APPLICATION: &str = "obnam";

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let config = load_config_without_passwords(&opt)?;
    setup_logging(&config.config().log)?;

    info!("client starts");
    debug!("{:?}", opt);
    debug!("configuration: {:#?}", config);

    let cfgname = config_filename(&opt);
    let result = if let Command::Init {
        insecure_passphrase,
    } = opt.cmd
    {
        init(config.config(), &cfgname, insecure_passphrase)
    } else {
        let config = load_config_with_passwords(&opt)?;
        match opt.cmd {
            Command::Init {
                insecure_passphrase: _,
            } => panic!("this cannot happen"),
            Command::Backup => backup(&config),
            Command::List => list(&config),
            Command::ShowGeneration { gen_id } => show_generation(&config, &gen_id),
            Command::ListFiles { gen_id } => list_files(&config, &gen_id),
            Command::Restore { gen_id, to } => restore(&config, &gen_id, &to),
            Command::GetChunk { chunk_id } => get_chunk(&config, &chunk_id),
            Command::Config => show_config(&config),
        }
    };

    if let Err(ref e) = result {
        error!("command failed: {}", e);
        result?
    }

    info!("client ends successfully");
    Ok(())
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

fn load_config_with_passwords(opt: &Opt) -> Result<ClientConfig, anyhow::Error> {
    Ok(ClientConfig::read_with_passwords(&config_filename(opt))?)
}

fn load_config_without_passwords(opt: &Opt) -> Result<ClientConfig, anyhow::Error> {
    Ok(ClientConfig::read_without_passwords(&config_filename(opt))?)
}

fn config_filename(opt: &Opt) -> PathBuf {
    match opt.config {
        None => default_config(),
        Some(ref filename) => filename.to_path_buf(),
    }
}

fn default_config() -> PathBuf {
    if let Some(dirs) = ProjectDirs::from(QUALIFIER, ORG, APPLICATION) {
        dirs.config_dir().join("obnam.yaml")
    } else {
        panic!("can't figure out the configuration directory");
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
    Init {
        #[structopt(long)]
        insecure_passphrase: Option<String>,
    },
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
