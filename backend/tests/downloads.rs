use download_manager::{DownloadManager, DownloadRequest, DownloadStatus};
use std::path::Path;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
    time::{timeout, Duration},
};
use url::Url;

async fn server(response: &'static [u8]) -> Url {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0; 2048];
        let _ = socket.read(&mut buf).await;
        let _ = socket.write_all(response).await;
    });
    Url::parse(&format!("http://{address}/file")).unwrap()
}

fn request(url: Url, destination: &Path) -> DownloadRequest {
    DownloadRequest {
        url,
        destination: destination.to_path_buf(),
    }
}

async fn done(handle: &download_manager::DownloadHandle) -> download_manager::DownloadSnapshot {
    timeout(Duration::from_secs(5), handle.wait())
        .await
        .expect("job timed out")
}

#[tokio::test]
async fn downloads_and_reports_completion() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    let url =
        server(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello").await;
    let handle = DownloadManager::new(2)
        .unwrap()
        .enqueue(request(url, &target));
    let result = done(&handle).await;
    assert_eq!(result.status, DownloadStatus::Completed);
    assert_eq!((result.downloaded_bytes, result.total_bytes), (5, Some(5)));
    assert_eq!(std::fs::read(&target).unwrap(), b"hello");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn refuses_existing_destination_and_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    std::fs::write(&target, b"original").unwrap();
    let url = server(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew").await;
    let result = done(
        &DownloadManager::new(1)
            .unwrap()
            .enqueue(request(url, &target)),
    )
    .await;
    assert!(matches!(result.status, DownloadStatus::Failed(_)));
    assert_eq!(std::fs::read(target).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn rejects_errors_and_incomplete_responses() {
    for response in [
        &b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"[..],
        &b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\n\r\nabc"[..],
        &b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabc"[..],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("file");
        let url = server(response).await;
        let result = done(
            &DownloadManager::new(1)
                .unwrap()
                .enqueue(request(url, &target)),
        )
        .await;
        assert!(
            matches!(result.status, DownloadStatus::Failed(_)),
            "{:?}",
            result.status
        );
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[tokio::test]
async fn cancellation_cleans_partial_file_and_frees_queue() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, sent_rx) = oneshot::channel();
    let (release, release_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0; 2048];
        let _ = socket.read(&mut buf).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc")
            .await
            .unwrap();
        let _ = sent.send(());
        let _ = release_rx.await;
    });
    let dir = tempfile::tempdir().unwrap();
    let manager = DownloadManager::new(1).unwrap();
    let first = manager.enqueue(request(
        Url::parse(&format!("http://{address}/")).unwrap(),
        &dir.path().join("first"),
    ));
    sent_rx.await.unwrap();
    let mut progress = first.subscribe();
    timeout(Duration::from_secs(5), async {
        while progress.borrow_and_update().downloaded_bytes < 3 {
            progress.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    let second_url = server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
    let second = manager.enqueue(request(second_url, &dir.path().join("second")));
    assert_eq!(second.snapshot().status, DownloadStatus::Queued);
    let queued = manager.enqueue(request(
        Url::parse("http://127.0.0.1:9/").unwrap(),
        &dir.path().join("queued"),
    ));
    queued.cancel();
    assert_eq!(done(&queued).await.status, DownloadStatus::Cancelled);
    first.cancel();
    assert_eq!(done(&first).await.status, DownloadStatus::Cancelled);
    assert_eq!(done(&second).await.status, DownloadStatus::Completed);
    assert!(!dir.path().join("first").exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let _ = release.send(());
}

#[tokio::test]
async fn queued_cancellation_and_invalid_input() {
    assert!(DownloadManager::new(0).is_err());
    let dir = tempfile::tempdir().unwrap();
    let manager = DownloadManager::new(1).unwrap();
    let invalid = manager.enqueue(request(
        Url::parse("file:///tmp/test").unwrap(),
        &dir.path().join("out"),
    ));
    assert!(matches!(
        done(&invalid).await.status,
        DownloadStatus::Failed(_)
    ));
    let handle = manager.enqueue(request(
        Url::parse("http://127.0.0.1:9/").unwrap(),
        &dir.path().join("out"),
    ));
    handle.cancel();
    assert_eq!(done(&handle).await.status, DownloadStatus::Cancelled);
}
