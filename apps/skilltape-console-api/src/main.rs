use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "skilltape-console-api")]
struct Args {
    #[arg(long, default_value = ".")]
    workspace: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    bind: IpAddr,
    #[arg(long, default_value_t = 0)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), skilltape_console_api::ServeError> {
    let args = Args::parse();
    skilltape_console_api::serve(&args.workspace, SocketAddr::new(args.bind, args.port)).await
}
