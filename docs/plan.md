# Planning note

This document describes the initial proposed plan. The repository now contains an HTTP engine and a desktop UI; see the root README for the current implementation and limitations.

# Super Download Manager — product and implementation plan

Status: plan only. Written 2026-11-14. Working name "Super Download Manager" (SDM), from the
repository name.

## 0. Status at a glance

| State | What |
|---|---|
| **Implemented in this repository** | Nothing. The repository holds one commit ("Initialize repository") and this plan. No code, no tests, no build. |
| **MVP (sections 3–8)** | HTTP/HTTPS downloads with pause, resume, crash-safe state, segmented connections, a queue, a single-window desktop UI, an in-process test server. Planned, not built. |
| **Future (section 10)** | Browser capture, torrents, video pages, scheduler, FTP, CLI, automation API, themes, translations, installers. Not planned in detail; listed so the MVP does not block them. |

Every statement below about SDM is a plan. Every statement about the reference project is
what its public repository says, read at the commits in section 1; none of it was run or
measured here.

## 1. What the references are, and what we take from them

### 1.1 misterdownloadmanager (MDM)

`anefzaoui/misterdownloadmanager` at `7418844`, version 0.5.2, **GPL-3.0-or-later**.

Observed from its README, `PRODUCT.md`, `CLAUDE.md`, `docs/ARCHITECTURE.md`, the user guide in
`docs/`, the workspace `Cargo.toml` and the directory layout. Its Rust source files were not read
for this plan.

- A Rust workspace of 14 crates. The engine crate (HTTP/FTP downloads) knows nothing of the UI;
  the app sees it only through events (progress, segment table snapshots, finished, failed).
- UI: egui 0.37 / eframe, one window "painted to fixed pixel coordinates", laid out to match
  Internet Download Manager: toolbar, category tree, download list, one progress window per
  download.
- Engine pipeline as documented: probe (size, resumable, ETag / Last-Modified, file name) →
  pure segment planner (a free connection takes half of the largest remaining part) → N
  workers writing at offsets into a sparse `.part` file → per-host connection limits → shared
  token-bucket limiter → segment table saved atomically → rename into place when the size
  checks out.
- Persistence: one `downloads.json` in the platform data folder, written atomically.
- Tests: `cargo test` runs end-to-end downloads against its own lab server crate with injected
  faults (slow, dropped, refused connections, no-range servers, changing files).
- Around the core: browser extension + native messaging host, torrents (librqbit), yt-dlp /
  gallery-dl wrappers, debrid, archive extraction, remote control, MCP server, automation API
  with a JSON schema, 86 UI languages, themes, crash reporter, installers.

What we take: the **shape**, not the content.

| Take | Leave |
|---|---|
| Engine isolated from UI, talking through commands and events | The IDM-clone layout and positioning ("IDM's window, but free") |
| Probe → plan → workers → atomic state → rename | Fixed-pixel custom-painted chrome and the nine style themes |
| A test server with fault injection as the main correctness tool | Everything in "around the core" until the core is proven |
| Honest docs: say what does not work | Its code, text, icons, screenshots, translations (GPL; see 1.2) |

### 1.2 Independence rules

1. No code, text, artwork, icons, screenshots or translation catalogs from MDM, IDM or any other
   download manager enter this repository. MDM is GPL-3.0; copying from it would bind SDM to
   GPL-3.0.
2. Behaviour is learned from public documentation, published protocol specifications (RFC 9110
   for HTTP semantics and range requests, RFC 6266 for `Content-Disposition`) and black-box use.
3. Implementers working on the engine do not read MDM's `crates/engine` source. Ideas stated in
   its public architecture document (for example "split the largest remaining part") are
   general techniques and may be reimplemented from the description.
4. SDM's own licence is an open decision (section 11). Recommendation: MIT OR Apache-2.0.

## 2. Product

**Who:** someone who downloads large files on a desktop and wants them to finish: faster than
the browser where the server allows it, and without starting over when the network or the
machine fails.

