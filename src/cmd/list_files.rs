use crate::backup_reason::Reason;
use crate::client::BackupClient;
use crate::client::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use tempfile::NamedTempFile;

pub fn list_files(config: &ClientConfig, gen_ref: &str) -> anyhow::Result<()> {
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
    for file in gen.files()? {
        println!("{}", format_entry(&file.entry(), file.reason()));
    }

    // Delete the temporary file.
    std::fs::remove_file(&dbname)?;

    Ok(())
}

fn format_entry(e: &FilesystemEntry, reason: Reason) -> String {
    let kind = match e.kind() {
        FilesystemKind::Regular => "-",
        FilesystemKind::Directory => "d",
        FilesystemKind::Symlink => "l",
    };
    format!("{} {} ({})", kind, e.pathbuf().display(), reason)
}
