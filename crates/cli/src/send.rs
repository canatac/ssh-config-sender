use crate::framing::{recv_frame, send_frame};
use crate::transport::{connect_direct, connect_relay, handshake_initiator};
use anyhow::{bail, Result};
use ssh_migrator_protocol::{code_to_psk, zeroize_psk, Control, CHUNK_SIZE, MAX_FRAME_SIZE};
use std::path::PathBuf;
use tracing::info;

pub async fn run_send(
    target: String,
    code: String,
    from: PathBuf,
    relay: Option<String>,
) -> Result<()> {
    let endpoint = relay.as_deref().unwrap_or(&target);
    info!("Connecting to {}", endpoint);
    let mut stream = if let Some(relay_addr) = relay.as_deref() {
        connect_relay(relay_addr, &code).await?
    } else {
        connect_direct(&target).await?
    };

    let mut psk = code_to_psk(&code);
    let mut session = match handshake_initiator(&mut stream, psk).await {
        Ok(s) => s,
        Err(e) => {
            bail!("Pairing code incorrect or target unreachable: {}", e);
        }
    };
    psk = code_to_psk(&code);
    zeroize_psk(&mut psk);

    let frame = recv_frame(&mut stream).await?;
    let plain = session.decrypt(&frame)?;
    let ctrl: Control = serde_json::from_slice(&plain)?;
    match ctrl {
        Control::Reject => {
            info!("Transfer rejected by receiver.");
            return Ok(());
        }
        Control::Accept => {}
        _ => bail!("Unexpected message"),
    }

    info!("Packing {:?}...", from);
    let packed = crate::pack::pack(&from)?;

    if packed.data.len() > MAX_FRAME_SIZE {
        bail!(
            "Payload too large: {} bytes (max 64 MiB)",
            packed.data.len()
        );
    }

    let meta = Control::Meta {
        file_count: packed.file_count,
        bytes: packed.data.len() as u64,
    };
    let meta_json = serde_json::to_vec(&meta)?;
    let encrypted = session.encrypt(&meta_json)?;
    send_frame(&mut stream, &encrypted).await?;

    for chunk in packed.data.chunks(CHUNK_SIZE) {
        let encrypted = session.encrypt(chunk)?;
        send_frame(&mut stream, &encrypted).await?;
    }

    let done_json = serde_json::to_vec(&Control::Done)?;
    let encrypted = session.encrypt(&done_json)?;
    send_frame(&mut stream, &encrypted).await?;

    let frame = recv_frame(&mut stream).await?;
    let plain = session.decrypt(&frame)?;
    let ctrl: Control = serde_json::from_slice(&plain)?;
    match ctrl {
        Control::Ok => {
            info!("✓ Transfer complete! {} files sent.", packed.file_count);
        }
        _ => bail!("Unexpected response from receiver"),
    }

    Ok(())
}