**Promise:** paste a link, get the file. Pause, quit, crash, lose Wi-Fi: the download continues
from where it stopped.

**How SDM differs from MDM:** not an IDM look-alike. One modern window, no per-download popup
windows, system light/dark, keyboard-first. Small scope done reliably before any breadth.

**Non-goals for the MVP:** browser integration, torrents, video-site extraction, accounts,
telemetry, auto-update, localisation beyond English.

## 3. MVP behaviour

Each item is an acceptance criterion: it is done when the test named in section 8 passes.

### 3.1 Adding

- Input: one `http://` or `https://` URL, pasted or typed. Any other scheme is refused with a
  message naming the scheme.
- Destination: a default folder (the platform Downloads folder), changeable per download and in
  settings.
- File name, in order: `Content-Disposition` filename (RFC 6266, including `filename*`), else
  the last URL path segment percent-decoded, else `download`. Then sanitised: path separators
  and control characters removed, Windows reserved names (`CON`, `NUL`, …) and trailing dots or
  spaces fixed, length capped at 255 bytes. A name can never leave the destination folder.
- Collisions: an existing file is never overwritten. The new file becomes `name (1).ext`.
- Adding the same URL twice asks once: add again, or show the existing download.

### 3.2 Downloading

- Probe with a ranged `GET` (`Range: bytes=0-0`), not `HEAD`: many servers answer `HEAD`
  wrongly. Learn total size, whether ranges work (`206` + `Content-Range`), and the validator
  (strong `ETag`, else `Last-Modified`).
- Ranges supported and size known: up to **8** connections per download (setting, 1–16).
  A free connection takes the second half of the largest remaining segment, if that half is at
  least 1 MiB.
- Ranges not supported or size unknown: one connection, streamed to the end. The UI says
  "this server cannot resume".
- Data is written to `<name>.sdmpart` beside the final path, at the segment's offset. When all
  segments are complete and the size matches, the file is flushed and renamed to its final name.
- Redirects followed, at most 10. TLS through rustls with the platform's trust store.
- Queue: at most **3** downloads active at once (setting, 1–10). The rest wait in list order.

### 3.3 Pause, resume, failure

- Pause stops all connections and keeps the part file and segment table.
- Resume sends `If-Range` with the stored validator on every ranged request. A `200` instead of
  `206`, or a changed size, means the file changed: the download stops with **Remote file
  changed**, and offers **Restart from zero**. It is never silently restarted or stitched.
- A failed connection is retried with backoff (1 s, 2 s, 4 s … capped at 60 s, with jitter).
  A download fails after 5 consecutive failures with no bytes received in between. `4xx`
  responses other than 408 and 429 fail at once.
- Segment progress is saved at most once a second and on every pause, completion and clean
  exit. After a crash or power loss, at most about one second of each connection's data is
  downloaded again. State files are written to a temporary name, synced, then renamed.
- On start, downloads that were active are restored as **Paused** by default (setting:
  resume automatically).
- Disk full is its own error and pauses the whole queue, not just one download.

### 3.4 Removing

- **Remove from list** keeps the files. **Delete** also removes the part file or, after
  confirmation, the finished file. Removing an active download cancels it first.

### 3.5 Download states

```
            add
             │
             ▼
   ┌──── Queued ◄──────────── resume ────────────┐
   │        │ slot free                          │
 pause      ▼                                    │
   │    Connecting ── probe ok ──► Downloading ──┼── pause ──► Paused
   │        │                         │    │     │
   │        └──── error ──────────────┘    │     │
   │                  ▼                    ▼     │
   └────────────►  Failed ── retry ────────┘  Completed
```

`Failed` carries an error (section 6.4). `Completed` is terminal except for remove/delete.
Every state except `Completed` accepts remove.

## 4. UI

One window. No per-download windows, no tray icon in the MVP.

