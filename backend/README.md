# Download manager engine

A Tokio-based HTTP/HTTPS GET engine for a desktop frontend. Construct a manager
inside a running Tokio runtime, enqueue jobs and retain their handles. For example:

```rust,no_run
use download_manager::{DownloadManager, DownloadRequest};
use std::path::PathBuf;
use url::Url;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let manager = DownloadManager::new(3)?;
let job = manager.enqueue(DownloadRequest {
    url: Url::parse("https://example.com/file.zip")?,
    destination: PathBuf::from("downloads/file.zip"),
});
// An egui app can poll job.snapshot() each frame, or subscribe to changes.
let result = job.wait().await;
println!("{:?}: {} bytes", result.status, result.downloaded_bytes);
# Ok(())
# }
```

`cancel()` works both while queued and during a transfer. `subscribe()` returns a
Tokio watch receiver; updates can be coalesced, so the latest snapshot is the
source of truth. Keep the Tokio runtime alive for the duration of the jobs. A
handle or manager can be dropped without cancelling a job. Destinations must
have an existing parent directory. Data is written to a temporary file in that
directory and published without replacing an existing destination; failures and
cancellation remove the temporary file during normal operation. A crash may
leave temporary files. Progress counts bytes written and the total is optional.
Cancellation racing with final publication may complete instead.

This first engine performs one request per download. It does not provide resume,
segmented downloads, retries, persistence, scheduling, or bandwidth limits.
