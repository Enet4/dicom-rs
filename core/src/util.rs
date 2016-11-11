use std::io;
use std::io::{Read, Write, Seek, SeekFrom};

#[cfg(test)]
mod tests {
    use super::{swap_bytes_16, swap_bytes_32, swap_bytes_64};

    const TEST_DATA: [u8; 8] = [0, 10, 20, 0, 100, 200, 100, 0];

    #[test]
    fn test_swap_bytes16() {
        let mut data: &mut [u8] = &mut TEST_DATA;
        swap_bytes_16(data);
        assert_eq!(data, &[10, 0, 0, 20, 200, 100, 0, 100]);
    }

    #[test]
    fn test_swap_bytes32() {
        let mut data: &mut [u8] = &mut TEST_DATA;
        swap_bytes_32(data);
        assert_eq!(data, &[0, 20, 10, 0, 0, 100, 200, 100]);
    }

    #[test]
    fn test_swap_bytes64() {
        let mut data: &mut [u8] = &mut TEST_DATA;
        swap_bytes_64(data);
        assert_eq!(data, &[0, 100, 200, 100, 0, 20, 10, 0]);
    }
}

/** A private type trait for the ability to efficiently implement stream skipping.
 */
pub trait ForwardSeek {
    fn skip(&mut self, n: u64) -> io::Result<u64>;
}

impl<S: ?Sized> ForwardSeek for S
    where S: Seek
{
    fn skip(&mut self, n: u64) -> io::Result<u64> {
        let curr_pos = try!(self.seek(SeekFrom::Current(0)));
        let new_pos = try!(self.seek(SeekFrom::Current(n as i64)));
        Ok(new_pos - curr_pos)
    }
}

pub trait ReadSeek: Read + Seek {}
impl<T: ?Sized> ReadSeek for T where T: Read + Seek {}

#[derive(Debug)]
pub struct SeekInterval<'s, S: Seek + ?Sized + 's> {
    source: &'s mut S,
    current: u64,
    begin: u64,
    end: u64,
}

impl<'s, S: Seek + ?Sized + 's> SeekInterval<'s, S> {
    pub fn new(source: &'s mut S, n: u32) -> io::Result<SeekInterval<'s, S>> {
        let pos = try!(source.seek(SeekFrom::Current(0)));
        Ok(SeekInterval {
            source: source,
            current: pos,
            begin: pos,
            end: pos + n as u64,
        })
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        (self.end - self.current) as usize
    }
}

impl<'s, S: Seek + ?Sized + 's> Seek for SeekInterval<'s, S> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(o) => {
                self.source.seek(SeekFrom::Start(self.begin + o)).map(|v| v - self.begin)
            }
            pos @ SeekFrom::Current(_) => self.source.seek(pos).map(|v| v - self.begin),
            SeekFrom::End(o) => {
                self.source
                    .seek(SeekFrom::Start((self.end as i64 + o) as u64))
                    .map(|v| v - self.begin)
            }
        }
    }
}

impl<'s, S: Seek + ?Sized + 's> Read for SeekInterval<'s, S>
    where S: Read
{
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let r = self.remaining();
        let buf = if buf.len() > r { &mut buf[0..r] } else { buf };

        self.source.read(buf)
    }
}


impl<'s, S: Seek + ?Sized + 's> Write for SeekInterval<'s, S>
    where S: Write
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let r = self.remaining();

        let buf = if buf.len() > r { &buf[0..r] } else { buf };

        self.source.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.source.flush()
    }
}

// Enumerate for the two kinds of endianness considered by the standard.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Endianness {
    /// Little Endian
    LE,
    /// Big Endian
    BE,
}

impl Endianness {
    /// Obtain this system's endianness
    #[cfg(target_endian = "little")]
    pub fn system() -> Endianness {
        Endianness::LE
    }

    /// Obtain this system's endianness
    #[cfg(target_endian = "big")]
    pub fn system() -> Endianness {
        Endianness::BE
    }
}

/// Swap the bytes of 16-bit words in place.
pub fn swap_bytes_16(data: &mut [u8]) {
    for chunk in data.chunks_mut(2) {
        chunk.reverse()
    }
}

/// Swap the bytes of 32-bit words in place.
pub fn swap_bytes_32(data: &mut [u8]) {
    for chunk in data.chunks_mut(4) {
        chunk.reverse()
    }
}

/// Swap the bytes of 64-bit words in place.
pub fn swap_bytes_64(data: &mut [u8]) {
    for chunk in data.chunks_mut(8) {
        chunk.reverse()
    }
}
