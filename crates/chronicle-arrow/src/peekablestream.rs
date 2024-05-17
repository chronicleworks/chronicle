use std::pin::Pin;

use arrow_flight::FlightData;
use futures::{Stream, stream::Peekable, StreamExt};
use tonic::{Status, Streaming};

pub struct PeekableFlightDataStream {
    inner: Peekable<Streaming<FlightData>>,
}

impl PeekableFlightDataStream {
    pub fn new(stream: Streaming<FlightData>) -> Self {
        Self { inner: stream.peekable() }
    }

    /// Convert this stream into a `Streaming<FlightData>`.
    /// Any messages observed through [`Self::peek`] will be lost
    /// after the conversion.
    pub fn into_inner(self) -> Streaming<FlightData> {
        self.inner.into_inner()
    }

    /// Convert this stream into a `Peekable<Streaming<FlightData>>`.
    /// Preserves the state of the stream, so that calls to [`Self::peek`]
    /// and [`Self::poll_next`] are the same.
    pub fn into_peekable(self) -> Peekable<Streaming<FlightData>> {
        self.inner
    }

    /// Peek at the head of this stream without advancing it.
    pub async fn peek(&mut self) -> Option<&Result<FlightData, Status>> {
        Pin::new(&mut self.inner).peek().await
    }
}

impl Stream for PeekableFlightDataStream {
    type Item = Result<FlightData, Status>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}
