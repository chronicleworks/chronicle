use std::pin::Pin;

use arrow_flight::FlightData;
use futures::{stream::Peekable, Stream, StreamExt};
use tonic::{Status, Streaming};

/// A wrapper around [`Streaming<FlightData>`] that allows "peeking" at the
/// message at the front of the stream without consuming it.
/// This is needed because sometimes the first message in the stream will contain
/// a [`FlightDescriptor`] in addition to potentially any data, and the dispatch logic
/// must inspect this information.
///
/// # Example
///
/// [`PeekableFlightDataStream::peek`] can be used to peek at the first message without
/// discarding it; otherwise, `PeekableFlightDataStream` can be used as a regular stream.
/// See the following example:
///
/// ```no_run
/// use arrow_array::RecordBatch;
/// use arrow_flight::decode::FlightRecordBatchStream;
/// use arrow_flight::FlightDescriptor;
/// use arrow_flight::error::FlightError;
/// use PeekableFlightDataStream;
/// use tonic::{Request, Status};
/// use futures::TryStreamExt;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Status> {
///     let request: Request<PeekableFlightDataStream> = todo!();
///     let stream: PeekableFlightDataStream = request.into_inner();
///
///     // The first message contains the flight descriptor and the schema.
///     // Read the flight descriptor without discarding the schema:
///     let flight_descriptor: FlightDescriptor = stream
///         .peek()
///         .await
///         .cloned()
///         .transpose()?
///         .and_then(|data| data.flight_descriptor)
///         .expect("first message should contain flight descriptor");
///
///     // Pass the stream through a decoder
///     let batches: Vec<RecordBatch> = FlightRecordBatchStream::new_from_flight_data(
///         request.into_inner().map_err(|e| e.into()),
///     )
///     .try_collect()
///     .await?;
/// }
/// ```
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
