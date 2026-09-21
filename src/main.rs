//! The `vivi-mail` binary.

use std::process;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use vivi_mail::Runtime;
use vivi_mail::VivariumError;
use vivi_mail::cli::{MailCli, MailCommand};

#[tokio::main]
async fn main() {
    let cli = MailCli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if cli.verbose {
                EnvFilter::new("vivi_mail=debug")
            } else {
                EnvFilter::new("vivi_mail=info")
            }
        }))
        .init();

    if let Err(e) = run(cli).await {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

async fn run(cli: MailCli) -> Result<(), VivariumError> {
    let MailCli {
        config,
        account,
        insecure,
        ignore_permissions,
        command,
        ..
    } = cli;

    // Bootstrap writes config.toml and accounts.toml, so it must run before the
    // mail runtime tries to load them.
    if matches!(command, MailCommand::Init) {
        return vivi_mail::init::run_init();
    }

    let runtime = Runtime::load(config, account, insecure, ignore_permissions)?;
    runtime.run(command).await
}
