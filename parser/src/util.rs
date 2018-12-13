use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::ops::{DerefMut, Range};

/** A private type trait for the ability to efficiently implement stream skipping.
 */
pub trait ForwardSeek {
    fn skip(&mut self, n: u64) -> io::Result<u64>;
}

impl<S: ?Sized> ForwardSeek for S
where
    S: Seek,
{
    fn skip(&mut self, n: u64) -> io::Result<u64> {
        let curr_pos = try!(self.seek(SeekFrom::Current(0)));
        let new_pos = try!(self.seek(SeekFrom::Current(n as i64)));
        Ok(new_pos - curr_pos)
    }
}

pub trait ReadSeek: Read + Seek {}
impl<T: ?Sized> ReadSeek for T
where
    T: Read + Seek,
{
}

#[derive(Debug)]
pub struct SeekInterval<S: ?Sized, B: DerefMut<Target = S>> {
    source: B,
    current: u64,
    begin: u64,
    end: u64,
    marker: PhantomData<S>,
}

impl<S: ?Sized, B> SeekInterval<S, B>
where
    S: Seek,
    B: DerefMut<Target = S>,
{
    /// Create an interval from the current position and ending
    /// after `n` bytes.
    pub fn new_here(mut source: B, n: u32) -> io::Result<Self> {
        let pos = try!(source.seek(SeekFrom::Current(0)));
        Ok(SeekInterval {
            source: source,
            current: pos,
            begin: pos,
            end: pos + u64::from(n),
            marker: PhantomData,
        })
    }

    /// Create an interval that starts and ends according to the given
    /// range of bytes.
    pub fn new_at(mut source: B, range: Range<u64>) -> io::Result<Self> {
        let pos = try!(source.seek(SeekFrom::Start(range.start)));
        Ok(SeekInterval {
            source: source,
            current: pos,
            begin: pos,
            end: range.end,
            marker: PhantomData,
        })
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        (self.end - self.current) as usize
    }
}

impl<S: ?Sized, B> Seek for SeekInterval<S, B>
where
    S: Seek,
    B: DerefMut<Target = S>,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(o) => self.source
                .seek(SeekFrom::Start(self.begin + o))
                .map(|v| v - self.begin),
            pos @ SeekFrom::Current(_) => self.source.seek(pos).map(|v| v - self.begin),
            SeekFrom::End(o) => self.source
                .seek(SeekFrom::Start((self.end as i64 + o) as u64))
                .map(|v| v - self.begin),
        }
    }
}

impl<S: ?Sized + Seek, B> Read for SeekInterval<S, B>
where
    S: Read,
    B: DerefMut<Target = S>,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let r = self.remaining();
        let buf = if buf.len() > r { &mut buf[0..r] } else { buf };

        self.source.read(buf)
    }
}

impl<S: ?Sized, B> Write for SeekInterval<S, B>
where
    S: Write + Seek,
    B: DerefMut<Target = S>,
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

/// Obtain an iterator of `n` void elements.
/// Useful for doing something N times as efficiently as possible.
pub fn n_times(n: usize) -> VoidRepeatN {
    VoidRepeatN { i: n }
}

pub struct VoidRepeatN {
    i: usize,
}

impl Iterator for VoidRepeatN {
    type Item = ();

    fn next(&mut self) -> Option<()> {
        match self.i {
            0 => None,
            _ => {
                self.i -= 1;
                Some(())
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.i, Some(self.i))
    }
}

impl ExactSizeIterator for VoidRepeatN {
    fn len(&self) -> usize {
        self.i
    }
}

#[cfg(test)]
mod tests {
    use super::n_times;
    use super::SeekInterval;
    use std::io::{Cursor, Write};

    #[test]
    fn void_repeat_n() {
        let it = n_times(5);
        assert_eq!(it.len(), 5);
        let mut k = 0;
        for v in it {
            assert_eq!(v, ());
            k += 1;
        }
        assert_eq!(k, 5);
        let mut it = n_times(0);
        assert_eq!(it.len(), 0);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn seek_interval_writing_here() {
        let mut buf = Cursor::new(vec![0xFFu8; 8]);
        {
            let mut interval = SeekInterval::new_here(&mut buf, 5).unwrap();
            let count = interval.write(&vec![0; 8]).unwrap();
            assert_eq!(count, 5);
        }
        assert_eq!(
            buf.into_inner(),
            vec![0, 0, 0, 0, 0, 0xFFu8, 0xFFu8, 0xFFu8]
        )
    }

    #[test]
    fn seek_interval_writing() {
        let mut buf = Cursor::new(vec![0xFFu8; 8]);
        {
            let mut interval = SeekInterval::new_at(&mut buf, 2..8).unwrap();
            let count = interval.write(&vec![0; 8]).unwrap();
            assert_eq!(count, 6);
        }
        assert_eq!(buf.into_inner(), vec![0xFFu8, 0xFF, 0, 0, 0, 0, 0, 0])
    }
}
