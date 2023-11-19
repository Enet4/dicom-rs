use dicom_core::{DataDictionary, Tag};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

// re-export from dicom_parser
pub use dicom_parser::dataset::read::OddLengthStrategy;

use crate::{DefaultDicomObject, ReadError};
use std::io::Read;
use std::path::Path;

pub type Result<T, E = ReadError> = std::result::Result<T, E>;

/// Create a DICOM object by reading from a byte source.
///
/// This function assumes the standard file encoding structure without the
/// preamble: file meta group, followed by the rest of the data set.
pub fn from_reader<F>(file: F) -> Result<DefaultDicomObject>
where
    F: Read + 'static,
{
    OpenFileOptions::new().from_reader(file)
}

/// Create a DICOM object by reading from a file.
///
/// This function assumes the standard file encoding structure: 128-byte
/// preamble, file meta group, and the rest of the data set.
pub fn open_file<P>(path: P) -> Result<DefaultDicomObject>
where
    P: AsRef<Path>,
{
    OpenFileOptions::new().open_file(path)
}

/// A builder type for opening a DICOM file with additional options.
///
/// This builder exposes additional properties
/// to configure the reading of a DICOM file.
///
/// # Example
///
/// Create a `OpenFileOptions`,
/// call adaptor methods in a chain,
/// and finish the operation with
/// either [`open_file()`](OpenFileOptions::open_file)
/// or [`from_reader()`](OpenFileOptions::from_reader).
///
/// ```no_run
/// # use dicom_object::OpenFileOptions;
/// let file = OpenFileOptions::new()
///     .read_until(dicom_dictionary_std::tags::PIXEL_DATA)
///     .open_file("path/to/file.dcm")?;
/// # Result::<(), Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct OpenFileOptions<D = StandardDataDictionary, T = TransferSyntaxRegistry> {
    data_dictionary: D,
    ts_index: T,
    read_until: Option<Tag>,
    read_preamble: ReadPreamble,
    odd_length: OddLengthStrategy,
}

impl OpenFileOptions {
    pub fn new() -> Self {
        OpenFileOptions::default()
    }
}

impl<D, T> OpenFileOptions<D, T> {
    /// Set the operation to read only until the given tag is found.
    ///
    /// The reading process ends immediately after this tag,
    /// or any other tag that is next in the standard DICOM tag ordering,
    /// is found in the object's root data set.
    /// An element with the exact tag will be excluded from the output.
    pub fn read_until(mut self, tag: Tag) -> Self {
        self.read_until = Some(tag);
        self
    }

    /// Set the operation to read all elements of the data set to the end.
    ///
    /// This is the default behavior.
    pub fn read_all(mut self) -> Self {
        self.read_until = None;
        self
    }

    /// Set whether to read the 128-byte DICOM file preamble.
    pub fn read_preamble(mut self, option: ReadPreamble) -> Self {
        self.read_preamble = option;
        self
    }

    /// Set how data elements with an odd length should be handled.
    pub fn odd_length_strategy(mut self, option: OddLengthStrategy) -> Self {
        self.odd_length = option;
        self
    }

    /// Set the transfer syntax index to use when reading the file.
    pub fn transfer_syntax_index<Tr>(self, ts_index: Tr) -> OpenFileOptions<D, Tr>
    where
        Tr: TransferSyntaxIndex,
    {
        OpenFileOptions {
            data_dictionary: self.data_dictionary,
            read_until: self.read_until,
            read_preamble: self.read_preamble,
            ts_index,
            odd_length: self.odd_length,
        }
    }

    /// Set the transfer syntax index to use when reading the file.
    #[deprecated(since="0.8.1", note="please use `transfer_syntax_index` instead")]
    pub fn tranfer_syntax_index<Tr>(self, ts_index: Tr) -> OpenFileOptions<D, Tr>
    where
        Tr: TransferSyntaxIndex,
    {
        self.transfer_syntax_index(ts_index)
    }

    /// Set the data element dictionary to use when reading the file.
    pub fn dictionary<Di>(self, dict: Di) -> OpenFileOptions<Di, T>
    where
        Di: DataDictionary,
        Di: Clone,
    {
        OpenFileOptions {
            data_dictionary: dict,
            read_until: self.read_until,
            read_preamble: self.read_preamble,
            ts_index: self.ts_index,
            odd_length: self.odd_length,
        }
    }

    /// Open the file at the given path.
    pub fn open_file<P>(self, path: P) -> Result<DefaultDicomObject<D>>
    where
        P: AsRef<Path>,
        D: DataDictionary,
        D: Clone,
        T: TransferSyntaxIndex,
    {
        DefaultDicomObject::open_file_with_all_options(
            path,
            self.data_dictionary,
            self.ts_index,
            self.read_until,
            self.read_preamble,
            self.odd_length,
        )
    }

    /// Obtain a DICOM object by reading from a byte source.
    ///
    /// This method assumes
    /// the standard file encoding structure without the preamble:
    /// file meta group, followed by the rest of the data set.
    pub fn from_reader<'s: 'static, R: 's>(self, from: R) -> Result<DefaultDicomObject<D>>
    where
        R: Read,
        D: DataDictionary,
        D: Clone,
        T: TransferSyntaxIndex,
    {
        DefaultDicomObject::from_reader_with_all_options(
            from,
            self.data_dictionary,
            self.ts_index,
            self.read_until,
            self.read_preamble,
            self.odd_length,
        )
    }
}

/// An enumerate of supported options for
/// whether to read the 128-byte DICOM file preamble.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
pub enum ReadPreamble {
    /// Try to detect the presence of the preamble automatically.
    /// If detection fails, it will revert to always reading the preamble
    /// when opening a file by path,
    /// and not reading it when reading from a byte source.
    #[default]
    Auto,
    /// Never read the preamble,
    /// thus assuming that the original source does not have it.
    Never,
    /// Always read the preamble first,
    /// thus assuming that the original source always has it.
    Always,
}
