use std::{env, fs};

use txtview::TxtView;

const MAX_BYTES: u64 = 256 * 1024 * 1024;

fn main() -> std::io::Result<()> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| std::io::Error::other("usage: view_file <path>"))?;

    let len = fs::metadata(&path)
        .map_err(|e| std::io::Error::other(format!("{}: {}", path, e)))?
        .len();
    if len > MAX_BYTES {
        return Err(std::io::Error::other(format!(
            "{}: {} bytes is over the {} byte viewing limit",
            path, len, MAX_BYTES
        )));
    }
    let bytes = fs::read(&path).map_err(|e| std::io::Error::other(format!("{}: {}", path, e)))?;
    let text = decode(&bytes).map_err(|e| std::io::Error::other(format!("{}: {}", path, e)))?;

    let mut viewer = TxtView::new(text);
    viewer.run()
}

/// Decode a file by its byte order mark, otherwise as UTF-8. UTF-16 and
/// UTF-32 marks are told apart, so no guessing is needed there. Only a
/// BOM-less file with NUL bytes hints at UTF-16 and the endianness falls
/// out of where the first NUL sits. Malformed input is an error.
fn decode(bytes: &[u8]) -> Result<String, String> {
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => utf8(rest),
        [0xFF, 0xFE, 0x00, 0x00, rest @ ..] => utf32(rest, true),
        [0x00, 0x00, 0xFE, 0xFF, rest @ ..] => utf32(rest, false),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ if bytes.contains(&0) => match bytes {
            [0, _, ..] => utf16(bytes, false),
            [_, 0, ..] => utf16(bytes, true),
            _ => utf8(bytes),
        },
        _ => utf8(bytes),
    }
}

fn utf8(bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| "not valid UTF-8".to_string())
}

fn utf16(bytes: &[u8], little: bool) -> Result<String, String> {
    let (units, tail) = bytes.as_chunks::<2>();
    if !tail.is_empty() {
        return Err("the file ends in the middle of a UTF-16 unit".to_string());
    }
    let units = units
        .iter()
        .map(|&pair| {
            if little {
                u16::from_le_bytes(pair)
            } else {
                u16::from_be_bytes(pair)
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| "unpaired UTF-16 surrogate".to_string())
}

fn utf32(bytes: &[u8], little: bool) -> Result<String, String> {
    let (quads, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return Err("the file ends in the middle of a UTF-32 unit".to_string());
    }
    let mut out = String::new();
    for &quad in quads {
        let scalar = if little {
            u32::from_le_bytes(quad)
        } else {
            u32::from_be_bytes(quad)
        };
        out.push(char::from_u32(scalar).ok_or("a code point past U+10FFFF")?);
    }
    Ok(out)
}
