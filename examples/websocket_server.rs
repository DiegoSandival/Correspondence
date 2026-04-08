use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;

use correspondence::{serve_websocket, WebServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let data_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("cwd should exist").join("ws-data"));
    let bind_addr: SocketAddr = args
        .get(2)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or_else(|| "127.0.0.1:3000".parse().expect("default socket addr should parse"));

    fs::create_dir_all(&data_dir)?;

    println!("serving web ui on http://{bind_addr}");
    println!("data root: {}", data_dir.display());
    println!("binary websocket endpoint: ws://{bind_addr}/ws");

    serve_websocket(WebServerConfig::new(bind_addr, data_dir)).await?;
    Ok(())
}
