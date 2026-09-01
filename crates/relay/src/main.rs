use dashmap::DashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

const MAX_HELLO_LEN: usize = 64;
const PAIR_TIMEOUT_SECS: u64 = 300;

type PairMap = Arc<DashMap<String, oneshot::Sender<TcpStream>>>;

fn mask_ip(addr: std::net::SocketAddr) -> String {
    match addr.ip() {
        std::net::IpAddr::V4(v4) => {
            let o = v4.octets();
            format!("{}.{}.x.{}", o[0], o[1], o[3])
        }
        std::net::IpAddr::V6(v6) => {
            let s = v6.segments();
            format!("{:x}:{:x}:x:x:x:x:x:x", s[0], s[1])
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8443);

    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    info!("Relay listening on 0.0.0.0:{}", port);

    let pairs: PairMap = Arc::new(DashMap::new());

    loop {
        let (stream, addr) = listener.accept().await?;
        let pairs = Arc::clone(&pairs);
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, addr, pairs).await {
                warn!("Client error from {}: {}", mask_ip(addr), error);
            }
        });
    }
}

async fn handle_client(
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
    pairs: PairMap,
) -> anyhow::Result<()> {
    let mut line = String::new();
    let n = {
        let mut reader = BufReader::new(&mut stream);
        timeout(Duration::from_secs(10), reader.read_line(&mut line)).await??
    };
    if n == 0 || line.len() > MAX_HELLO_LEN {
        return Ok(());
    }

    let line = line.trim();
    let code = if let Some(rest) = line.strip_prefix("HELLO ") {
        rest.to_string()
    } else {
        warn!("Invalid greeting from {}", mask_ip(addr));
        return Ok(());
    };

    let code_prefix = if code.len() >= 2 { &code[..2] } else { &code };
    info!("HELLO from {} code {}...", mask_ip(addr), code_prefix);

    if let Some((_, sender)) = pairs.remove(&code) {
        info!("Pairing code {}...", code_prefix);
        if sender.send(stream).is_err() {
            warn!("Peer for code {}... already gone", code_prefix);
        }
    } else {
        let (tx, rx) = oneshot::channel::<TcpStream>();
        pairs.insert(code.clone(), tx);

        match timeout(Duration::from_secs(PAIR_TIMEOUT_SECS), rx).await {
            Ok(Ok(mut peer_stream)) => {
                info!("Connected pair for code {}...", code_prefix);
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut peer_stream).await;
            }
            _ => {
                pairs.remove(&code);
                info!("Timeout waiting for pair for code {}...", code_prefix);
            }
        }
    }

    Ok(())
}
