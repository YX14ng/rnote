//! Uploading big files to Drive in a resumable session, chunk by chunk: a chunk that doesn't
//! get through (a dropped mobile connection, a Drive hiccup) is sent again from where Google
//! got, instead of the whole notebook starting over and the ones after it never going up.

use std::time::Duration;

use anyhow::{Context, anyhow};
use ureq::http::{HeaderMap, Method, Request};

use super::{Body, DriveClient, FileJson, RETRY_WAITS_SECS, should_retry, transport_error};
use crate::remote::{RemoteError, RemoteFile};

/// Bytes sent per request of a resumable upload; Drive wants a multiple of 256 KiB.
pub(super) const CHUNK_BYTES: usize = 8 * 1024 * 1024;

/// What Drive answered to a chunk.
enum Answer {
    /// The file is complete.
    Done(RemoteFile),
    /// Drive has the bytes before this offset; the rest goes next.
    Next(usize),
}

/// Why a chunk didn't get through.
enum ChunkError {
    /// The connection or Drive failed: asking how far it got and going on may work.
    Retry(RemoteError),
    /// Anything else, e.g. the session expired or Drive refused the file.
    Stop(RemoteError),
}

impl DriveClient {
    /// Uploads `bytes` in a resumable session started with `method` on `path`: the first
    /// request (with `metadata`, if any) opens the session, then the bytes go in chunks.
    pub(super) fn upload_resumable(
        &mut self,
        method: Method,
        path: &str,
        metadata: Option<serde_json::Value>,
        bytes: &[u8],
    ) -> Result<RemoteFile, RemoteError> {
        let query = [("uploadType", "resumable"), ("fields", super::FILE_FIELDS)];
        let url = self.url(path, &query)?;
        let body = metadata.map_or(Body::Empty, Body::Json);
        let headers = [("X-Upload-Content-Type", "application/octet-stream")];
        let (headers, _) = self.send_to(method, &url, &headers, body)?;
        let session = headers
            .get("location")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| anyhow!("Drive opened no upload session"))?;
        let session = url::Url::parse(session).context("Bad Drive upload session URL")?;
        self.upload_chunks(&session, bytes)
    }

    /// Sends `bytes` to the upload `session` chunk by chunk, going on from where Google got
    /// after a chunk fails, a few times before giving up.
    fn upload_chunks(
        &mut self,
        session: &url::Url,
        bytes: &[u8],
    ) -> Result<RemoteFile, RemoteError> {
        let total = bytes.len();
        let mut start = 0;
        let mut waits = RETRY_WAITS_SECS.iter();
        loop {
            let end = (start + self.chunk_bytes).min(total);
            let range = format!("bytes {start}-{}/{total}", end.saturating_sub(1));
            let answer = match self.put_chunk(session, &range, &bytes[start..end]) {
                Ok(answer) => answer,
                Err(ChunkError::Stop(error)) => return Err(error),
                Err(ChunkError::Retry(error)) => {
                    let Some(&wait) = waits.next() else {
                        return Err(error);
                    };
                    (self.sleep)(Duration::from_secs(wait));
                    // How far Google got; if even that fails, the same chunk goes again.
                    match self.put_chunk(session, &format!("bytes */{total}"), &[]) {
                        Ok(answer) => answer,
                        Err(ChunkError::Stop(error)) => return Err(error),
                        Err(ChunkError::Retry(_)) => Answer::Next(start),
                    }
                }
            };
            match answer {
                Answer::Done(file) => return Ok(file),
                Answer::Next(next) if next > total => {
                    return Err(RemoteError::Failed(anyhow!("Drive got more bytes than sent")));
                }
                Answer::Next(next) => start = next,
            }
        }
    }

    /// Sends `chunk` with the `Content-Range` header `range` to the upload `session`.
    fn put_chunk(
        &mut self,
        session: &url::Url,
        range: &str,
        chunk: &[u8],
    ) -> Result<Answer, ChunkError> {
        let token = (self.token)().map_err(ChunkError::Stop)?;
        let request = Request::builder()
            .method(Method::PUT)
            .uri(session.as_str())
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/octet-stream")
            .header("Content-Range", range)
            .body(chunk)
            .context("Building a Drive request")
            .map_err(|error| ChunkError::Stop(RemoteError::Failed(error)))?;
        let mut response = self
            .upload_agent
            .run(request)
            .map_err(|error| ChunkError::Retry(transport_error(error)))?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let text = response
            .body_mut()
            .with_config()
            .limit(u64::MAX)
            .read_to_vec()
            .map_err(|error| ChunkError::Retry(transport_error(error)))?;
        match status {
            200 | 201 => {
                let file: FileJson = serde_json::from_slice(&text)
                    .context("Reading Drive's answer")
                    .map_err(|error| ChunkError::Stop(RemoteError::Failed(error)))?;
                Ok(Answer::Done(file.into_remote_file()))
            }
            308 => Ok(Answer::Next(received(&headers))),
            401 => Err(ChunkError::Stop(RemoteError::Unauthorized)),
            _ if should_retry(status, &text) => Err(ChunkError::Retry(RemoteError::Failed(
                crate::DriveRefusal::new(status, &text).into(),
            ))),
            _ => Err(ChunkError::Stop(RemoteError::Failed(
                crate::DriveRefusal::new(status, &text).into(),
            ))),
        }
    }
}

/// How many bytes Drive has, from the `Range: bytes=0-N` of a 308 answer (none: 0).
fn received(headers: &HeaderMap) -> usize {
    headers
        .get("range")
        .and_then(|value| value.to_str().ok())
        .and_then(|range| range.rsplit('-').next())
        .and_then(|last| last.parse::<usize>().ok())
        .map_or(0, |last| last + 1)
}
