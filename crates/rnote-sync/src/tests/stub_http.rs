//! A one-thread HTTP server on 127.0.0.1 that answers scripted responses in order and
//! records the requests, to test the Drive client without Google.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// A request the stub got.
#[derive(Debug, Clone)]
pub(super) struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The path, without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    /// The decoded value of the query parameter `key`.
    pub fn query(&self, key: &str) -> Option<String> {
        let url = url::Url::parse(&format!("http://stub{}", self.target)).unwrap();
        url.query_pairs().find(|(name, _)| name == key).map(|(_, value)| value.into_owned())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

/// A scripted response: status, content type, extra headers and body.
pub(super) type Response = (u16, &'static str, Vec<(&'static str, &'static str)>, Vec<u8>);

/// The server; it stops after answering every scripted response.
pub(super) struct Stub {
    url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    thread: Option<JoinHandle<()>>,
}

impl Stub {
    /// Starts answering `responses` (status, content type, body), one per connection.
    pub fn start(responses: Vec<(u16, &'static str, Vec<u8>)>) -> Self {
        let responses = responses
            .into_iter()
            .map(|(status, content_type, body)| (status, content_type, Vec::new(), body))
            .collect();
        Self::start_with_headers(responses)
    }

    /// Like [`Stub::start`], each response with extra headers; `{url}` in a header value
    /// becomes the stub's own URL.
    pub fn start_with_headers(responses: Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let own_url = url.clone();
        let thread = std::thread::spawn(move || {
            for (status, content_type, extra, body) in responses {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream);
                recorded.lock().unwrap().push(read_request(&mut reader));
                let mut stream = reader.into_inner();
                let extra: String = extra
                    .iter()
                    .map(|(key, value)| format!("{key}: {}\r\n", value.replace("{url}", &own_url)))
                    .collect();
                let head = format!(
                    "HTTP/1.1 {status} Stub\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
                stream.flush().unwrap();
            }
        });
        Self { url, requests, thread: Some(thread) }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// The requests so far.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Stub {
    fn drop(&mut self) {
        // Only wait if every response was used; otherwise the thread waits for a client.
        if let Some(thread) = self.thread.take().filter(|thread| thread.is_finished()) {
            thread.join().unwrap();
        }
    }
}

fn read_request(reader: &mut BufReader<std::net::TcpStream>) -> Request {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((key, value)) = header.split_once(':') {
            headers.push((key.trim().to_owned(), value.trim().to_owned()));
        }
    }
    let length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, value)| value.parse().unwrap());
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    Request { method, target, headers, body }
}
