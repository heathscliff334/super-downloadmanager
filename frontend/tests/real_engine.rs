use download_manager_ui::engine::{AddRequest, DownloadEngine, DownloadState};
use download_manager_ui::real::RealEngine;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

#[test]
fn downloads_a_real_file_through_the_ui_adapter() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello")
            .unwrap();
    });
    let dir = std::env::temp_dir().join(format!(
        "super-download-manager-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut engine = RealEngine::new().unwrap();
    engine
        .add(AddRequest {
            url: format!("http://{addr}/hello.txt"),
            file_name: None,
            save_dir: dir.clone(),
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let item = &engine.downloads()[0];
        match &item.state {
            DownloadState::Completed => break,
            DownloadState::Failed(e) => panic!("download failed: {e}"),
            _ => assert!(Instant::now() < deadline, "download timed out"),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read(dir.join("hello.txt")).unwrap(), b"hello");
    server.join().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
