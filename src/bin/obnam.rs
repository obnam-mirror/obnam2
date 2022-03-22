use directories_next::ProjectDirs;
use log::{debug, error, info, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Logger, Root};
use obnam::cmd::backup::Backup;
use obnam::cmd::chunk::{DecryptChunk, EncryptChunk};
use obnam::cmd::chunkify::Chunkify;
use obnam::cmd::gen_info::GenInfo;
use obnam::cmd::get_chunk::GetChunk;
use obnam::cmd::init::Init;
use obnam::cmd::inspect::Inspect;
use obnam::cmd::list::List;
use obnam::cmd::list_backup_versions::ListSchemaVersions;
use obnam::cmd::list_files::ListFiles;
use obnam::cmd::resolve::Resolve;
use obnam::cmd::restore::Restore;
use obnam::cmd::show_config::ShowConfig;
use obnam::cmd::show_gen::ShowGeneration;
use obnam::config::ClientConfig;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

const QUALIFIER: &str = "";
const ORG: &str = "";
const APPLICATION: &str = "obnam";

fn main() {
    if let Err(err) = main_program() {
        error!("{}", err);
        eprintln!("ERROR: {}", err);
        std::process::exit(1);
    }
}

fn main_program() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let config = ClientConfig::read(&config_filename(&opt))?;
    setup_logging(&config.log)?;

    info!("client starts");
    debug!("{:?}", opt);
    debug!("configuration: {:#?}", config);

    match opt.cmd {
        Command::Init(x) => x.run(&config),
        Command::ListBackupVersions(x) => x.run(&config),
        Command::Backup(x) => x.run(&config),
        Command::Inspect(x) => x.run(&config),
        Command::Chunkify(x) => x.run(&config),
        Command::List(x) => x.run(&config),
        Command::ShowGeneration(x) => x.run(&config),
        Command::ListFiles(x) => x.run(&config),
        Command::Resolve(x) => x.run(&config),
        Command::Restore(x) => x.run(&config),
        Command::GenInfo(x) => x.run(&config),
        Command::GetChunk(x) => x.run(&config),
        Command::Config(x) => x.run(&config),
        Command::EncryptChunk(x) => x.run(&config),
        Command::DecryptChunk(x) => x.run(&config),
    }?;

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
    Inspect(Inspect),
    Chunkify(Chunkify),
    List(List),
    ListBackupVersions(ListSchemaVersions),
    ListFiles(ListFiles),
    Restore(Restore),
    GenInfo(GenInfo),
    ShowGeneration(ShowGeneration),
    Resolve(Resolve),
    GetChunk(GetChunk),
    Config(ShowConfig),
    EncryptChunk(EncryptChunk),
    DecryptChunk(DecryptChunk),
}
