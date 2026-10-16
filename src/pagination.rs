//! Cursor pagination. A list call returns one page when awaited; its
//! [`pages()`](crate::call::Call::pages) and [`items()`](crate::call::Call::items)
//! adapters walk every page lazily.

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// A response page. Implemented by [`Page`] and by the page types whose shape
/// differs from `{ data, nextCursor }`.
pub trait Paginated: DeserializeOwned + Send + 'static {
    /// The item type of the page.
    type Item: Send + 'static;

    /// The cursor of the next page, or `None` on the last page.
    fn next_cursor(&self) -> Option<&Cursor>;

    /// The page's items.
    fn into_items(self) -> Vec<Self::Item>;
}

/// The common `{ data, nextCursor }` page.
#[derive(Clone, Debug, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    /// The page's items.
    pub data: Vec<T>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
}

impl<T: DeserializeOwned + Send + 'static> Paginated for Page<T> {
    type Item = T;

    fn next_cursor(&self) -> Option<&Cursor> {
        self.next_cursor.as_ref()
    }

    fn into_items(self) -> Vec<T> {
        self.data
    }
}

/// An opaque position in a list, taken from a page's `nextCursor`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cursor(String);

impl Cursor {
    /// The cursor as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Cursor {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for Cursor {
    fn from(value: String) -> Self {
        Self(value)
    }
}

type BoxStream<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send>>;

/// Every page of a list, fetched lazily. Nothing is sent before the first poll.
pub struct Pages<P> {
    inner: BoxStream<P>,
}

impl<P> Pages<P> {
    pub(crate) fn new(inner: BoxStream<P>) -> Self {
        Self { inner }
    }
}

impl<P> Stream for Pages<P> {
    type Item = Result<P>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

impl<P> fmt::Debug for Pages<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Pages")
    }
}

/// Every item of a list across its pages, fetched lazily.
pub struct Items<T> {
    inner: BoxStream<T>,
}

impl<T> Items<T> {
    pub(crate) fn new(inner: BoxStream<T>) -> Self {
        Self { inner }
    }
}

impl<T> Stream for Items<T> {
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

impl<T> fmt::Debug for Items<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Items")
    }
}
