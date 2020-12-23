use log::{debug, info};
use obnam::client::ClientConfig;
use obnam::cmd::{backup, list, restore};
use std::path::PathBuf;
use structopt::StructOpt;

const BUFFER_SIZE: usize = 1024 * 1024;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let config = ClientConfig::read_config(&opt.config)?;

    info!("obnam starts");
    debug!("opt: {:?}", opt);

    match opt.cmd {
        Command::Backup => backup(&config, BUFFER_SIZE)?,
        Command::List => list(&config)?,
        Command::Restore { gen_id, to } => restore(&config, &gen_id, &to)?,
    }
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
    Restore {
        #[structopt()]
        gen_id: String,

        #[structopt(parse(from_os_str))]
        to: PathBuf,
    },
}
