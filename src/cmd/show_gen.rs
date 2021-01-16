use crate::client::BackupClient;
use crate::client::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::FilesystemKind;
use indicatif::HumanBytes;
use tempfile::NamedTempFile;

pub fn show_generation(config: &ClientConfig, gen_ref: &str) -> anyhow::Result<()> {
    // Create a named temporary file. We don't meed the open file
    // handle, so we discard that.
    let dbname = {
        let temp = NamedTempFile::new()?;
        let (_, dbname) = temp.keep()?;
        dbname
    };

    let client = BackupClient::new(&config.server_url)?;

    let genlist = client.list_generations()?;
    let gen_id: String = match genlist.resolve(gen_ref) {
        None => return Err(ObnamError::UnknownGeneration(gen_ref.to_string()).into()),
        Some(id) => id,
    };

    let gen = client.fetch_generation(&gen_id, &dbname)?;
    let files = gen.files()?;

    let total_bytes = files.iter().fold(0, |acc, file| {
        let e = file.entry();
        if e.kind() == FilesystemKind::Regular {
            acc + file.entry().len()
        } else {
            acc
        }
    });

    println!("generation-id: {}", gen_id);
    println!("file-count: {}", gen.file_count()?);
    println!("file-bytes: {}", HumanBytes(total_bytes));
    println!("file-bytes-raw: {}", total_bytes);

    // Delete the temporary file.
    std::fs::remove_file(&dbname)?;

    Ok(())
}
