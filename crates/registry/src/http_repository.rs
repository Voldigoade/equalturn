use crate::RegistryError;
use sigstore_tuf::{HttpRepository, Repository, reqwest};
use std::time::Duration;
use url::Url;

/// HTTPS-only TUF transport with bounded requests and redirects disabled.
pub(crate) struct HttpsRepository {
    inner: HttpRepository,
}

impl HttpsRepository {
    pub(crate) fn new(base_url: &Url) -> Result<Self, RegistryError> {
        Self::build(base_url, false)
    }

    fn build(base_url: &Url, allow_http: bool) -> Result<Self, RegistryError> {
        if base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
            || (!allow_http && base_url.scheme() != "https")
            || (allow_http && !matches!(base_url.scheme(), "http" | "https"))
        {
            return Err(RegistryError::Repository);
        }

        let mut repository_root = base_url.clone();
        if !repository_root.path().ends_with('/') {
            let directory = format!("{}/", repository_root.path().trim_end_matches('/'));
            repository_root.set_path(&directory);
        }
        let metadata_base = repository_root
            .join("metadata/")
            .map_err(|_| RegistryError::Repository)?;
        let targets_base = repository_root
            .join("targets/")
            .map_err(|_| RegistryError::Repository)?;

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| RegistryError::Repository)?;
        let inner = HttpRepository::new(metadata_base.as_str())
            .map_err(|_| RegistryError::Repository)?
            .with_targets_base(targets_base.as_str())
            .map_err(|_| RegistryError::Repository)?
            .with_http_client(client);
        Ok(Self { inner })
    }

    #[cfg(test)]
    fn for_test(base_url: &Url) -> Self {
        Self::build(base_url, true).expect("test HTTP repository is valid")
    }
}

impl Repository for HttpsRepository {
    fn fetch_metadata<'a>(
        &'a self,
        name: &'a str,
        max_length: u64,
    ) -> sigstore_tuf::transport::FetchFuture<'a> {
        self.inner.fetch_metadata(name, max_length)
    }

    fn fetch_target<'a>(
        &'a self,
        path: &'a str,
        max_length: u64,
    ) -> sigstore_tuf::transport::FetchFuture<'a> {
        self.inner.fetch_target(path, max_length)
    }
}

#[cfg(test)]
mod tests {
    use super::HttpsRepository;
    use sigstore_tuf::Repository;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };
    use url::Url;

    fn response_server(response: Vec<u8>) -> (Url, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener binds");
        let address = listener
            .local_addr()
            .expect("test listener address is available");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test request arrives");
            let _ = stream.write_all(&response);
        });
        (
            Url::parse(&format!("http://{address}/repository/")).expect("test URL parses"),
            server,
        )
    }

    fn path_server(count: usize) -> (Url, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener binds");
        let address = listener
            .local_addr()
            .expect("test listener address is available");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let observed_requests = Arc::clone(&requests);
        let server = thread::spawn(move || {
            for _ in 0..count {
                let (mut stream, _) = listener.accept().expect("test request arrives");
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while stream.read(&mut byte).expect("request reads") == 1 {
                    request.push(byte[0]);
                    if request.ends_with(b"\r\n") {
                        break;
                    }
                }
                observed_requests
                    .lock()
                    .expect("request collection lock succeeds")
                    .push(String::from_utf8(request).expect("request line is UTF-8"));
                stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .expect("404 response writes");
            }
            drop(observed_requests);
            Arc::try_unwrap(requests)
                .expect("test server owns request collection")
                .into_inner()
                .expect("request collection lock succeeds")
        });
        (
            Url::parse(&format!("http://{address}/repository/")).expect("test URL parses"),
            server,
        )
    }

    #[test]
    fn accepts_only_clean_https_origins() {
        for input in [
            "http://example.test/repository/",
            "https://user@example.test/repository/",
            "https://user:password@example.test/repository/",
            "https://example.test/repository/?token=secret",
            "https://example.test/repository/#fragment",
            "file:///tmp/repository/",
        ] {
            let url = Url::parse(input).expect("test URL parses");
            assert!(HttpsRepository::new(&url).is_err(), "accepted {input:?}");
        }
    }

    #[tokio::test]
    async fn does_not_follow_redirects() {
        let redirect_target = TcpListener::bind("127.0.0.1:0").expect("target listener binds");
        redirect_target
            .set_nonblocking(true)
            .expect("target listener becomes nonblocking");
        let target_address = redirect_target
            .local_addr()
            .expect("target listener address is available");
        let target_hit = Arc::new(AtomicBool::new(false));
        let observed_hit = Arc::clone(&target_hit);
        let target_server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(250);
            while Instant::now() < deadline {
                match redirect_target.accept() {
                    Ok((mut stream, _)) => {
                        observed_hit.store(true, Ordering::SeqCst);
                        let _ = stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nredirect",
                        );
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return,
                }
            }
        });
        let redirect = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let (url, server) = response_server(redirect.into_bytes());
        let repository = HttpsRepository::for_test(&url);
        assert!(
            repository
                .fetch_target("registry-v3.json", 128)
                .await
                .is_err()
        );
        server.join().expect("test server exits");
        target_server.join().expect("redirect target server exits");
        assert!(
            !target_hit.load(Ordering::SeqCst),
            "redirect target was contacted"
        );
    }

    #[tokio::test]
    async fn rejects_response_larger_than_tuf_limit() {
        let (url, server) = response_server(
            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n12345678".to_vec(),
        );
        let repository = HttpsRepository::for_test(&url);
        assert!(
            repository
                .fetch_target("registry-v3.json", 4)
                .await
                .is_err()
        );
        server.join().expect("test server exits");
    }

    #[tokio::test]
    async fn rejects_chunked_response_larger_than_tuf_limit() {
        let (url, server) = response_server(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n".to_vec(),
        );
        let repository = HttpsRepository::for_test(&url);
        assert!(
            repository
                .fetch_target("registry-v3.json", 5)
                .await
                .is_err()
        );
        server.join().expect("test server exits");
    }

    #[tokio::test]
    async fn maps_repository_root_to_metadata_and_targets_directories() {
        let (url, server) = path_server(2);
        let repository = HttpsRepository::for_test(&url);

        assert!(
            repository
                .fetch_metadata("timestamp.json", 128)
                .await
                .expect("missing timestamp is a normal missing-object response")
                .is_none()
        );
        assert!(
            repository
                .fetch_target("registry-v3.json", 128)
                .await
                .expect("missing target is a normal missing-object response")
                .is_none()
        );

        assert_eq!(
            server.join().expect("test server exits"),
            [
                "GET /repository/metadata/timestamp.json HTTP/1.1\r\n",
                "GET /repository/targets/registry-v3.json HTTP/1.1\r\n",
            ]
        );
    }
}
