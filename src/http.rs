use std::{
    ops::RangeInclusive,
    path::{Component, Path, PathBuf},
};

use bytes::Bytes;
use futures::Stream;
use http_body::Body;
use http_range_header::RangeUnsatisfiableError;
use percent_encoding::percent_decode;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, Take};
use tokio_util::io::ReaderStream;

pin_project! {
    #[derive(Debug)]
    pub struct AsyncReadBody<T> {
        #[pin]
        reader: ReaderStream<T>
    }
}

impl<T> AsyncReadBody<T>
where
    T: AsyncRead,
{
    /// Create a new [`AsyncReadBody`] wrapping the given reader, with a
    /// specific read buffer capacity
    pub fn with_capacity(read: T, capacity: usize) -> Self {
        Self {
            reader: ReaderStream::with_capacity(read, capacity),
        }
    }
}

impl<T> AsyncReadBody<T>
where
    T: AsyncRead + AsyncSeek + Unpin,
{
    /// Create a new [`AsyncReadBody`] wrapping the given reader with a
    /// specific buffer capacity and range.
    ///
    /// # Errors
    ///
    /// If the reader fails to seek to the start of the range
    pub async fn with_range(
        mut read: T,
        capacity: usize,
        range: RangeInclusive<u64>,
    ) -> std::io::Result<AsyncReadBody<Take<T>>> {
        read.seek(std::io::SeekFrom::Start(*range.start())).await?;
        let max_read_bytes = range.end() - range.start();
        Ok(AsyncReadBody {
            reader: ReaderStream::with_capacity(read.take(max_read_bytes), capacity),
        })
    }
}

impl<T> Body for AsyncReadBody<T>
where
    T: AsyncRead,
{
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_data(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Self::Data, Self::Error>>> {
        self.project().reader.poll_next(cx)
    }

    fn poll_trailers(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        std::task::Poll::Ready(Ok(None))
    }
}

/// Tries to parse a given range header
///
/// # Errors
///
/// if the range cannot be parsed or extends past the given file length
pub fn try_parse_range(
    header_value: &str,
    file_size: u64,
) -> Result<Vec<RangeInclusive<u64>>, RangeUnsatisfiableError> {
    http_range_header::parse_range_header(header_value)
        .and_then(|first_pass| first_pass.validate(file_size))
}

/// Builds a path from a given request string
///
/// # Errors
///
/// - If the path (after percent decoding) isn't valid utf-8
/// - If a subcomponent of the path isn't a [`std::path::Component::Normal`]
/// - If the paht contains [`std::path::Component::Prefix`], [`std::path::Component::RootDir`], or [`std::path::Component::ParentDir`] elements.
pub fn build_and_validate_path(requested_path: &str) -> Result<PathBuf, PathError> {
    // taken from https://github.com/tower-rs/tower-http/blob/d895678bd70ae894f2001d30a3499995eab874ce/tower-http/src/services/fs/serve_dir/mod.rs#L486
    let str_decoded =
        percent_decode(requested_path.trim_start_matches('/').as_ref()).decode_utf8()?;
    let path_decoded = Path::new(&*str_decoded);

    let mut path_to_file = PathBuf::with_capacity(str_decoded.len());
    for component in path_decoded.components() {
        match component {
            Component::Normal(comp) => {
                if Path::new(&comp)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                {
                    path_to_file.push(comp);
                } else {
                    return Err(PathError::SubComponentNotNormal);
                }
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(PathError::ComponentNotAllowed);
            }
        }
    }
    Ok(path_to_file)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("A sub-component of the path was not normal")]
    SubComponentNotNormal,
    #[error("A component of the path was not of the allowed types")]
    ComponentNotAllowed,
    #[error("Path not valid utf-8")]
    Utf8(
        #[from]
        #[source]
        std::str::Utf8Error,
    ),
}