```
┌──────────────────────────────────────────────────────────────────────┐
│ [ Paste a link…                                         ] [ Add  ⏎ ] │
├───────────┬──────────────────────────────────────────────────────────┤
│ All    12 │ ubuntu-26.10-desktop-amd64.iso                           │
│ Active  2 │ ████████████░░░░░░░  62%  3.9 / 6.2 GB  18 MB/s  2 min ⏸ │
│ Queued  1 │──────────────────────────────────────────────────────────│
│ Done    8 │ dataset.tar.zst                    Remote file changed ↻ │
│ Failed  1 │──────────────────────────────────────────────────────────│
│           │ report.pdf                      2.1 MB  Done   📂  ✕     │
├───────────┴──────────────────────────────────────────────────────────┤
│ ▼ 21 MB/s   2 active · 1 queued                            Settings  │
└──────────────────────────────────────────────────────────────────────┘
```

- **Add bar**, always visible. `Ctrl/Cmd+V` anywhere in the window with a URL on the clipboard
  fills it. `Enter` adds with defaults; an expander shows destination and file name.
- **Sidebar filters** by state with counts.
- **List rows**: name, progress bar, percent, bytes, speed, time left, one primary action for
  the state (pause / resume / retry / open folder), and a context menu (copy link, show in
  folder, remove, delete). Selecting a row opens a **detail strip** under it: URL, destination,
  resumable yes/no, a segment bar showing each connection's range and progress, the last error.
- **Status bar**: total speed, counts, settings.
- **Settings**: default folder, connections per download, simultaneous downloads, resume on
  start, theme (system / light / dark).
- **Empty state** teaches the one action: "Paste a link to start."
- **Errors** are sentences with a next step, shown in the row, never a modal.

Rules: every action reachable by keyboard (`Space` pause/resume, `Delete` remove, arrows to
move, `Ctrl/Cmd+,` settings); every control has an accessible name (AccessKit); contrast at
least WCAG AA in both themes; speed and ETA smoothed over about 3 seconds so they do not
flicker; the list stays responsive with 10,000 rows (virtualised).

### Toolkit

**egui / eframe** (pure Rust, one binary, AccessKit built in, immediate mode suits a list that
changes every frame; MDM shows it carries a far larger app). Use the current stable release at
implementation time; the version is not pinned by this plan. Modern look comes from layout,
spacing and a small palette, not custom-painted chrome.

The UI depends only on the contract in section 6, so replacing the toolkit later (Tauri, Slint,
iced) does not touch the engine.

## 5. Architecture

```
crates/
  engine/      sdm-engine      library: everything in sections 3 and 6. No UI types.
  app/         sdm-app         binary: eframe window, view model, settings.
  testserver/  sdm-testserver  dev-only library: HTTP server with ranges and faults (section 8).
```

`app → engine`. `engine` and `app` both use `testserver` as a dev-dependency only.

Inside the engine:

| Module | Owns | Pure? |
|---|---|---|
| `naming` | file name from headers/URL, sanitising, collision suffix | yes |
| `planner` | segment table: split, claim, advance, complete | yes |
| `probe` | first request, validators, range support | no |
| `worker` | one connection: ranged GET, write at offset, report bytes | no |
| `store` | atomic load/save of list and segment tables | no |
| `download` | one download's state machine, its workers, retry | no |
| `manager` | the queue, slots, the public API | no |

Runtime: Tokio inside the engine, started and owned by it. HTTP: `reqwest` with rustls.
The UI thread never blocks on the engine and holds no async code.

Data on disk, under the platform data directory (`dirs`), overridable with `SDM_DATA_DIR` for
tests and portable use:

```
settings.json
downloads.json            the list: id, url, path, state, sizes, validator, error
segments/<id>.json        one download's segment table; deleted on completion
```

Each file carries `"version": 1`. An unreadable file is renamed to `*.corrupt` and reported; it
is never silently replaced with an empty one.

## 6. Engine contract (the UI ↔ backend boundary)

This is the single integration surface. The UI team builds against it with a fake; the engine
team implements it. Changes to it are changes to this section first.

All types are plain data, `Clone + Debug + Serialize + Deserialize`, so the same contract can
later cross a process boundary (CLI, browser host) unchanged.

### 6.1 Handle

