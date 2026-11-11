//! Google Drive v3 over REST. The app only asks for the `drive.file` scope, so Drive shows
//! it nothing but the files it created; the access token comes from the platform.

use std::thread;
use std::time::Duration;

use anyhow::{Context, anyhow};
use serde::Deserialize;
use serde_json::json;
use ureq::Agent;
use ureq::http::{Method, Request};

use crate::local::sha256_hex;
use crate::remote::{
    Listing, ROOT_FOLDER_NAME, Remote, RemoteError, RemoteFile, RemoteFolder, Revision,
};

/// Drive's API, unless a test points elsewhere.
const DEFAULT_BASE_URL: &str = "https://www.googleapis.com";
/// The type Drive gives folders.
const FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";
/// The fields of a file the sync reads.
const FILE_FIELDS: &str = "id,name,mimeType,parents,md5Checksum,modifiedTime,createdTime";
/// The waits before each retry of a request Drive asked to slow down.
const RETRY_WAITS_SECS: [u64; 5] = [1, 2, 4, 8, 16];
/// The longest a request may take, uploads of big notebooks included.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

/// Gives the current access token for the `drive.file` scope.
type TokenSource = Box<dyn FnMut() -> Result<String, RemoteError>>;

/// A [`Remote`] on the real Google Drive.
pub struct DriveClient {
    agent: Agent,
    base_url: String,
    token: TokenSource,
    sleep: Box<dyn FnMut(Duration)>,
}

