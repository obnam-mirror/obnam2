use directories_next::ProjectDirs;
use log::{debug, error, info, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Logger, Root};
use obnam::cmd::backup::Backup;
use obnam::cmd::get_chunk::GetChunk;
use obnam::cmd::init::Init;
use obnam::cmd::list::List;
use obnam::cmd::list_files::ListFiles;
use obnam::cmd::restore::Restore;
use obnam::cmd::show_config::ShowConfig;
use obnam::cmd::show_gen::ShowGeneration;
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
    let result = if let Command::Init(init) = opt.cmd {
        init.run(config.config(), &cfgname)
    } else {
        let config = load_config_with_passwords(&opt)?;
        match opt.cmd {
            Command::Init(_) => panic!("this cannot happen"),
            Command::Backup(x) => x.run(&config),
            Command::List(x) => x.run(&config),
            Command::ShowGeneration(x) => x.run(&config),
            Command::ListFiles(x) => x.run(&config),
            Command::Restore(x) => x.run(&config),
            Command::GetChunk(x) => x.run(&config),
            Command::Config(x) => x.run(&config),
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

    let config = log4rs::Config::builder()
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
    Init(Init),
    Backup(Backup),
    List(List),
    ListFiles(ListFiles),
    Restore(Restore),
    ShowGeneration(ShowGeneration),
    GetChunk(GetChunk),
    Config(ShowConfig),
}
