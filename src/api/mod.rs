pub mod app;
pub mod client;

/// A redirect that leaves the provider's host must not carry what these clients send.
#[cfg(test)]
pub(crate) mod redirect_stub {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// A server on 127.0.0.1 answering each request with `answer`, and what arrived there.
    pub fn stub(answer: String) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                log.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                let _ = stream.write_all(answer.as_bytes());
            }
        });
        (url, seen)
    }

    /// The first stub redirects with a 307, which re-sends a POST's body, to a second on another
    /// port. The second is where a credential must not arrive.
    pub fn redirect_to_another_host() -> (String, Arc<Mutex<Vec<String>>>) {
        let (elsewhere, seen) = stub("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".into());
        let (first, _) = stub(format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {elsewhere}/x\r\nContent-Length: 0\r\n\r\n"
        ));
        (first, seen)
    }

    /// What the refused redirect comes back as.
    pub fn refused_redirect() -> reqwest::Error {
        let (first, _) = redirect_to_another_host();
        let client = reqwest::Client::builder()
            .redirect(pk_cli_core::transport::same_host_redirects())
            .build()
            .unwrap();
        runtime()
            .block_on(async { client.get(&first).send().await })
            .unwrap_err()
    }

    pub fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }
}
