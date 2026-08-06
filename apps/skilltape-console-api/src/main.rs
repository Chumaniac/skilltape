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
    #[arg(long)]
    static_root: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), skilltape_console_api::ServeError> {
    let args = Args::parse();
    skilltape_console_api::serve_with_static(
        &args.workspace,
        SocketAddr::new(args.bind, args.port),
        args.static_root.as_deref(),
    )
    .await
}
