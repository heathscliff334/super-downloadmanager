# Super Download Manager

A native Rust desktop download manager for Windows, macOS, and Linux, built with egui and Tokio. The current MVP downloads HTTP and HTTPS files, shows transfer progress, queues up to three simultaneous transfers, retries failed transfers, and removes jobs from the list (cancelling active jobs). Completed files are never overwritten.

## Download

Download the latest release for your computer:

- **Windows x64:** [Installer](https://github.com/heathscliff334/super-downloadmanager/releases/latest/download/Super-Download-Manager-Setup.exe) or [standalone executable](https://github.com/heathscliff334/super-downloadmanager/releases/latest/download/Super-Download-Manager-Windows-x64.exe).
- **macOS Apple Silicon:** [Download the DMG](https://github.com/heathscliff334/super-downloadmanager/releases/latest/download/Super-Download-Manager-macOS-arm64.dmg), open it, and drag the app to Applications.

**macOS release note:** The v0.1.0 DMG has an invalid app-bundle signature and may be reported as damaged. A corrected, notarized public build is pending.

See the [release page](https://github.com/heathscliff334/super-downloadmanager/releases/latest) for release notes and checksums. The Windows binaries are unsigned, and the macOS app is unsigned and not notarized, so the operating system may show a warning. Intel Mac packages are not yet available.

## Build and run from source

Install a current Rust toolchain, then run:

```sh
cargo run --release --manifest-path frontend/Cargo.toml
```

Choose **Add URL**, paste one or more links, and enter an existing destination folder. Files use a name derived from the URL. The application keeps its queue in memory; closing it cancels active downloads. Pause/resume, transfer limits, persistent history, segmented downloads, browser integration, and FTP are not yet implemented.

## Build and run locally on macOS

1. Install the Xcode Command Line Tools if they are not already installed:

   ```sh
   xcode-select --install
   ```

2. Install a current Rust toolchain using [rustup](https://rustup.rs/), then open a new Terminal window so `cargo` is on your `PATH`.

3. Clone the repository and enter the project directory:

   ```sh
   git clone https://github.com/heathscliff334/super-downloadmanager.git
   cd super-downloadmanager
   ```

4. Run the app in development mode:

   ```sh
   cargo run --locked --manifest-path frontend/Cargo.toml
   ```

   For an optimized build, run:

   ```sh
   cargo build --locked --release --manifest-path frontend/Cargo.toml
   ./frontend/target/release/download-manager-ui
   ```

   The standalone executable is created at `frontend/target/release/download-manager-ui`.

5. On an Apple Silicon Mac, optionally package the release build as a DMG:

   ```sh
   packaging/macos/build-dmg.sh
   ```

   The disk image is written to `packaging/macos/output/Super-Download-Manager-macOS-arm64.dmg`. By default, it is ad-hoc signed for local testing and is not notarized; macOS may block it when downloaded or shared. See [macOS signing and notarization](#macos-signing-and-notarization) for a distributable build.

## Linux x86-64

Build and package on Linux x86-64 (Ubuntu 22.04 or a compatible system):

```sh
cargo build --release --locked --manifest-path frontend/Cargo.toml
sh packaging/linux/build-tarball.sh
```

The archive is written to `packaging/linux/output/`. Extract it and run `./install.sh` for a per-user installation, or run `./bin/super-download-manager` directly. See [Linux installation notes](packaging/linux/INSTALL.txt) for dependencies and uninstall instructions. A Linux release download is not yet published.

## Development

```sh
cargo test --manifest-path backend/Cargo.toml
cargo test --manifest-path frontend/Cargo.toml
```

See the [product plan](docs/plan.md) for background and proposed work. The plan describes future goals, not the current feature set.

## License

[MIT](LICENSE).

## macOS signing and notarization

The macOS packaging script seals the complete app bundle. By default it uses an ad-hoc signature for local testing; this is not a trusted signature for distributing downloads. To produce a trusted public build, configure a **Developer ID Application** signing identity and an Apple notarization keychain profile, then run:

```sh
MACOS_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
MACOS_NOTARY_PROFILE="your-notary-profile" \
  packaging/macos/build-dmg.sh
```

The script signs the app and disk image, submits the image to Apple, and staples the notarization ticket. Build the release binary before running it. See [Apple’s notarization documentation](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
