use crate::crypto::NOISE_PATTERN;
use crate::framing::{recv_frame, send_frame};
use anyhow::Result;
use snow::Builder;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use zeroize::Zeroize;

pub struct Session {
    transport: snow::TransportState,
}

impl Session {
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; plaintext.len() + 16];
        let len = self.transport.write_message(plaintext, &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; ciphertext.len()];
        let len = self.transport.read_message(ciphertext, &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }
}

pub async fn connect_direct(target: &str) -> Result<TcpStream> {
    Ok(TcpStream::connect(target).await?)
}

pub async fn connect_relay(relay: &str, code: &str) -> Result<TcpStream> {
    let mut stream = TcpStream::connect(relay).await?;
    stream
        .write_all(
            format!(
                "HELLO {code}
"
            )
            .as_bytes(),
        )
        .await?;
    stream.flush().await?;
    Ok(stream)
}

pub async fn handshake_initiator<S>(stream: &mut S, mut psk: [u8; 32]) -> Result<Session>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut handshake = Builder::new(NOISE_PATTERN.parse()?)
        .psk(2, &psk)
        .build_initiator()?;
    psk.zeroize();

    let mut buf = vec![0u8; 65535];

    // Noise_NNpsk2 is a 2-message pattern: -> e, <- e, ee, psk
    let len = handshake.write_message(&[], &mut buf)?;
    send_frame(stream, &buf[..len]).await?;

    let msg2 = recv_frame(stream).await?;
    handshake.read_message(&msg2, &mut buf)?;

    let transport = handshake.into_transport_mode()?;
    Ok(Session { transport })
}

pub async fn handshake_responder<S>(stream: &mut S, mut psk: [u8; 32]) -> Result<Session>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut handshake = Builder::new(NOISE_PATTERN.parse()?)
        .psk(2, &psk)
        .build_responder()?;
    psk.zeroize();

    let mut buf = vec![0u8; 65535];

    let msg1 = recv_frame(stream).await?;
    handshake.read_message(&msg1, &mut buf)?;

    let len = handshake.write_message(&[], &mut buf)?;
    send_frame(stream, &buf[..len]).await?;

    let transport = handshake.into_transport_mode()?;
    Ok(Session { transport })
}