```rust
pub struct Engine { /* cheap to clone */ }

impl Engine {
    /// Loads state from `config.data_dir`, starts the runtime. Events go to `events`;
    /// `wake` is called after each batch so the UI can request a repaint.
    pub fn start(config: Config, wake: impl Fn() + Send + Sync + 'static)
        -> Result<(Engine, std::sync::mpsc::Receiver<Event>), StartError>;

    /// Validates synchronously (URL, scheme, destination writable). Network work is async.
    pub fn add(&self, request: AddRequest) -> Result<DownloadId, AddError>;

    pub fn pause(&self, id: DownloadId);
    pub fn resume(&self, id: DownloadId);            // also: retry a Failed download
    pub fn restart(&self, id: DownloadId);           // discard partial data, start from zero
    pub fn remove(&self, id: DownloadId, files: RemoveFiles);
    pub fn reorder(&self, id: DownloadId, before: Option<DownloadId>);
    pub fn set_settings(&self, settings: Settings);

    /// Full current state. Called once at startup; afterwards the UI applies events.
    pub fn snapshot(&self) -> Snapshot;

    /// Stops connections, saves state, returns when the state is on disk.
    pub fn shutdown(self);
}
```

Commands other than `add` return nothing: their outcome arrives as events. A command for an
unknown id, or one that makes no sense in the current state (pause a completed download), is
ignored and logged; it is not an error the UI must handle.

### 6.2 Data

```rust
pub struct DownloadId(pub u64);          // unique for the life of the data dir, never reused

pub struct AddRequest {
    pub url: String,
    pub dir: Option<PathBuf>,            // None → Settings::default_dir
    pub file_name: Option<String>,       // None → derived (3.1); always sanitised
    pub start: bool,                     // false → added Paused
}

pub struct Download {
    pub id: DownloadId,
    pub url: String,                     // credentials in the URL are redacted here
    pub path: PathBuf,                   // final path; may change once at probe time
    pub state: State,
    pub total: Option<u64>,              // None until probed, or unknown size
    pub received: u64,
    pub resumable: Option<bool>,         // None until probed
    pub added_at: u64,                   // Unix seconds
    pub finished_at: Option<u64>,
}

pub enum State { Queued, Connecting, Downloading, Paused, Completed, Failed(Failure) }

pub struct Progress {                    // only while Connecting / Downloading
    pub id: DownloadId,
    pub received: u64,
    pub bytes_per_sec: u64,              // engine-smoothed
    pub eta_secs: Option<u64>,
    pub segments: Vec<Segment>,          // sorted by start, non-overlapping, cover 0..total
}
pub struct Segment { pub start: u64, pub end: u64, pub received: u64, pub active: bool }

pub struct Settings {
    pub default_dir: PathBuf,
    pub connections_per_download: u8,    // 1..=16, default 8
    pub max_active: u8,                  // 1..=10, default 3
    pub resume_on_start: bool,           // default false
}

pub struct Snapshot { pub downloads: Vec<Download>, pub settings: Settings } // list order
pub enum RemoveFiles { Keep, Delete }
```

### 6.3 Events

```rust
pub enum Event {
    Added(Download),
    Changed(Download),                   // any field except progress numbers
    Progress(Progress),                  // at most 4 per second per download
    Removed(DownloadId),
    Reordered(Vec<DownloadId>),
    SettingsChanged(Settings),
    Notice(Notice),                      // not tied to one download: disk full, state file corrupt
}
```

Guarantees the UI may rely on:

1. Events for one download arrive in order. `Added` comes before anything else for that id;
   nothing follows `Removed`.
2. A `Changed` to `Completed` is sent only after the file is at `path` under its final name.
3. `Progress.received` never decreases, except after `restart`.
4. `snapshot()` followed by applying every later event yields the engine's state.
5. The receiver is unbounded for state events; `Progress` events are coalesced, so a UI that
   stops reading for a while loses nothing but intermediate progress.

### 6.4 Errors

