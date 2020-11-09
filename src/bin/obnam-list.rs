use log::{debug, info};
use obnam::client::{BackupClient, ClientConfig};
use std::path::PathBuf;
use structopt::StructOpt;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    info!("obnam-list starts");
    debug!("opt: {:?}", opt);
    let config = ClientConfig::read_config(&opt.config)?;
    let client = BackupClient::new(&config.server_name, config.server_port)?;

    for gen_id in client.list_generations()? {
        println!("{}", gen_id);
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,
}
