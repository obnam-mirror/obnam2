use obnam::benchmark::ChunkGenerator;
use obnam::indexedstore::IndexedStore;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "benchmark-indexedstore",
    about = "Benhcmark the store without HTTP"
)]
enum Opt {
    Create {
        #[structopt(parse(from_os_str))]
        chunks: PathBuf,

        #[structopt()]
        num: u32,
    },

    Lookup {
        #[structopt(parse(from_os_str))]
        chunks: PathBuf,

        #[structopt()]
        warmup_count: u32,

        #[structopt()]
        hot_count: u32,
    },
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opt = Opt::from_args();

    match opt {
        Opt::Create { chunks, num } => create(&chunks, num)?,
        Opt::Lookup {
            chunks,
            warmup_count,
            hot_count,
        } => {
            let mut index = IndexedStore::new(&chunks)?;
            let time = SystemTime::now();
            warmup(&mut index, warmup_count)?;
            let warmup_time = time.elapsed()?;
            hot(&mut index, hot_count)?;
            let hot_time = time.elapsed()? - warmup_time;
            println!("warmup {}", warmup_time.as_millis());
            println!("hot    {}", hot_time.as_millis());
        }
    }

    Ok(())
}

fn create(chunks: &Path, num: u32) -> anyhow::Result<()> {
    let mut store = IndexedStore::new(chunks)?;
    let gen = ChunkGenerator::new(num);

    for (_, _, chunk) in gen {
        store.save(&chunk)?;
    }

    Ok(())
}

fn warmup(index: &mut IndexedStore, num: u32) -> anyhow::Result<()> {
    println!("warming up cache");
    lookup(index, num)
}

fn hot(index: &mut IndexedStore, num: u32) -> anyhow::Result<()> {
    println!("using hot cache");
    lookup(index, num)
}

fn lookup(index: &mut IndexedStore, num: u32) -> anyhow::Result<()> {
    let mut done = 0;

    loop {
        let gen = ChunkGenerator::new(num);
        for (_, _, chunk) in gen {
            index.find_by_sha256(chunk.meta().sha256())?;
            done += 1;
            if done >= num {
                return Ok(());
            }
        }
    }
}
