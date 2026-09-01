# SSH Migrator

Migrate your entire `~/.ssh` directory between machines with **end-to-end encryption**. No server ever sees your private keys in plaintext.

## How it works

- **P2P direct** (LAN / port-forward): target listens, source connects directly.
- **Relay NAT fallback**: both machines connect *outbound* to a relay server and are paired by a one-time code. The relay is stateless and only sees encrypted ciphertext.

Encryption: `Noise_NNpsk2_25519_ChaChaPoly_BLAKE2s` — forward secrecy guaranteed.

## Quick start

### Direct LAN migration

On the **new machine** (receiver):
```bash
sshmigrate receive
```

On the **old machine** (sender) — use the code and IP shown above:
```bash
sshmigrate send --to 192.168.1.42:8444 --code 482915
```

Answer `y` on the receiver when prompted.

### NAT relay migration

On the **new machine**:
```bash
sshmigrate receive --relay relay.exemple.fr:8443
```

On the **old machine**:
```bash
sshmigrate send --relay relay.exemple.fr:8443 --to relay.exemple.fr:8443 --code 482915
```

## Build

```bash
cargo build --release --package ssh-migrator-cli
```

The `sshmigrate` binary will be at `target/release/sshmigrate`.

## License

Apache-2.0 — open source, free to use.
