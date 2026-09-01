# Security Policy

## Trust model

- The **relay server** is untrusted by design. It only forwards encrypted bytes and cannot decrypt or MITM traffic.
- The **pairing code** acts as a pre-shared key (PSK) in the Noise NNpsk2 handshake. Without the code, the handshake fails cryptographically.
- A **human confirmation** prompt (showing source IP and code prefix) is required before any data is received.

## Authentication

Two factors required:
1. Correct pairing code (PSK — handshake fails otherwise).
2. Human `y/N` confirmation on the receiver with the source IP displayed.

## Vulnerability reporting

Please **do not** open a public GitHub issue for security vulnerabilities.

Send a PGP-encrypted email to the maintainer. We aim to respond within **72 hours** and will coordinate a fix before public disclosure.

## Limitations

- **Malware on source**: if the sending machine is compromised, the attacker may send arbitrary content. This tool does not protect against a compromised source.
- **Relay DoS**: a malicious actor can fill the relay's pairing table with garbage sessions. There is no data leakage, but availability may be affected.
- **Code entropy**: the 6-digit code has ~20 bits of entropy. Rate limiting (3 attempts/IP/60 s) mitigates brute-force.
