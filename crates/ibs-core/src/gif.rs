//! Byte-level GIF metadata stripper.
//!
//! Removes comment, plain-text, XMP and ICC extension blocks from a GIF87a /
//! GIF89a stream without decoding pixels, so animation frames, frame delays
//! and the NETSCAPE loop extension survive untouched. Used when a batch only
//! strips metadata; decoding to a bitmap would collapse the animation to its
//! first frame.

use crate::error::{FileError, FileErrorCode};

/// Header (6) + Logical Screen Descriptor (7).
const LSD_END: usize = 13;

fn malformed() -> FileError {
    FileError::new(FileErrorCode::Decode, "malformed GIF stream")
}

/// Walks a chain of `len byte + len bytes` data sub-blocks starting at `pos`
/// and returns the index just past the terminating zero-length block.
fn skip_sub_blocks(src: &[u8], mut i: usize) -> Result<usize, FileError> {
    loop {
        let n = *src.get(i).ok_or_else(malformed)? as usize;
        i += 1 + n;
        if i > src.len() {
            return Err(malformed());
        }
        if n == 0 {
            return Ok(i);
        }
    }
}

/// The bytes of the first sub-block at `pos` (extension header payload).
fn first_block(src: &[u8], i: usize) -> Result<&[u8], FileError> {
    let n = *src.get(i).ok_or_else(malformed)? as usize;
    src.get(i + 1..i + 1 + n).ok_or_else(malformed)
}

/// `src` must be a GIF stream; anything else or a truncated stream errors.
/// Returns the stream minus metadata blocks.
pub fn strip_metadata(src: &[u8]) -> Result<Vec<u8>, FileError> {
    if src.len() < LSD_END || !src.starts_with(b"GIF8") {
        return Err(malformed());
    }
    let mut out = Vec::with_capacity(src.len());
    out.extend_from_slice(&src[..LSD_END]);
    let mut i = LSD_END;
    // Global color table, if the LSD flags one.
    if src[10] & 0x80 != 0 {
        let n = 3 * (1usize << ((src[10] & 0x07) + 1));
        if i + n > src.len() {
            return Err(malformed());
        }
        out.extend_from_slice(&src[i..i + n]);
        i += n;
    }
    let mut last_gce_len = 0;
    loop {
        match *src.get(i).ok_or_else(malformed)? {
            0x3B => {
                // Trailer: stop; ignore any trailing garbage.
                out.push(0x3B);
                return Ok(out);
            }
            0x21 => {
                let label = *src.get(i + 1).ok_or_else(malformed)?;
                let drop = match label {
                    // Comment and plain-text extensions are pure metadata.
                    0xFE | 0x01 => true,
                    0xFF => {
                        // Application extension: 11-byte identifier block.
                        let id = first_block(src, i + 2)?;
                        id.starts_with(b"XMP Data") || id.starts_with(b"ICCRGBG")
                    }
                    // GCE (0xF9), unknown extensions: keep.
                    _ => false,
                };
                let end = skip_sub_blocks(src, i + 2)?;
                if !drop {
                    out.extend_from_slice(&src[i..end]);
                    // Remember if a GCE was just written: it binds to the next
                    // graphic, which may be a plain-text block we then drop.
                    last_gce_len = if label == 0xF9 { end - i } else { 0 };
                } else if label == 0x01 && last_gce_len > 0 {
                    // Drop the bound GCE too or it would rebind to the
                    // following image and change its delay/disposal.
                    out.truncate(out.len() - last_gce_len);
                    last_gce_len = 0;
                }
                // Comments/app extensions don't consume a pending GCE.
                i = end;
            }
            0x2C => {
                // Image descriptor: consumes any pending GCE, then separator
                // + 9 bytes, optional local color table, 1-byte LZW minimum
                // code size, then data sub-blocks.
                last_gce_len = 0;
                let packed = *src.get(i + 9).ok_or_else(malformed)?;
                let lct = if packed & 0x80 != 0 {
                    3 * (1usize << ((packed & 0x07) + 1))
                } else {
                    0
                };
                let data = i + 10 + lct;
                if data + 1 > src.len() {
                    return Err(malformed());
                }
                let end = skip_sub_blocks(src, data + 1)?;
                out.extend_from_slice(&src[i..end]);
                i = end;
            }
            _ => return Err(malformed()),
        }
    }
}

/// Logical screen size, read straight from the LSD (little-endian).
pub fn screen_size(src: &[u8]) -> Option<(u32, u32)> {
    if src.len() < LSD_END || !src.starts_with(b"GIF8") {
        return None;
    }
    Some((
        u16::from_le_bytes([src[6], src[7]]) as u32,
        u16::from_le_bytes([src[8], src[9]]) as u32,
    ))
}
