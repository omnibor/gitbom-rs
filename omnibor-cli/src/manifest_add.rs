//! The `manifest add` command, which adds manifests.

use crate::cli::Config;
use crate::cli::ManifestAddArgs;
use crate::print::PrinterCmd;
use anyhow::Result;
use tokio::sync::mpsc::Sender;

/// Run the `manifest add` subcommand.
pub async fn run(
    _tx: &Sender<PrinterCmd>,
    _config: &Config,
    _args: &ManifestAddArgs,
) -> Result<()> {
    todo!()
}