```rust
pub struct Failure { pub kind: FailureKind, pub detail: String, pub retryable: bool }

pub enum FailureKind {
    Network,              // DNS, connect, TLS, timeout, reset; after retries are exhausted
    HttpStatus(u16),      // final non-success status
    RemoteChanged,        // validator or size mismatch on resume → offer restart
    TooManyRedirects,
    DiskFull,
    Disk,                 // any other I/O error on the destination
    Internal,             // a bug; detail goes to the log
}

pub enum AddError { InvalidUrl, UnsupportedScheme(String), DestinationNotWritable(PathBuf) }
```

The UI maps `FailureKind` to a sentence and a next step; `detail` is shown only in the detail
strip. `retryable` decides whether the row's primary action is Retry.

### 6.5 Fake engine

`sdm-engine` ships `Engine::fake(script)` behind a `fake` feature: same handle, events driven by
a scripted timeline, no network or disk. The UI is developed and tested against it, and it is
the reference for guarantees 1–5.

## 7. Milestones

Each ends with something that runs and its tests green.

| # | Delivers | Proves |
|---|---|---|
| M0 | Workspace, CI (`fmt`, `clippy -D warnings`, `test`) on Linux, macOS, Windows; contract types; fake engine | The contract compiles and is usable |
| M1 | `testserver`; engine with one connection: add, probe, naming, download, complete, remove; persistence | A file arrives byte-identical |
| M2 | Pause, resume with `If-Range`, retry with backoff, crash-safe segment state, restore on start | Kill at any point, resume, byte-identical |
| M3 | Planner + multiple connections, queue with `max_active`, reorder | Segmented download is correct under faults |
| M4 | UI against the fake engine: add bar, list, filters, detail strip, settings | Every MVP action reachable by keyboard |
| M5 | UI on the real engine; error sentences; disk-full handling; manual test pass on three systems | MVP acceptance (section 3) |

M4 can run in parallel with M1–M3 once M0 is merged.

## 8. Testing

Order of preference: end-to-end through the public contract, then
module-boundary tests, then unit tests for what the first two cannot reach.

### 8.1 Test server (`sdm-testserver`)

An in-process HTTP server on `127.0.0.1:0`, serving deterministic pseudo-random bodies of a
given size and seed (so no fixture files and any size is cheap), with a request log and faults
set per path:

| Fault | Checks |
|---|---|
| `no-range` (always `200`) | single-connection fallback, "cannot resume" |
| `no-length` (chunked) | unknown-size path |
| `drop-after=N` bytes | retry, no duplicated or missing bytes |
| `stall=N` seconds | read timeout, retry |
| `status=503` for first K requests | backoff, eventual success |
| `status=404` | immediate failure, not retried |
| `change-after=K` requests (new body + ETag) | `RemoteChanged`, never a stitched file |
| `ignore-if-range` | size/validator check catches it |
| `max-connections=N` (extra get `503`) | engine lowers its connection count |
| `redirect=N` hops | follows 10, fails at 11 |
| `content-disposition=<raw>` | naming, traversal attempts |
| `rate=N` bytes/s | speed and ETA figures are sane |

### 8.2 End-to-end (through `Engine`, real disk in a temp dir)

Each asserts the final file's SHA-256 against the generated body, and the event sequence
against guarantees 1–5.

- Happy path at 0 bytes, 1 byte, 1 MiB − 1, 1 MiB, 100 MiB; 1 and 8 connections.
- Every fault in 8.1.
- Pause/resume at random points, 100 seeded iterations.
- **Crash resume:** run the engine in a child process, `SIGKILL` it at a random time, start a
  new engine on the same data dir, resume, compare hashes. 50 seeded iterations.
- Queue: 10 added, `max_active = 3`; the request log never shows more than 3 files at once.
- Collision: existing file untouched, new one suffixed.
- Disk full (a small tmpfs/quota where the CI system has one; otherwise an injected write
  error at the `store`/`worker` boundary).

### 8.3 Unit and property tests

- `planner`: property test with random claim/advance/fail/split sequences. Invariants:
  segments never overlap, always cover `0..total`, `received ≤ len`, completion ⇔ all bytes
  received exactly once.
