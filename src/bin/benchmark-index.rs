use obnam::benchmark::ChunkGenerator;
use obnam::index::Index;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "benchmark-index",
    about = "Benhcmark the chunk store index without HTTP"
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
            let mut index = Index::new(chunks)?;
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
    let mut index = Index::new(chunks)?;
    let gen = ChunkGenerator::new(num);

    for (id, _, meta, _) in gen {
        index.insert_meta(id, meta)?;
    }

    Ok(())
}

fn warmup(index: &mut Index, num: u32) -> anyhow::Result<()> {
    println!("warming up cache");
    lookup(index, num)
}

fn hot(index: &mut Index, num: u32) -> anyhow::Result<()> {
    println!("using hot cache");
    lookup(index, num)
}

fn lookup(index: &mut Index, num: u32) -> anyhow::Result<()> {
    let mut done = 0;

    loop {
        let gen = ChunkGenerator::new(num);
        for (_, _, meta, _) in gen {
            index.find_by_sha256(&meta.sha256())?;
            done += 1;
            if done >= num {
                return Ok(());
            }
        }
    }
}
