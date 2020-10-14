use serde::Serialize;

/// Store an arbitrary chunk of data.
///
/// The data is just arbitrary binary data.
///
/// A chunk also contains its associated metadata, except its
/// identifier.
#[derive(Debug, Serialize)]
pub struct DataChunk {
    data: Vec<u8>,
}

impl DataChunk {
    /// Construct a new chunk.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Return a chunk's data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
