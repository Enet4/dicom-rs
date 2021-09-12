
use std::io::{self, Write, BufRead, Read};
use serde::de::DeserializeOwned;
use serde_json::{self, Deserializer};

use crate::{Error, Result};

pub fn download_multipart(
    mut reader: impl BufRead + Send, 
    mut to: impl Write + Send,
    verbose: bool
) -> Result<()> {
    // eprintln!("download dicom from {:?}", resp);
    let mut boundary = String::new();
    let mut content_type = String::new();
    let mut content_length = String::new();
    let mut empty = String::new();
    let _ = reader.read_line(&mut boundary).unwrap();
    let _ = reader.read_line(&mut content_type).unwrap();
    let _ = reader.read_line(&mut content_length).unwrap();
    let _ = reader.read_line(&mut empty).unwrap();
    let _ = reader.read_line(&mut empty).unwrap();

    let mut h = content_type.split(':');
    let _ = h.next();
    if let Some(v) = h.next() {
        let _v = v.trim();
        // eprintln!("content-type: {:?}", v);
    } else {
        return Err(Error::Weired);
    }
    let mut h = content_length.split(':');
    let _ = h.next();
    let content_length = if let Some(v) = h.next() {
        let v: usize = v.trim().parse().unwrap();
        // eprintln!("content-length: {:?}", v);
        v
    } else {
        return Err(Error::Weired);
    };

    if verbose { eprintln!("content-length: {}", content_length); }
    let mut bytes = vec![0_u8; 4096_usize];
    let mut remains = content_length;
    while remains > bytes.capacity() {
        // eprintln!("read->write: {}", bytes.len());
        reader.read_exact(&mut bytes).unwrap();
        // eprintln!("{}", bytes.len());
        to.write_all(&bytes).unwrap();
        remains -= bytes.capacity();
    }
    if remains != 0_usize {
        bytes.truncate(remains);
        // eprintln!("read->write: {}", bytes.len());
        reader.read_exact(&mut bytes).unwrap();
        to.write_all(&bytes).unwrap();
    }
    Ok(())
}

pub fn iter_json_array<T: DeserializeOwned, R: Read + Unpin>(
    mut reader: R,
) -> impl Iterator<Item = Result<T, io::Error>> {
    let mut at_start = false;
    std::iter::from_fn(move || yield_next_obj(&mut reader, &mut at_start).transpose())
}
fn yield_next_obj<T: DeserializeOwned, R: Read + Unpin>(
    mut reader: R,
    at_start: &mut bool,
) -> io::Result<Option<T>> {
    if !*at_start {
        *at_start = true;
        if read_skipping_ws(&mut reader)? == b'[' {
            // read the next char to see if the array is empty
            let peek = read_skipping_ws(&mut reader)?;
            if peek == b']' {
                Ok(None)
            } else {
                deserialize_single(io::Cursor::new([peek]).chain(reader)).map(Some)
            }
        } else {
            Err(invalid_data("`[` not found"))
        }
    } else {
        match read_skipping_ws(&mut reader)? {
            b',' => deserialize_single(reader).map(Some),
            b']' => Ok(None),
            _ => Err(invalid_data("`,` or `]` not found")),
        }
    }
}

fn read_skipping_ws(mut reader: impl Read + Unpin) -> io::Result<u8> {
    loop {
        let mut byte = 0u8;
        reader.read_exact(std::slice::from_mut(&mut byte))?;
        if !byte.is_ascii_whitespace() {
            return Ok(byte);
        }
    }
}

fn invalid_data(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn deserialize_single<T: DeserializeOwned, R: Read + Unpin>(reader: R) -> io::Result<T> {
    let next_obj = Deserializer::from_reader(reader).into_iter::<T>().next();
    match next_obj {
        Some(result) => result.map_err(Into::into),
        None => Err(invalid_data("premature EOF")),
    }
}
