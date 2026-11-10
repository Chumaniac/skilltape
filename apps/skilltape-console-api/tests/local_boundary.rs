use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn loopback_server_rejects_foreign_hosts_and_origins() {
    let workspace = tempfile::tempdir().unwrap();
    let reserve = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reserve.local_addr().unwrap();
    drop(reserve);
    let root = workspace.path().to_owned();
    let task = tokio::spawn(async move { skilltape_console_api::serve(root, address).await });
    for _ in 0..100 {
        if TcpStream::connect(address).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    for (host, origin, status) in [
        (address.to_string(), None, 200),
        (format!("localhost:{}", address.port()), None, 200),
        (format!("outside.invalid:{}", address.port()), None, 403),
        (
            format!(
                "127.0.0.1:{}",
                if address.port() == u16::MAX {
                    address.port() - 1
                } else {
                    address.port() + 1
                }
            ),
            None,
            403,
        ),
        (
            address.to_string(),
            Some("http://outside.invalid".to_owned()),
            403,
        ),
        (address.to_string(), Some("null".to_owned()), 403),
        (address.to_string(), Some(format!("http://{address}")), 200),
    ] {
        let mut connection = TcpStream::connect(address).await.unwrap();
        let origin = origin.map_or(String::new(), |value| format!("Origin: {value}\r\n"));
        let request = format!(
            "GET /api/v1/workspaces HTTP/1.1\r\nHost: {host}\r\n{origin}Connection: close\r\n\r\n"
        );
        connection.write_all(request.as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(2),
            connection.take(131072).read_to_end(&mut bytes),
        )
        .await
        .unwrap()
        .unwrap();
        let line = String::from_utf8(bytes)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_owned();
        assert!(
            line.starts_with(&format!("HTTP/1.1 {status} ")),
            "unexpected HTTP status"
        );
    }
    task.abort();
    let _ = task.await;
}
