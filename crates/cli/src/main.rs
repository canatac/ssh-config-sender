mod crypto;
mod framing;
mod pack;
mod receive;
mod send;
mod transport;
mod unpack;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sshmigrate",
    about = "Migrate ~/.ssh between machines with E2EE"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Receive SSH config from another machine
    Receive {
        #[arg(long, default_value = "8444")]
        port: u16,
        #[arg(long)]
        to: Option<PathBuf>,
        #[arg(long)]
        relay: Option<String>,
    },
    /// Send SSH config to another machine
    Send {
        #[arg(long)]
        to: String,
        #[arg(long)]
        code: String,
        #[arg(long)]
        from: Option<PathBuf>,
        #[arg(long)]
        relay: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Receive { port, to, relay } => {
            let to = to.unwrap_or_else(|| {
                let mut home = dirs_home();
                home.push(".ssh");
                home
            });
            receive::run_receive(port, to, relay).await?;
        }
        Commands::Send {
            to,
            code,
            from,
            relay,
        } => {
            let from = from.unwrap_or_else(|| {
                let mut home = dirs_home();
                home.push(".ssh");
                home
            });
            send::run_send(to, code, from, relay).await?;
        }
    }

    Ok(())
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
