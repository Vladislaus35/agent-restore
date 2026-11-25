# Third-Party Notices

Agent Restore is licensed under the **MIT License**. It incorporates the following open-source libraries, each verified for compatibility with MIT redistribution.

## Rust Crates (Direct Dependencies)

| Crate | License | Purpose |
| ----- | ------- | ------- |
| `blake3` | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | Cryptographic hashing for content-addressed storage and integrity verification |
| `chrono` | MIT OR Apache-2.0 | ISO-8601 / RFC-3339 timestamps and retention calculations |
| `fs4` | MIT OR Apache-2.0 | Cross-platform available disk space checks |
| `globset` | Unlicense OR MIT | High-performance glob pattern matching for exclusion rules |
| `notify` | CC0-1.0 | Cross-platform native filesystem event monitoring |
| `rusqlite` (bundled SQLite) | MIT (SQLite: Public Domain) | Embedded transactional local metadata database |
| `serde` / `serde_json` | MIT OR Apache-2.0 | Data serialization between Rust core, SQLite, and Tauri IPC |
| `similar` | Apache-2.0 | Line-level code diff computation |
| `tauri` / `tauri-build` | MIT OR Apache-2.0 | Lightweight native desktop framework and system tray |
| `tauri-plugin-dialog` | MIT OR Apache-2.0 | Native OS folder picker dialog |
| `tauri-plugin-opener` | MIT OR Apache-2.0 | Safe OS path opening capability |
| `thiserror` | MIT OR Apache-2.0 | Typed error definitions |
| `uuid` | Apache-2.0 OR MIT | Unique identifiers for projects, checkpoints, and atomic temp files |
| `walkdir` | Unlicense OR MIT | Safe recursive directory traversal without following symlinks |
| `zstd` (zstd-sys) | MIT (libzstd: BSD-3-Clause) | Fast lossless compression for stored file objects |
| `tempfile` (dev only) | MIT OR Apache-2.0 | Isolated temporary directories for automated testing and benchmarks |

## Frontend Packages (Direct Dependencies)

| Package | License | Purpose |
| ------- | ------- | ------- |
| `react` / `react-dom` | MIT | User interface rendering |
| `lucide-react` | ISC | Bundled minimalist SVG icons (offline, no CDN) |
| `@tauri-apps/api` | MIT OR Apache-2.0 | Desktop IPC bridge |
| `@tauri-apps/plugin-dialog` | MIT OR Apache-2.0 | Native folder picker bindings |
| `@tauri-apps/plugin-opener` | MIT OR Apache-2.0 | Desktop opener bindings |
| `typescript` (dev) | Apache-2.0 | Static type checking |
| `vite` / `@vitejs/plugin-react` (dev) | MIT | Frontend build bundler |
| `vitest` / `jsdom` (dev) | MIT | Frontend unit testing |
| `@tauri-apps/cli` (dev) | MIT OR Apache-2.0 | Desktop application packaging and bundling |

## Offline Assets

All UI fonts use native OS system font stacks (`Segoe UI`, `Inter`, `-apple-system`, `Cascadia Code`, `Consolas`, monospace) and all icons are statically bundled via `lucide-react`. No external CDNs or third-party web services are contacted at runtime.
