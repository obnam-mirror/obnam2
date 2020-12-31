use crate::chunkid::ChunkId;
use crate::generation::FinishedGeneration;

pub struct GenerationList {
    list: Vec<FinishedGeneration>,
}

impl GenerationList {
    pub fn new(gens: Vec<FinishedGeneration>) -> Self {
        let mut list = gens.clone();
        list.sort_by_cached_key(|gen| gen.ended().to_string());
        Self { list }
    }

    pub fn iter(&self) -> impl Iterator<Item = &FinishedGeneration> {
        self.list.iter()
    }

    pub fn resolve(&self, genref: &str) -> Option<String> {
        let gen = if self.list.is_empty() {
            eprintln!("genlist: empty");
            None
        } else if genref == "latest" {
            let i = self.list.len() - 1;
            eprintln!("genlist: latest={} of {}", i, self.list.len());
            Some(self.list[i].clone())
        } else {
            let genref: ChunkId = genref.parse().unwrap();
            let hits: Vec<FinishedGeneration> = self
                .iter()
                .filter(|gen| gen.id() == genref)
                .map(|gen| gen.clone())
                .collect();
            eprintln!("genlist: hits={}", hits.len());
            if hits.len() == 1 {
                Some(hits[0].clone())
            } else {
                None
            }
        };
        let ret = match gen {
            None => None,
            Some(gen) => Some(gen.id().to_string()),
        };
        eprintln!("genlist: return {:?}", ret);
        ret
    }
}
