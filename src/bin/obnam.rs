use log::{debug, info};
use obnam::cmd::{backup, list, restore};
use std::path::PathBuf;
use structopt::StructOpt;

const BUFFER_SIZE: usize = 1024 * 1024;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    info!("obnam starts");
    debug!("opt: {:?}", opt);

    match opt {
        Opt::Backup { config } => backup(&config, BUFFER_SIZE)?,
        Opt::List { config } => list(&config)?,
        Opt::Restore {
            config,
            gen_id,
            dbname,
            to,
        } => restore(&config, &gen_id, &dbname, &to)?,
    }
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
enum Opt {
    Backup {
        #[structopt(parse(from_os_str))]
        config: PathBuf,
    },
    List {
        #[structopt(parse(from_os_str))]
        config: PathBuf,
    },
    Restore {
        #[structopt(parse(from_os_str))]
        config: PathBuf,

        #[structopt()]
        gen_id: String,

        #[structopt(parse(from_os_str))]
        dbname: PathBuf,

        #[structopt(parse(from_os_str))]
        to: PathBuf,
    },
}
