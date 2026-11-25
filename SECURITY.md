# Security Policy

## Security Guarantees

Agent Restore is designed as a **100% offline, local-first** desktop tool:

- **Zero network activity**: The application makes no outbound HTTP/WebSocket connections, loads no remote assets, and enforces a strict Content Security Policy (CSP) blocking external origins.
- **Filesystem confinement**: Every backup and restore operation validates canonical paths inside registered project directories, rejects `..` traversal, absolute paths, drive prefixes, and null bytes, and refuses to read or write through symbolic links.
- **`.git` immutability**: Any path containing a `.git` segment is hard-blocked at both the ignore layer and the path validation layer. Optional Git metadata queries run strictly read-only with `GIT_OPTIONAL_LOCKS=0`.
- **Default secret exclusion**: `.env`, `.env.*` (except safe templates `.env.example`, `.env.sample`, `.env.template`), private keys (`*.pem`, `*.key`, `id_rsa`, `id_ed25519`), keystores (`*.p12`, `*.pfx`, `*.jks`, `*.kdbx`), and credential files (`*credentials*.json`, `secrets.yml`, `.npmrc`, `.netrc`) are excluded by immutable security rules.
- **Cryptographic integrity**: All stored file objects are content-addressed and verified via **BLAKE3** cryptographic hashes before any restore writes a single byte to disk.

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 1.0.x   | Yes       |

## Reporting a Vulnerability

If you discover a security vulnerability (such as path traversal, symlink bypass, unintended file modification, or secret leakage):

1. **Do not** open a public GitHub issue with exploit details.
2. Use GitHub's **Private Vulnerability Reporting** ("Security" tab -> "Report a vulnerability") or contact the maintainers directly.
3. Include reproduction steps, affected OS/version, and minimal proof-of-concept using an isolated temporary directory.

We aim to acknowledge reports within 48 hours and provide a fix or mitigation plan promptly.
