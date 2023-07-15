use bytes::Bytes;
use futures::Stream;
use http_body::Body;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncReadExt, Take};
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
    /// Create a new [`AsyncReadBody`] wrapping the given reader,
    /// with a specific read buffer capacity
    pub fn with_capacity(read: T, capacity: usize) -> Self {
        Self {
            reader: ReaderStream::with_capacity(read, capacity),
        }
    }

    pub fn with_capacity_limited(
        read: T,
        capacity: usize,
        max_read_bytes: u64,
    ) -> AsyncReadBody<Take<T>> {
        AsyncReadBody {
            reader: ReaderStream::with_capacity(read.take(max_read_bytes), capacity),
        }
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
