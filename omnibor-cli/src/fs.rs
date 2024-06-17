//! File system helper operations.

use crate::cli::Format;
use crate::cli::SelectedHash;
use crate::print::PrinterCmd;
use anyhow::Context as _;
use anyhow::Result;
use async_walkdir::DirEntry as AsyncDirEntry;
use async_walkdir::WalkDir;
use futures_lite::stream::StreamExt as _;
use omnibor::hashes::Sha256;
use omnibor::ArtifactId;
use std::path::Path;
use tokio::fs::File as AsyncFile;
use tokio::sync::mpsc::Sender;
use url::Url;

// Identify, recursively, all the files under a directory.
pub async fn id_directory(
    tx: &Sender<PrinterCmd>,
    path: &Path,
    format: Format,
    hash: SelectedHash,
) -> Result<()> {
    let mut entries = WalkDir::new(path);

    loop {
        match entries.next().await {
            None => break,
            Some(Err(e)) => tx.send(PrinterCmd::error(e, format)).await?,
            Some(Ok(entry)) => {
                let path = &entry.path();

                if entry_is_dir(&entry).await? {
                    continue;
                }

                let mut file = open_async_file(path).await?;
                id_file(tx, &mut file, path, format, hash).await?;
            }
        }
    }

    Ok(())
}

/// Identify a single file.
pub async fn id_file(
    tx: &Sender<PrinterCmd>,
    file: &mut AsyncFile,
    path: &Path,
    format: Format,
    hash: SelectedHash,
) -> Result<()> {
    let url = hash_file(hash, file, path).await?;
    tx.send(PrinterCmd::id(path, &url, format)).await?;
    Ok(())
}

/// Hash the file and produce a `gitoid`-scheme URL.
pub async fn hash_file(hash: SelectedHash, file: &mut AsyncFile, path: &Path) -> Result<Url> {
    match hash {
        SelectedHash::Sha256 => sha256_id_async_file(file, path).await.map(|id| id.url()),
    }
}

/// Check if the file is for a directory.
pub async fn file_is_dir(file: &AsyncFile) -> Result<bool> {
    Ok(file.metadata().await.map(|meta| meta.is_dir())?)
}

/// Check if the entry is for a directory.
pub async fn entry_is_dir(entry: &AsyncDirEntry) -> Result<bool> {
    entry
        .file_type()
        .await
        .with_context(|| {
            format!(
                "unable to identify file type for '{}'",
                entry.path().display()
            )
        })
        .map(|file_type| file_type.is_dir())
}

/// Open an asynchronous file.
pub async fn open_async_file(path: &Path) -> Result<AsyncFile> {
    AsyncFile::open(path)
        .await
        .with_context(|| format!("failed to open file '{}'", path.display()))
}

/// Identify a file using a SHA-256 hash.
pub async fn sha256_id_async_file(file: &mut AsyncFile, path: &Path) -> Result<ArtifactId<Sha256>> {
    ArtifactId::id_async_reader(file)
        .await
        .with_context(|| format!("failed to produce Artifact ID for '{}'", path.display()))
}
