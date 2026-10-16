//! Splits a `text/event-stream` body into complete events, keeping each event's
//! exact bytes including its blank-line delimiter. Bytes after the last
//! delimiter are an event the server never finished, and fail the stream,
//! unless they are only whitespace.

use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::{Bytes, BytesMut};
use futures_core::Stream;

use crate::call::ResponseMeta;
use crate::error::{Result, StreamError, StreamErrorKind};

type Body = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>;

pub(crate) struct Frames {
    body: Option<Body>,
    buffer: BytesMut,
    meta: ResponseMeta,
}

impl Frames {
    pub(crate) fn new(body: Body, meta: ResponseMeta) -> Self {
        Self {
            body: Some(body),
            buffer: BytesMut::new(),
            meta,
        }
    }

    /// The bytes of the first complete event in `buffer`, when there is one.
    fn split_frame(buffer: &mut BytesMut) -> Option<Bytes> {
        let end = frame_end(buffer)?;
        Some(buffer.split_to(end).freeze())
    }
}

/// The index just past the first `\n\n` or `\r\n\r\n`, whichever starts first.
fn frame_end(bytes: &[u8]) -> Option<usize> {
    let lf = find(bytes, b"\n\n").map(|start| start + 2);
    let crlf = find(bytes, b"\r\n\r\n").map(|start| start + 4);
    match (lf, crlf) {
        (Some(lf), Some(crlf)) if crlf - 4 < lf - 2 => Some(crlf),
        (Some(lf), _) => Some(lf),
        (None, crlf) => crlf,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

impl Stream for Frames {
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(frame) = Self::split_frame(&mut self.buffer) {
                return Poll::Ready(Some(Ok(frame)));
            }
            let Some(body) = self.body.as_mut() else {
                return Poll::Ready(None);
            };
            match ready!(body.as_mut().poll_next(cx)) {
                Some(Ok(chunk)) => self.buffer.extend_from_slice(&chunk),
                Some(Err(error)) => {
                    self.body = None;
                    return Poll::Ready(Some(Err(StreamError {
                        kind: StreamErrorKind::Read,
                        meta: self.meta.clone(),
                        source: Some(Box::new(error)),
                    }
                    .into())));
                }
                None => {
                    self.body = None;
                    if self.buffer.iter().all(u8::is_ascii_whitespace) {
                        self.buffer.clear();
                        return Poll::Ready(None);
                    }
                    self.buffer.clear();
                    return Poll::Ready(Some(Err(StreamError {
                        kind: StreamErrorKind::Incomplete,
                        meta: self.meta.clone(),
                        source: None,
                    }
                    .into())));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures_util::{StreamExt, stream};
    use http::HeaderMap;

    use super::{Frames, frame_end};
    use crate::call::ResponseMeta;
    use crate::error::{Error, StreamErrorKind};

    #[test]
    fn frame_ends_at_the_first_delimiter_of_either_kind() {
        assert_eq!(frame_end(b"data: a\n\ndata: b\n\n"), Some(9));
        assert_eq!(frame_end(b"data: a\r\n\r\ndata: b\n\n"), Some(11));
        assert_eq!(frame_end(b"data: a\n\ndata: b\r\n\r\n"), Some(9));
        assert_eq!(frame_end(b"data: a\ndata: b"), None);
    }

    #[tokio::test]
    async fn a_body_read_error_fails_the_stream() {
        let failing = reqwest::get("http://[::1]:1/").await.unwrap_err();
        let meta = ResponseMeta {
            status: 200,
            headers: HeaderMap::new(),
            request_id: None,
        };
        let mut frames = Frames::new(
            Box::pin(stream::iter(vec![
                Ok(Bytes::from_static(b"data: a\n")),
                Ok(Bytes::from_static(b"\ndata: b")),
                Err(failing),
            ])),
            meta,
        );

        let first = frames.next().await.unwrap().unwrap();
        assert_eq!(first, Bytes::from_static(b"data: a\n\n"));
        let error = frames.next().await.unwrap().unwrap_err();
        assert!(
            matches!(
                &error,
                Error::Stream(e)
                    if e.kind() == StreamErrorKind::Read
                        && std::error::Error::source(e.as_ref()).is_some()
            ),
            "{error:?}"
        );
        assert!(frames.next().await.is_none());
    }

    #[tokio::test]
    async fn a_body_end_is_clean_or_incomplete_depending_on_the_tail() {
        let meta = || ResponseMeta {
            status: 200,
            headers: HeaderMap::new(),
            request_id: None,
        };
        let mut frames = Frames::new(
            Box::pin(stream::iter(vec![Ok(Bytes::from_static(
                b"data: a\n\n \r\n",
            ))])),
            meta(),
        );
        assert_eq!(
            frames.next().await.unwrap().unwrap(),
            Bytes::from_static(b"data: a\n\n")
        );
        assert!(frames.next().await.is_none());

        let mut frames = Frames::new(
            Box::pin(stream::iter(vec![Ok(Bytes::from_static(
                b"data: a\n\ndata: b",
            ))])),
            meta(),
        );
        assert_eq!(
            frames.next().await.unwrap().unwrap(),
            Bytes::from_static(b"data: a\n\n")
        );
        let error = frames.next().await.unwrap().unwrap_err();
        assert!(
            matches!(&error, Error::Stream(e) if e.kind() == StreamErrorKind::Incomplete),
            "{error:?}"
        );
    }
}
