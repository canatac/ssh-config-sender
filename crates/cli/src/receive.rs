use crate::framing::{recv_frame, send_frame};
use crate::transport::{connect_relay, handshake_responder};
use anyhow::Result;
use ssh_migrator_protocol::{code_to_psk, random_code, zeroize_psk, Control, MAX_FRAME_SIZE};
use std::collections::HashMap;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

struct RateLimit {
    attempts: HashMap<IpAddr, (u32, Instant)>,
}

impl RateLimit {
    fn new() -> Self {
        Self {
            attempts: HashMap::new(),
        }
    }

    fn check_and_record(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let entry = self.attempts.entry(ip).or_insert((0, now));
        if now.duration_since(entry.1) >= Duration::from_secs(60) {
            *entry = (0, now);
        }
        if entry.0 >= 3 {
            return false;
        }
        entry.0 += 1;
        true
    }
}

fn detect_local_ip() -> String {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok();
    if let Some(socket) = socket {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

pub async fn run_receive(port: u16, to: PathBuf, relay: Option<String>) -> Result<()> {
    let code = random_code();

    if let Some(relay_addr) = relay {
        print_relay_banner(&code, &relay_addr);
        let mut stream = connect_relay(&relay_addr, &code).await?;
        handle_stream(&mut stream, &to, &code, relay_label(&relay_addr)).await?;
        return Ok(());
    }

    let local_ip = detect_local_ip();
    print_direct_banner(&code, &local_ip, port);

    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    info!("Listening on 0.0.0.0:{}", port);

    let mut rate_limit = RateLimit::new();

    loop {
        let (mut stream, peer_addr) = listener.accept().await?;
        let peer_ip = peer_addr.ip();

        if !rate_limit.check_and_record(peer_ip) {
            warn!("Rate limit exceeded for {}", mask_ip(peer_ip));
            drop(stream);
            continue;
        }

        if let Err(error) = handle_stream(&mut stream, &to, &code, mask_ip(peer_ip)).await {
            warn!("Receive session failed for {}: {}", mask_ip(peer_ip), error);
            continue;
        }

        return Ok(());
    }
}

async fn handle_stream(
    stream: &mut TcpStream,
    to: &PathBuf,
    code: &str,
    source_label: String,
) -> Result<()> {
    let mut psk = code_to_psk(code);
    let mut session = match handshake_responder(stream, psk).await {
        Ok(session) => session,
        Err(error) => {
            warn!(
                "Handshake failed from {}: incorrect pairing code",
                source_label
            );
            return Err(error);
        }
    };
    psk = code_to_psk(code);
    zeroize_psk(&mut psk);

    let code_prefix = &code[..2];
    print!(
        "
▶ {} authenticated with code {}... Receive SSH config? [y/N] ",
        source_label, code_prefix
    );
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let accepted = input.trim().eq_ignore_ascii_case("y");

    if !accepted {
        let reject_json = serde_json::to_vec(&Control::Reject)?;
        let encrypted = session.encrypt(&reject_json)?;
        send_frame(stream, &encrypted).await?;
        info!("Transfer rejected by user.");
        return Ok(());
    }

    let accept_json = serde_json::to_vec(&Control::Accept)?;
    let encrypted = session.encrypt(&accept_json)?;
    send_frame(stream, &encrypted).await?;

    let frame = recv_frame(stream).await?;
    let plain = session.decrypt(&frame)?;
    let ctrl: Control = serde_json::from_slice(&plain)?;
    let (file_count, bytes) = match ctrl {
        Control::Meta { file_count, bytes } => (file_count, bytes),
        _ => anyhow::bail!("Expected Meta message"),
    };
    if bytes as usize > MAX_FRAME_SIZE {
        anyhow::bail!("Payload too large: {} bytes (max 64 MiB)", bytes);
    }
    info!("Receiving {} files, {} bytes...", file_count, bytes);

    let mut data_buf = Vec::with_capacity(bytes as usize);
    while data_buf.len() < bytes as usize {
        let frame = recv_frame(stream).await?;
        let plain = session.decrypt(&frame)?;
        data_buf.extend_from_slice(&plain);
        if data_buf.len() > bytes as usize {
            anyhow::bail!("Received more data than declared");
        }
    }

    let frame = recv_frame(stream).await?;
    let plain = session.decrypt(&frame)?;
    let ctrl: Control = serde_json::from_slice(&plain)?;
    match ctrl {
        Control::Done => {}
        _ => anyhow::bail!("Expected Done message"),
    }

    let count = crate::unpack::unpack(&data_buf, to)?;
    info!("✓ Restored {} files to {:?}", count, to);

    let ok_json = serde_json::to_vec(&Control::Ok)?;
    let encrypted = session.encrypt(&ok_json)?;
    send_frame(stream, &encrypted).await?;

    println!(
        "✓ SSH config restored successfully! {} files written to {:?}",
        count, to
    );
    Ok(())
}

fn print_direct_banner(code: &str, local_ip: &str, port: u16) {
    println!("SSH Migrator - RECEIVE MODE");
    println!("Pairing code: {}", code);
    println!("Local IP: {}", local_ip);
    println!("Run on source machine:");
    println!(
        "  sshmigrate send --to {}:{} --code {}",
        local_ip, port, code
    );
}

fn print_relay_banner(code: &str, relay: &str) {
    println!("SSH Migrator - RELAY RECEIVE MODE");
    println!("Pairing code: {}", code);
    println!("Relay: {}", relay);
    println!("Run on source machine:");
    println!(
        "  sshmigrate send --relay {} --to {} --code {}",
        relay, relay, code
    );
}

fn relay_label(relay: &str) -> String {
    format!("relay:{}", relay)
}

fn mask_ip(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            format!("{}.{}.x.x", octets[0], octets[1])
        }
        IpAddr::V6(v6) => {
            let segs = v6.segments();
            format!("{:x}:{:x}:x:x:x:x:x:x", segs[0], segs[1])
        }
    }
}
