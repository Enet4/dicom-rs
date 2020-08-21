use dicom_core::header::{DataElementHeader, Header, HasLength, VR, Length, Tag};
use crate::util::{SeekInterval, ReadSeek};
use std::ops::DerefMut;
use std::io::{Seek, SeekFrom};
use snafu::{OptionExt, ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Unknown value length"))]
    UnknownValueLength,
    CreateInterval {
        source: std::io::Error,
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A data type for a DICOM element residing in a file, or any other source
/// with random access. A position in the file is kept for future access.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct DicomElementMarker {
    /// The header, kept in memory. At this level, the value representation
    /// "UN" may also refer to a non-applicable vr (i.e. for items and
    /// delimiters).
    pub header: DataElementHeader,
    /// The ending position of the element's header (or the starting position
    /// of the element's value if it exists), relative to the beginning of the
    /// file.
    pub pos: u64,
}

impl DicomElementMarker {
    /// Obtain an interval of the raw data associated to this element's data value.
    pub fn get_data_stream<S: ?Sized, B: DerefMut<Target = S>>(
        &self,
        source: B,
    ) -> Result<SeekInterval<S, B>>
    where
        S: ReadSeek,
    {
        let len = u64::from(
            self.header
                .length()
                .get()
                .context(UnknownValueLength)?,
        );
        let interval = SeekInterval::new_at(source, self.pos..len)
            .context(CreateInterval)?;
        Ok(interval)
    }

    /// Move the source to the position indicated by the marker
    pub fn move_to_start<S: ?Sized, B: DerefMut<Target = S>>(
        &self,
        mut source: B,
    ) -> std::io::Result<()>
    where
        S: Seek,
    {
        source.seek(SeekFrom::Start(self.pos))?;
        Ok(())
    }

    /// Getter for this element's value representation. May be `UN`
    /// when this is not applicable.
    pub fn vr(&self) -> VR {
        self.header.vr()
    }
}

impl HasLength for DicomElementMarker {
    fn length(&self) -> Length {
        self.header.length()
    }
}
impl Header for DicomElementMarker {
    fn tag(&self) -> Tag {
        self.header.tag()
    }
}
