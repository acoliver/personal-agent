//! SSE stream normalization for providers with non-standard formatting.
//!
//! Some providers (notably Kimi) send SSE chunks as `data:{json}` without the
//! space after `data:` that the SSE specification requires. The `serdes-ai`
//! stream parser expects `data: {json}` (with space).
//!
//! This module provides a `Bytes`-stream wrapper that normalizes bare `data:`
//! prefixes so every downstream parser sees spec-compliant SSE.

use bytes::{BufMut, Bytes, BytesMut};
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Wraps a `Bytes` stream and ensures every `data:` SSE line has a space
/// after the colon (i.e. `data: …`).
///
/// Maintains an internal line buffer so that `data:` prefixes split across
/// TCP chunks are handled correctly.
pub struct NormalizeSseStream<S> {
    inner: Pin<Box<S>>,
    /// Partial line leftover from the previous chunk.
    buf: BytesMut,
}

impl<S> NormalizeSseStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner: Box::pin(inner),
            buf: BytesMut::new(),
        }
    }
}

impl<S> Stream for NormalizeSseStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>>,
{
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                self.buf.put(bytes);

                // Find the last newline — everything before it contains
                // complete lines that we can normalize and emit now.
                let last_nl = self
                    .buf
                    .iter()
                    .rposition(|&b| b == b'\n')
                    .map_or(0, |i| i + 1); // include the newline

                if last_nl == 0 {
                    // No complete line yet — wait for more data.
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }

                let complete = self.buf.split_to(last_nl).freeze();
                let normalized = normalize_sse_bytes(&complete);
                Poll::Ready(Some(Ok(normalized)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                // Stream ended — flush any remaining buffer.
                if self.buf.is_empty() {
                    Poll::Ready(None)
                } else {
                    let remaining = self.buf.split().freeze();
                    let normalized = normalize_sse_bytes(&remaining);
                    Poll::Ready(Some(Ok(normalized)))
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Normalize SSE bytes: replace bare `data:` at line starts with `data: `.
///
/// The SSE spec says `data:` followed by optional space, but many parsers
/// (including serdes-ai's `OpenAIStreamParser`) only handle `data: ` (with
/// space). Kimi sends `data:{json}` without the space.
fn normalize_sse_bytes(bytes: &Bytes) -> Bytes {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.clone();
    };

    // Fast path: if there's no bare `data:` without trailing space, return as-is.
    if !text.contains("data:") || !needs_normalization(text) {
        return bytes.clone();
    }

    let mut result = String::with_capacity(text.len() + 32);
    for line in text.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if rest.starts_with(' ') {
                result.push_str(line);
            } else {
                // Missing space: `data:{…}` → `data: {…}`
                result.push_str("data: ");
                result.push_str(rest);
            }
        } else {
            result.push_str(line);
        }
    }

    Bytes::from(result)
}

/// Check if any `data:` line is missing the trailing space.
fn needs_normalization(text: &str) -> bool {
    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix("data:") {
            if !rest.is_empty() && !rest.starts_with(' ') {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_bare_data_prefix() {
        let input = Bytes::from("data:{\"id\":\"123\"}\n\n");
        let output = normalize_sse_bytes(&input);
        assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            "data: {\"id\":\"123\"}\n\n"
        );
    }

    #[test]
    fn preserve_correct_data_prefix() {
        let input = Bytes::from("data: {\"id\":\"123\"}\n\n");
        let output = normalize_sse_bytes(&input);
        assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            "data: {\"id\":\"123\"}\n\n"
        );
    }

    #[test]
    fn normalize_done_already_correct() {
        let input = Bytes::from("data: [DONE]\n\n");
        let output = normalize_sse_bytes(&input);
        assert_eq!(std::str::from_utf8(&output).unwrap(), "data: [DONE]\n\n");
    }

    #[test]
    fn normalize_mixed_formats() {
        let input = Bytes::from("data:{\"chunk\":1}\n\ndata: [DONE]\n\n");
        let output = normalize_sse_bytes(&input);
        assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            "data: {\"chunk\":1}\n\ndata: [DONE]\n\n"
        );
    }

    #[test]
    fn no_data_lines_passthrough() {
        let input = Bytes::from(": keep-alive\n\n");
        let output = normalize_sse_bytes(&input);
        assert_eq!(std::str::from_utf8(&output).unwrap(), ": keep-alive\n\n");
    }

    #[test]
    fn empty_data_line_preserved() {
        let input = Bytes::from("data:\n\n");
        let output = normalize_sse_bytes(&input);
        assert_eq!(std::str::from_utf8(&output).unwrap(), "data:\n\n");
    }

    #[test]
    fn kimi_style_sse_chunk_normalized() {
        let input = Bytes::from(
            "data:{\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\
             \"created\":1,\"model\":\"kimi-for-coding\",\"choices\":[{\"index\":0,\
             \"delta\":{\"reasoning_content\":\"hello\"},\"finish_reason\":null}]}\n\n",
        );
        let output = normalize_sse_bytes(&input);
        let text = std::str::from_utf8(&output).unwrap();
        assert!(
            text.starts_with("data: {"),
            "should start with 'data: {{': {text}"
        );
    }

    #[tokio::test]
    async fn stream_handles_split_data_prefix() {
        use futures::StreamExt;

        // Simulate `data:` split across two chunks: "dat" | "a:{json}\n\n..."
        let chunks: Vec<Result<Bytes, reqwest::Error>> = vec![
            Ok(Bytes::from("dat")),
            Ok(Bytes::from("a:{\"x\":1}\n\ndata: [DONE]\n\n")),
        ];
        let inner = futures::stream::iter(chunks);
        let mut stream = NormalizeSseStream::new(inner);

        let mut collected = String::new();
        while let Some(Ok(bytes)) = stream.next().await {
            collected.push_str(std::str::from_utf8(&bytes).unwrap());
        }

        assert_eq!(
            collected, "data: {\"x\":1}\n\ndata: [DONE]\n\n",
            "split data: prefix should be normalized"
        );
    }
}
