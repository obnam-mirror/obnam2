use crate::backup_reason::Reason;
use crate::client::BackupClient;
use crate::config::ClientConfig;
use crate::error::ObnamError;
use crate::fsentry::{FilesystemEntry, FilesystemKind};
use tempfile::NamedTempFile;

pub fn list_files(config: &ClientConfig, gen_ref: &str) -> Result<(), ObnamError> {
    let temp = NamedTempFile::new()?;

    let client = BackupClient::new(config)?;

    let genlist = client.list_generations()?;
    let gen_id: String = genlist.resolve(gen_ref)?;

    let gen = client.fetch_generation(&gen_id, temp.path())?;
    for file in gen.files()? {
        println!("{}", format_entry(&file.entry(), file.reason()));
    }

    Ok(())
}

fn format_entry(e: &FilesystemEntry, reason: Reason) -> String {
    let kind = match e.kind() {
        FilesystemKind::Regular => "-",
        FilesystemKind::Directory => "d",
        FilesystemKind::Symlink => "l",
        FilesystemKind::Socket => "s",
        FilesystemKind::Fifo => "p",
    };
    format!("{} {} ({})", kind, e.pathbuf().display(), reason)
}
