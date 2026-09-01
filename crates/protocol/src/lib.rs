use hkdf::Hkdf;
use rand::Rng;
use sha2::Sha256;
use zeroize::Zeroize;

pub const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024; // 64 MiB
pub const CHUNK_SIZE: usize = 60 * 1024; // 60 KiB
pub const NOISE_PATTERN: &str = "Noise_NNpsk2_25519_ChaChaPoly_BLAKE2s";
pub const PSK_INFO: &[u8] = b"ssh-migrator-psk/v1";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "t")]
pub enum Control {
    Accept,
    Reject,
    Meta { file_count: u64, bytes: u64 },
    Done,
    Ok,
}

/// Generate a random 6-digit pairing code
pub fn random_code() -> String {
    let n: u32 = rand::thread_rng().gen_range(0..1_000_000);
    format!("{:06}", n)
}

/// Derive 32-byte PSK from pairing code using HKDF-SHA256
pub fn code_to_psk(code: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, code.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(PSK_INFO, &mut okm).expect("HKDF expand failed");
    okm
}

pub fn zeroize_psk(psk: &mut [u8; 32]) {
    psk.zeroize();
}
