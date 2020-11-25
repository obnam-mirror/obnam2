use obnam::benchmark::ChunkGenerator;
use obnam::index::Index;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "benchmark-index",
    about = "Benhcmark the store index in memory"
)]
struct Opt {
    // We don't use this, but we accept it for command line
    // compatibility with other benchmark programs.
    #[structopt(parse(from_os_str))]
    chunks: PathBuf,

    #[structopt()]
    num: u32,
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let gen = ChunkGenerator::new(opt.num);

    let mut index = Index::default();
    for (id, checksum, _, _) in gen {
        index.insert(id, "sha25", &checksum);
    }

    Ok(())
}
