#![feature(const_for)]
#![allow(overflowing_literals)]

use flate2::{
    read::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use lazy_static::lazy_static;
use regex::Regex;
use std::io::Read;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SaveError {
    #[error("save string not in a known format")]
    InvalidSaveString,
    #[error("save data not valid base64")]
    InvalidBase64,
    #[error("save data compression error")]
    CompressError(#[from] std::io::Error),
}

/// Key for the vigenere cipher
const CIPHER_KEY: &[u8] = b"therealmisalie";

/// Decodes a save into raw binary data which can then be parsed.
///
/// # Example
/// ```
/// # use savecodec::decode_to_raw;
/// assert_eq!(decode_to_raw("$00seJwrLi0GAAK5AVw=$e").unwrap(), vec![7, 29, 22]);
///
/// let save = std::fs::read_to_string("../save.txt").unwrap();
/// assert!(decode_to_raw(&save).is_ok());
/// ```
pub fn decode_to_raw(save: &str) -> Result<Vec<u8>, SaveError> {
    lazy_static! {
        /// Regex to extract save version (first group) and save data (second group) from the string
        static ref SAVE_REGEX: Regex = Regex::new(r"^\$([0-9]{2})s(.*)\$e$").unwrap();
    }

    // extract save data from save string, and then decode to byte array
    let data = &SAVE_REGEX
        .captures(save)
        .ok_or(SaveError::InvalidSaveString)?[2];
    let data = base64::decode(data).or(Err(SaveError::InvalidBase64))?;

    // then inflate with zlib
    let mut decoder = ZlibDecoder::new(&data[..]);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(SaveError::CompressError)?;

    // finally apply vigenere cipher with known key to get the raw save data in a usable form
    out.iter_mut()
        .zip(CIPHER_KEY.iter().cycle())
        .for_each(|(byte, key)| *byte ^= key);
    Ok(out)
}

/// Encodes raw binary data into an RG save
///
/// # Example
/// ```
/// # use savecodec::encode_from_raw;
/// assert_eq!(encode_from_raw(&[7, 29, 22], 0).unwrap(), "$00seJwrLi0GAAK5AVw=$e");
/// ```
pub fn encode_from_raw(data: &[u8], version: u16) -> Result<String, SaveError> {
    // encrypt with vigenere cipher first
    let data: Vec<u8> = data
        .iter()
        .zip(CIPHER_KEY.iter().cycle())
        .map(|(byte, key)| byte ^ key)
        .collect();

    // then deflate with zlib
    let mut encoder = ZlibEncoder::new(&data[..], Compression::new(6));
    let mut out = Vec::new();
    encoder
        .read_to_end(&mut out)
        .map_err(SaveError::CompressError)?;

    // then base64 encoding
    let data = base64::encode(out);

    // and finally put in format save expects
    Ok(format!("${version:02}s{data}$e"))
}
