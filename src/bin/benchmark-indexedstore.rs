use obnam::benchmark::ChunkGenerator;
use obnam::indexedstore::IndexedStore;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "benchmark-store", about = "Benhcmark the store without HTTP")]
struct Opt {
    #[structopt(parse(from_os_str))]
    chunks: PathBuf,

    #[structopt()]
    num: u32,
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let gen = ChunkGenerator::new(opt.num);

    let mut store = IndexedStore::new(&opt.chunks)?;
    for (_, _, meta, chunk) in gen {
        store.save(&meta, &chunk)?;
    }

    Ok(())
}
