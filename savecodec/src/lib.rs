use binformat::format_source;
use flate2::read::ZlibDecoder;
use lazy_static::lazy_static;
use regex::Regex;
use std::io::Read;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("save string not in a known format")]
    InvalidSaveString,
    #[error("save data not valid base64")]
    InvalidBase64,
    #[error("save data inflation error")]
    InflateError(#[from] std::io::Error),
}

/// Decodes a save into raw binary data which can then be parsed.
///
/// # Example
/// ```
/// # use savecodec::decode_to_raw;
/// assert_eq!(decode_to_raw("$00seJwrLi0GAAK5AVw=$e").unwrap(), vec![7, 29, 22]);
///
/// let save = std::fs::read_to_string("save.txt").unwrap();
/// assert!(decode_to_raw(&save).is_ok());
/// ```
pub fn decode_to_raw(save: &str) -> Result<Vec<u8>, DecodeError> {
    lazy_static! {
        /// Regex to extract save version (first group) and save data (second group) from the string
        static ref SAVE_REGEX: Regex = Regex::new(r"^\$([0-9]{2})s(.*)\$e$").unwrap();
    }
    /// Key for the vigenere cipher
    const CIPHER_KEY: &[u8] = b"therealmisalie";

    // extract save data from save string, and then decode to byte array
    let data = &SAVE_REGEX
        .captures(save)
        .ok_or(DecodeError::InvalidSaveString)?[2];
    let data = base64::decode(data).or(Err(DecodeError::InvalidBase64))?;

    // then deflate with zlib
    let mut decoder = ZlibDecoder::new(&data[..]);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(DecodeError::InflateError)?;

    // finally apply vigenere cipher with known key to get the raw save data in a usable form
    out.iter_mut()
        .zip(CIPHER_KEY.iter().cycle())
        .for_each(|(byte, key)| *byte ^= key);
    Ok(out)
}
