//! The `artifact find` command, which finds files by ID.

use crate::cli::Config;
use crate::cli::FindArgs;
use crate::cli::SelectedHash;
use crate::fs::*;
use crate::print::PrinterCmd;
use anyhow::Result;
use async_walkdir::WalkDir;
use futures_lite::stream::StreamExt as _;
use tokio::sync::mpsc::Sender;

/// Run the `artifact find` subcommand.
pub async fn run(tx: &Sender<PrinterCmd>, config: &Config, args: &FindArgs) -> Result<()> {
    let FindArgs { aid, path } = args;

    let url = aid.url();

    let mut entries = WalkDir::new(path);

    loop {
        match entries.next().await {
            None => break,
            Some(Err(e)) => tx.send(PrinterCmd::error(e, config.format())).await?,
            Some(Ok(entry)) => {
                let path = &entry.path();

                if entry_is_dir(&entry).await? {
                    continue;
                }

                let mut file = open_async_file(path).await?;
                let file_url = hash_file(SelectedHash::Sha256, &mut file, path).await?;

                if url == file_url {
                    tx.send(PrinterCmd::find(path, &url, config.format()))
                        .await?;
                }
            }
        }
    }

    Ok(())
}
