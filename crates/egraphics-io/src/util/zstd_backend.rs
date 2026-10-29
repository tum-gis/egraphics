//! ZStandard backend selection.
//!
//! Native targets use the `zstd` crate (bindings to the C library), while
//! `wasm32` falls back to the pure Rust `ruzstd` crate, so no C toolchain
//! is required for WebAssembly builds.

use std::io::{Read, Write};

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(test), expect(dead_code, reason = "reader is not implemented yet"))]
pub(crate) fn decoder<'a, R: Read + 'a>(reader: R) -> Result<Box<dyn Read + 'a>, std::io::Error> {
    Ok(Box::new(zstd::Decoder::new(reader)?))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn encoder<'a, W: Write + 'a>(
    writer: W,
    level: i32,
) -> Result<Box<dyn Write + 'a>, std::io::Error> {
    Ok(Box::new(zstd::Encoder::new(writer, level)?.auto_finish()))
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(not(test), expect(dead_code, reason = "reader is not implemented yet"))]
pub(crate) fn decoder<'a, R: Read + 'a>(reader: R) -> Result<Box<dyn Read + 'a>, std::io::Error> {
    let decoder = ruzstd::decoding::StreamingDecoder::new(reader)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Box::new(decoder))
}

/// The compression level is ignored, as `ruzstd` only implements its fastest level
/// (roughly equivalent to ZStandard level 1).
#[cfg(target_arch = "wasm32")]
pub(crate) fn encoder<'a, W: Write + 'a>(
    writer: W,
    _level: i32,
) -> Result<Box<dyn Write + 'a>, std::io::Error> {
    Ok(Box::new(ruzstd_writer::RuzstdWriter::new(writer)))
}

#[cfg(target_arch = "wasm32")]
mod ruzstd_writer {
    use ruzstd::encoding::{CompressionLevel, compress_to_vec};
    use std::io::Write;

    /// Buffers all written data and compresses it into the inner writer when dropped,
    /// mirroring the auto-finish behavior of the native `zstd` encoder.
    ///
    /// `ruzstd` provides no streaming encoder, hence the buffering.
    pub(super) struct RuzstdWriter<W: Write> {
        inner: Option<W>,
        buffer: Vec<u8>,
    }

    impl<W: Write> RuzstdWriter<W> {
        pub(super) fn new(inner: W) -> Self {
            Self {
                inner: Some(inner),
                buffer: Vec::new(),
            }
        }

        fn finish(&mut self) -> Result<(), std::io::Error> {
            let Some(mut inner) = self.inner.take() else {
                return Ok(());
            };
            // compress into memory first, since ruzstd panics on errors of the target writer
            let compressed = compress_to_vec(self.buffer.as_slice(), CompressionLevel::Fastest);
            inner.write_all(&compressed)?;
            inner.flush()
        }
    }

    impl<W: Write> Write for RuzstdWriter<W> {
        fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
            self.buffer.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> Result<(), std::io::Error> {
            // data is only written to the inner writer once compression is finished on drop
            Ok(())
        }
    }

    impl<W: Write> Drop for RuzstdWriter<W> {
        fn drop(&mut self) {
            // errors cannot be propagated from drop, same as zstd's AutoFinishEncoder
            let _ = self.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let input = b"egraphics info document ".repeat(64);
        let mut compressed = Vec::new();
        {
            let mut encoder = encoder(&mut compressed, 3).unwrap();
            encoder.write_all(&input).unwrap();
        }
        assert!(!compressed.is_empty());

        let mut output = Vec::new();
        decoder(compressed.as_slice())
            .unwrap()
            .read_to_end(&mut output)
            .unwrap();
        assert_eq!(output, input);
    }
}