- `naming`: table of hostile inputs (`../../etc/passwd`, `..\\x`, `CON.txt`, NUL bytes, 4 KB
  names, RTL override characters, `filename*=UTF-8''…`). Property: result is one path
  component, non-empty, inside the destination.
- `store`: truncated and garbage files load as "corrupt", not as empty; a version from the
  future is refused, not rewritten.
- Retry/backoff schedule with a paused Tokio clock.

### 8.4 UI

- View-model tests: feed scripted events from the fake engine, assert rows, counts, the primary
  action per state, and the sentence per `FailureKind`.
- One snapshot/interaction test per screen with the toolkit's test harness if it is stable at
  implementation time (egui has `egui_kittest`; unverified here), else skipped and listed.
- Manual checklist before M5 sign-off: keyboard-only pass, screen reader names the rows
  (VoiceOver, NVDA; Orca noted as known-unknown), light and dark, 200 % scaling, 10,000 rows.

### 8.5 CI

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on Linux, macOS
and Windows. Crash-resume and 100 MiB tests run on every push to main, and on demand for pull
requests. No test touches the public internet.

## 9. Security and safety

- Only `http` and `https`. Redirects to other schemes are refused.
- File names from the network are untrusted (3.1); tested with hostile inputs (8.3).
- Never overwrite an existing file; never write outside the chosen folder; the part file is
  created with `create_new`.
- Nothing downloaded is opened or run by SDM. "Open" hands the file to the system's default
  handler only on an explicit click; "Show in folder" is the row's default.
- Passwords in URLs are redacted in the UI, events and logs; kept only in `downloads.json`
  (keychain storage is future work).
- TLS verification cannot be turned off in the MVP.
- No telemetry, no update checks, no network traffic except the user's downloads.
- `unsafe_code = "forbid"` in all three crates.

## 10. Future work (not MVP, not designed)

In rough order of value. Each needs its own plan before it starts. Present in MDM unless marked.

1. Per-download and global speed limit (token bucket).
2. Command line (`sdm get <url>`) over the same engine contract.
3. Single-instance channel + browser extension and native messaging host (capture browser
   downloads). Largest piece; needs store listings and per-browser manifests.
4. "Refresh download address" for expired links (replace the URL, keep the data if the
   validator matches).
5. Checksums (verify a pasted SHA-256) and per-host connection limits learned from `503`/`429`.
6. Tray icon, completion notifications, start at login.
7. Batch add (many URLs, numbered patterns), categories by file type, scheduler.
8. Proxy settings, HTTP auth, cookies, secrets in the system keychain.
9. FTP/FTPS, torrents and magnet links, video pages via an external yt-dlp.
10. Translations, themes, installers, signing, auto-update, portable mode.
11. Automation API / MCP server / remote control.

The MVP keeps these possible by: serialisable contract types (2, 3, 11), `SDM_DATA_DIR`
(portable mode), `Settings` as one versioned struct (1, 8), and the engine owning all state (6).

## 11. Unknowns and open decisions

| Item | Kind | Recommendation |
|---|---|---|
| SDM's licence | Decision for the owner | MIT OR Apache-2.0 |
| Product name ("Super Download Manager" is the repo name only) | Decision for the owner | Check for trademark conflicts before any release |
| UI toolkit | Decision; this plan assumes egui | Revisit only if M4 shows egui cannot meet section 4 |
| Target systems for the first release | Decision | macOS and Windows first, Linux in CI |
| Default of 8 connections and 1 MiB minimum split | Inference, unmeasured | Measure against the test server's `rate` and `max-connections` faults in M3 before fixing |
| MDM's "5× faster than IDM" figures | Its claim, not verified here | Do not repeat; SDM makes no speed claims until it has its own benchmark |
| `egui_kittest` maturity, current egui / reqwest / Tokio versions | Not checked | Check at M0 |
| How MDM's engine handles any specific edge case | Unknown by design (source not read) | Derive from the RFCs and our own tests |
