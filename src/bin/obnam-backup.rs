use log::{debug, info};
use obnam::client::{BackupClient, ClientConfig};
use obnam::fsiter::FsIterator;
use obnam::generation::Generation;
use std::path::PathBuf;
use structopt::StructOpt;

const BUFFER_SIZE: usize = 1024 * 1024;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    info!("obnam-backup starts");
    debug!("opt: {:?}", opt);
    let config = ClientConfig::read_config(&opt.config)?;
    let client = BackupClient::new(&config.server_name, config.server_port)?;

    {
        let mut gen = Generation::create(&config.dbname)?;
        gen.insert_iter(FsIterator::new(&config.root).map(|entry| match entry {
            Err(err) => Err(err),
            Ok(entry) => client.upload_filesystem_entry(entry, BUFFER_SIZE),
        }))?;
    }
    let gen_id = client.upload_generation(&config.dbname, BUFFER_SIZE)?;
    println!("gen id: {}", gen_id);

    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "obnam-backup", about = "Simplistic backup client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,
}
