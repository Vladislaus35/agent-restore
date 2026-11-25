# Contributing to Agent Restore

Thank you for your interest in contributing to **Agent Restore**! We welcome bug reports, documentation improvements, translations, performance benchmarks, and pull requests.

## Core Design Principles

Before submitting changes, please ensure your contribution aligns with these non-negotiable rules:

1. **Data Safety First**: Never silently overwrite or delete user files. Destructive operations must require explicit confirmation and create an automatic pre-restore checkpoint.
2. **100% Offline & Private**: Never introduce telemetry, analytics, external font/CDN fetches, API keys, mandatory accounts, or background network calls.
3. **Never Modify `.git`**: Git integration must remain strictly read-only (`GIT_OPTIONAL_LOCKS=0`).
4. **Provider Independence**: Do not tie core functionality to any specific AI coding tool, editor, or proprietary API.
5. **License Hygiene**: Only add dependencies with permissive, MIT-compatible licenses (e.g., MIT, Apache-2.0, BSD-2/3-Clause, ISC, CC0, Unlicense) and update `THIRD_PARTY_NOTICES.md`.

## Development Setup

### Prerequisites

- **Rust** (stable toolchain, 1.78+)
- **Node.js** (20 LTS or 22+) and **npm**
- Platform Tauri v2 prerequisites:
  - **Windows**: Visual Studio Build Tools (Desktop development with C++) and Microsoft Edge WebView2
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`

### Common Commands

```bash
# Install frontend dependencies
npm install

# Run Rust unit and realistic scenario integration tests
cargo test --workspace

# Run core engine performance benchmark
cargo run --release -p agent-restore-core --example benchmark

# Type-check and test frontend
npm run typecheck
npm test

# Start desktop app in development mode
npm run tauri dev

# Build production desktop binary & installer
npm run tauri build
```

## Pull Request Checklist

- [ ] Code compiles cleanly (`cargo check --workspace`, `npm run typecheck`).
- [ ] All Rust and frontend tests pass (`cargo test --workspace`, `npm test`).
- [ ] Any UI text additions include both English (`en`) and Turkish (`tr`) translations in `src/i18n/translations.ts`.
- [ ] New third-party dependencies are documented in `THIRD_PARTY_NOTICES.md`.
- [ ] Tests use isolated `TempDir` directories and never modify real user directories.
- [ ] No network calls or telemetry have been introduced.

## Code of Conduct

Be respectful, constructive, and collaborative in all issues, pull requests, and discussions.
