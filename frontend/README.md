# Desktop interface

Run the real HTTP download manager with `cargo run --release --manifest-path frontend/Cargo.toml` from the repository root. See the [root README](../README.md) for features and limitations.

The UI communicates with the backend through the `DownloadEngine` trait. `RealEngine` adapts the Tokio-based backend; `SimulatedEngine` remains available for UI tests and development.
