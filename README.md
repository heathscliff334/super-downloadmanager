# Super Download Manager

A native Rust desktop download manager built with egui and Tokio. The current MVP downloads HTTP and HTTPS files, shows transfer progress, queues up to three simultaneous transfers, retries failed transfers, and removes jobs from the list (cancelling active jobs). Completed files are never overwritten.

## Download

Download the latest release for your computer:

- **Windows x64:** [Installer](https://github.com/heathscliff334/super-downloadmanager/releases/latest/download/Super-Download-Manager-Setup.exe) or [standalone executable](https://github.com/heathscliff334/super-downloadmanager/releases/latest/download/Super-Download-Manager-Windows-x64.exe).
- **macOS Apple Silicon:** [Download the DMG](https://github.com/heathscliff334/super-downloadmanager/releases/latest/download/Super-Download-Manager-macOS-arm64.dmg), open it, and drag the app to Applications.

See the [release page](https://github.com/heathscliff334/super-downloadmanager/releases/latest) for release notes and checksums. The Windows binaries are unsigned, and the macOS app is unsigned and not notarized, so the operating system may show a warning. Intel Mac packages are not yet available.

## Build and run from source

Install a current Rust toolchain, then run:

```sh
cargo run --release --manifest-path frontend/Cargo.toml
```

Choose **Add URL**, paste one or more links, and enter an existing destination folder. Files use a name derived from the URL. The application keeps its queue in memory; closing it cancels active downloads. Pause/resume, transfer limits, persistent history, segmented downloads, browser integration, and FTP are not yet implemented.

## Development

```sh
cargo test --manifest-path backend/Cargo.toml
cargo test --manifest-path frontend/Cargo.toml
```

See the [product plan](docs/plan.md) for background and proposed work. The plan describes future goals, not the current feature set.

## License

[MIT](LICENSE).