/// What a request sends.
enum Body {
    Empty,
    Json(serde_json::Value),
    Bytes { content_type: String, bytes: Vec<u8> },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileJson {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    parents: Vec<String>,
    #[serde(default)]
    md5_checksum: String,
    #[serde(default)]
    modified_time: String,
    #[serde(default)]
    created_time: String,
    #[serde(default)]
    trashed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevisionJson {
    id: String,
    #[serde(default)]
    md5_checksum: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevisionListJson {
    #[serde(default)]
    revisions: Vec<RevisionJson>,
    next_page_token: Option<String>,
}

/// Drive answered 404: the file isn't there (or the app can't see it).
#[derive(Debug, thiserror::Error)]
#[error("Google Drive has no such file")]
struct NotFound;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileListJson {
    #[serde(default)]
    files: Vec<FileJson>,
    next_page_token: Option<String>,
}

impl DriveClient {
    /// A client that asks `token` for the access token before each request.
    pub fn new(token: impl FnMut() -> Result<String, RemoteError> + 'static) -> Self {
        let agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();
        Self {
            agent,
            base_url: DEFAULT_BASE_URL.to_owned(),
            token: Box::new(token),
            sleep: Box::new(thread::sleep),
        }
    }

    /// Sends the requests to `url` instead of Google (for tests).
    #[must_use]
    pub fn with_base_url(mut self, url: &str) -> Self {
        url.trim_end_matches('/').clone_into(&mut self.base_url);
        self
    }

    /// Waits with `sleep` before retrying (for tests).
    #[must_use]
    pub fn with_sleep(mut self, sleep: impl FnMut(Duration) + 'static) -> Self {
        self.sleep = Box::new(sleep);
        self
    }

    /// Sends a request, retrying when Drive asks to slow down. Returns the response body.
    fn send(
        &mut self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Body,
    ) -> Result<Vec<u8>, RemoteError> {
        let mut url = url::Url::parse(&format!("{}{path}", self.base_url))
            .with_context(|| format!("Bad Drive URL for {path}"))?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let mut waits = RETRY_WAITS_SECS.iter();
        loop {
            let token = (self.token)()?;
            let mut request = Request::builder()
                .method(method.clone())
                .uri(url.as_str())
                .header("Authorization", format!("Bearer {token}"));
            let bytes = match &body {
                Body::Empty => Vec::new(),
                Body::Json(value) => {
                    request = request.header("Content-Type", "application/json; charset=UTF-8");
                    serde_json::to_vec(value).context("Encoding a Drive request")?
                }
                Body::Bytes { content_type, bytes } => {
                    request = request.header("Content-Type", content_type.as_str());
                    bytes.clone()
                }
            };
            let request = request.body(bytes).context("Building a Drive request")?;
            let mut response = self.agent.run(request).map_err(transport_error)?;
            let status = response.status().as_u16();
            let text = response
                .body_mut()
                .with_config()
                .limit(u64::MAX)
                .read_to_vec()
                .map_err(transport_error)?;
            if (200..300).contains(&status) {
                return Ok(text);
            }
            if status == 401 {
                return Err(RemoteError::Unauthorized);
            }
            if status == 404 {
                return Err(RemoteError::Failed(NotFound.into()));
            }
            match waits.next() {
                Some(&wait) if should_retry(status, &text) => {
                    (self.sleep)(Duration::from_secs(wait));
                }
                _ => {
                    let text = String::from_utf8_lossy(&text);
                    return Err(RemoteError::Failed(anyhow!("Drive answered {status}: {text}")));
                }
            }
        }
    }

    fn send_json<T: for<'de> Deserialize<'de>>(
        &mut self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Body,
    ) -> Result<T, RemoteError> {
        let bytes = self.send(method, path, query, body)?;
        Ok(serde_json::from_slice(&bytes).context("Reading Drive's answer")?)
    }

    fn send_file(
        &mut self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Body,
    ) -> Result<RemoteFile, RemoteError> {
        let file: FileJson = self.send_json(method, path, query, body)?;
        Ok(file.into_remote_file())
    }
}

impl FileJson {
    fn into_remote_file(self) -> RemoteFile {
        RemoteFile {
            id: self.id,
            parent: self.parents.into_iter().next().unwrap_or_default(),
            name: self.name,
            md5: self.md5_checksum,
            modified: self.modified_time,
        }
    }
}

impl Remote for DriveClient {
    fn find_root(&mut self) -> Result<Option<String>, RemoteError> {
        let q = format!(
            "name = '{ROOT_FOLDER_NAME}' and mimeType = '{FOLDER_MIME_TYPE}' and 'root' in parents and trashed = false"
        );
        let query = [("q", q.as_str()), ("fields", "files(id)"), ("spaces", "drive")];
        let list: FileListJson =
            self.send_json(Method::GET, "/drive/v3/files", &query, Body::Empty)?;
        Ok(list.files.into_iter().next().map(|file| file.id))
    }

    fn list(&mut self) -> Result<Listing, RemoteError> {
        let fields = format!("nextPageToken,files({FILE_FIELDS})");
        let mut listing = Listing::default();
        let mut page_token: Option<String> = None;
        loop {
            let mut query = vec![
                ("q", "trashed = false"),
                ("fields", fields.as_str()),
                ("pageSize", "1000"),
                ("spaces", "drive"),
            ];
            if let Some(token) = &page_token {
                query.push(("pageToken", token.as_str()));
            }
            let page: FileListJson =
                self.send_json(Method::GET, "/drive/v3/files", &query, Body::Empty)?;
            for file in page.files {
                if file.mime_type == FOLDER_MIME_TYPE {
                    listing.folders.push(RemoteFolder {
                        parent: file.parents.first().cloned(),
                        id: file.id,
                        name: file.name,
                        created: file.created_time,
                    });
                } else {
                    listing.files.push(file.into_remote_file());
                }
            }
            match page.next_page_token {
                Some(token) => page_token = Some(token),
                None => return Ok(listing),
            }
        }
    }

    fn get(&mut self, id: &str) -> Result<Option<RemoteFile>, RemoteError> {
        let fields = format!("{FILE_FIELDS},trashed");
        let query = [("fields", fields.as_str())];
        match self.send_json::<FileJson>(
            Method::GET,
            &format!("/drive/v3/files/{id}"),
            &query,
            Body::Empty,
        ) {
            Ok(file) if file.trashed => Ok(None),
            Ok(file) => Ok(Some(file.into_remote_file())),
            Err(RemoteError::Failed(error)) if error.is::<NotFound>() => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn revisions(&mut self, id: &str) -> Result<Vec<Revision>, RemoteError> {
        let path = format!("/drive/v3/files/{id}/revisions");
        let mut revisions = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut query =
                vec![("fields", "nextPageToken,revisions(id,md5Checksum)"), ("pageSize", "1000")];
            if let Some(token) = &page_token {
                query.push(("pageToken", token.as_str()));
            }
            let page: RevisionListJson = self.send_json(Method::GET, &path, &query, Body::Empty)?;
            revisions.extend(
                page.revisions
                    .into_iter()
                    .map(|revision| Revision { id: revision.id, md5: revision.md5_checksum }),
            );
            match page.next_page_token {
                Some(token) => page_token = Some(token),
                None => return Ok(revisions),
            }
        }
    }

    fn download_revision(&mut self, id: &str, revision: &str) -> Result<Vec<u8>, RemoteError> {
        let path = format!("/drive/v3/files/{id}/revisions/{revision}");
        self.send(Method::GET, &path, &[("alt", "media")], Body::Empty)
    }

    fn download(&mut self, id: &str) -> Result<Vec<u8>, RemoteError> {
        self.send(Method::GET, &format!("/drive/v3/files/{id}"), &[("alt", "media")], Body::Empty)
    }

    fn create_file(
        &mut self,
        parent: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<RemoteFile, RemoteError> {
        // The content can't hold its own hash, so it can't hold the boundary.
        let boundary = format!("rnote-sync-{}", sha256_hex(bytes));
        let metadata = json!({"name": name, "parents": [parent]});
        let mut body = format!(
            "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let query = [("uploadType", "multipart"), ("fields", FILE_FIELDS)];
        let content_type = format!("multipart/related; boundary={boundary}");
        let body = Body::Bytes { content_type, bytes: body };
        self.send_file(Method::POST, "/upload/drive/v3/files", &query, body)
    }

    fn update_file(&mut self, id: &str, bytes: &[u8]) -> Result<RemoteFile, RemoteError> {
        let query = [("uploadType", "media"), ("fields", FILE_FIELDS)];
        let body = Body::Bytes {
            content_type: "application/octet-stream".to_owned(),
            bytes: bytes.to_vec(),
        };
        self.send_file(Method::PATCH, &format!("/upload/drive/v3/files/{id}"), &query, body)
    }

    fn move_file(
        &mut self,
        id: &str,
        from_parent: &str,
        to_parent: &str,
        name: &str,
    ) -> Result<RemoteFile, RemoteError> {
        let mut query = vec![("fields", FILE_FIELDS)];
        if from_parent != to_parent {
            query.extend([("addParents", to_parent), ("removeParents", from_parent)]);
        }
        let body = Body::Json(json!({"name": name}));
        self.send_file(Method::PATCH, &format!("/drive/v3/files/{id}"), &query, body)
    }

    fn create_folder(&mut self, parent: Option<&str>, name: &str) -> Result<String, RemoteError> {
        let mut metadata = json!({"name": name, "mimeType": FOLDER_MIME_TYPE});
        if let Some(parent) = parent {
            metadata["parents"] = json!([parent]);
        }
        let file: FileJson = self.send_json(
            Method::POST,
            "/drive/v3/files",
            &[("fields", "id")],
            Body::Json(metadata),
        )?;
        Ok(file.id)
    }

    fn trash(&mut self, id: &str) -> Result<(), RemoteError> {
        let body = Body::Json(json!({"trashed": true}));
        self.send(Method::PATCH, &format!("/drive/v3/files/{id}"), &[("fields", "id")], body)?;
        Ok(())
    }
}

/// Whether Drive asked to slow down or failed on its side.
fn should_retry(status: u16, body: &[u8]) -> bool {
    let rate_limited = || {
        let text = String::from_utf8_lossy(body);
        text.contains("\"rateLimitExceeded\"") || text.contains("\"userRateLimitExceeded\"")
    };
    status == 429 || (500..600).contains(&status) || (status == 403 && rate_limited())
}

/// A failure to talk to Google at all.
pub(crate) fn transport_error(error: ureq::Error) -> RemoteError {
    match error {
        ureq::Error::Io(_)
        | ureq::Error::Timeout(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed => RemoteError::Offline(error.to_string()),
        other => RemoteError::Failed(anyhow!(other).context("Talking to Google Drive")),
    }
}
