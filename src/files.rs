//! File bodies: [`Upload`] for multipart and octet-stream requests, [`Download`]
//! for streamed octet-stream responses.

use std::fmt;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use futures_core::Stream;
use futures_util::TryStreamExt;

use crate::call::ResponseMeta;
use crate::error::{Result, StreamError, StreamErrorKind};

type ByteStream<E> = Pin<Box<dyn Stream<Item = std::result::Result<Bytes, E>> + Send>>;

enum UploadBody {
    Bytes(Bytes),
    Stream {
        stream: ByteStream<std::io::Error>,
        length: Option<u64>,
    },
}

/// A file to send. Build it from bytes, a path, or a byte stream.
pub struct Upload {
    body: UploadBody,
    file_name: Option<String>,
    content_type: Option<String>,
}

impl Upload {
    /// An in-memory body.
    pub fn bytes(bytes: impl Into<Bytes>) -> Self {
        Self::new(UploadBody::Bytes(bytes.into()))
    }

    /// Streams a file from disk. The file name and length come from the path,
    /// and a multipart part's Content-Type is guessed from the file name's
    /// extension unless [`content_type`](Self::content_type) is set.
    pub async fn file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let file = tokio::fs::File::open(path).await?;
        let length = file.metadata().await?.len();
        let stream = tokio_util::io::ReaderStream::new(file);
        let mut upload = Self::stream(stream, Some(length));
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            upload = upload.file_name(name);
        }
        Ok(upload)
    }

    /// A streamed body. Pass `length` when it is known so the server gets a
    /// `Content-Length`.
    pub fn stream(
        stream: impl Stream<Item = std::io::Result<Bytes>> + Send + 'static,
        length: Option<u64>,
    ) -> Self {
        Self::new(UploadBody::Stream {
            stream: Box::pin(stream),
            length,
        })
    }

    /// The file name sent in a multipart part; its extension sets the part's
    /// Content-Type unless [`content_type`](Self::content_type) is set.
    /// Defaults to `file`.
    #[must_use]
    pub fn file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    /// The MIME type sent in a multipart part. Overrides the type guessed
    /// from the file name's extension.
    #[must_use]
    pub fn content_type(mut self, mime: impl Into<String>) -> Self {
        self.content_type = Some(mime.into());
        self
    }

    fn new(body: UploadBody) -> Self {
        Self {
            body,
            file_name: None,
            content_type: None,
        }
    }

    pub(crate) fn default_file_name(mut self, name: impl Into<String>) -> Self {
        if self.file_name.is_none() {
            self.file_name = Some(name.into());
        }
        self
    }

    pub(crate) fn into_part(self) -> Result<reqwest::multipart::Part> {
        let mut part = match self.body {
            UploadBody::Bytes(bytes) => reqwest::multipart::Part::bytes(bytes.to_vec()),
            UploadBody::Stream { stream, length } => {
                let body = reqwest::Body::wrap_stream(stream);
                match length {
                    Some(length) => reqwest::multipart::Part::stream_with_length(body, length),
                    None => reqwest::multipart::Part::stream(body),
                }
            }
        };
        let mime = self.content_type.or_else(|| {
            self.file_name
                .as_deref()
                .map(|name| guess_content_type(name).to_owned())
        });
        part = part.file_name(self.file_name.unwrap_or_else(|| "file".to_owned()));
        if let Some(mime) = mime {
            part = part
                .mime_str(&mime)
                .map_err(|_| crate::Error::InvalidArgument(format!("invalid MIME type {mime}")))?;
        }
        Ok(part)
    }

    pub(crate) fn into_body(self) -> reqwest::Body {
        match self.body {
            UploadBody::Bytes(bytes) => reqwest::Body::from(bytes),
            UploadBody::Stream { stream, .. } => reqwest::Body::wrap_stream(stream),
        }
    }
}

impl fmt::Debug for Upload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upload")
            .field("file_name", &self.file_name)
            .field("content_type", &self.content_type)
            .finish_non_exhaustive()
    }
}

/// The part Content-Type a file name implies, by its extension.
fn guess_content_type(file_name: &str) -> &'static str {
    match Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("zip") => "application/zip",
        Some("tar") => "application/x-tar",
        Some("gz" | "tgz") => "application/gzip",
        Some("md") => "text/markdown",
        Some("txt") => "text/plain",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// A streamed `application/octet-stream` response body. Drop it to close the
/// connection.
pub struct Download {
    meta: ResponseMeta,
    body: ByteStream<reqwest::Error>,
}

impl Download {
    pub(crate) fn new(meta: ResponseMeta, body: ByteStream<reqwest::Error>) -> Self {
        Self { meta, body }
    }

    /// Status, headers and request ID of the response.
    pub fn meta(&self) -> &ResponseMeta {
        &self.meta
    }

    /// The `Content-Type` header.
    pub fn content_type(&self) -> Option<&str> {
        self.meta
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
    }

    /// The `Content-Length` header.
    pub fn content_length(&self) -> Option<u64> {
        self.meta
            .headers()
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok()?.parse().ok())
    }

    /// Reads the rest of the body into memory.
    pub async fn bytes(mut self) -> Result<Bytes> {
        let mut buffer = BytesMut::new();
        while let Some(chunk) = self.try_next().await? {
            buffer.extend_from_slice(&chunk);
        }
        Ok(buffer.freeze())
    }
}

impl Stream for Download {
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.body.as_mut().poll_next(cx).map(|next| {
            next.map(|chunk| {
                chunk.map_err(|error| {
                    StreamError {
                        kind: StreamErrorKind::Read,
                        meta: self.meta.clone(),
                        source: Some(Box::new(error)),
                    }
                    .into()
                })
            })
        })
    }
}

impl fmt::Debug for Download {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Download")
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}
