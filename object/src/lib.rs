//! This crate contains a high-level abstraction for reading and manipulating
//! DICOM objects.
//! At this level, objects are comparable to a dictionary of elements,
//! in which some of them can have DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM
//! objects.
//!
//! # Examples
//!
//! Loading a DICOM file and reading some attributes by their standard alias:
//!
//! ```no_run
//! use dicom_object::open_file;
//! # fn foo() -> Result<(), Box<dyn std::error::Error>> {
//! let obj = open_file("0001.dcm")?;
//! let patient_name = obj.element_by_name("PatientName")?.to_str()?;
//! let modality = obj.element_by_name("Modality")?.to_str()?;
//! # Ok(())
//! # }
//! ```
//!
//! Elements can also be fetched by tag.
//! Methods are available for converting the element's DICOM value
//! into something more usable in Rust.
//!
//! ```
//! # use dicom_object::{DefaultDicomObject, Tag};
//! # fn something(obj: DefaultDicomObject) -> Result<(), Box<dyn std::error::Error>> {
//! let patient_date = obj.element(Tag(0x0010, 0x0030))?.to_date()?;
//! let pixel_data_bytes = obj.element(Tag(0x7FE0, 0x0010))?.to_bytes()?;
//! # Ok(())
//! # }
//! ```
//!
//! Finally, DICOM objects can be written back into a file.
//!
//! ```no_run
//! # use dicom_object::{DefaultDicomObject, Tag};
//! # fn something(obj: DefaultDicomObject) -> Result<(), Box<dyn std::error::Error>> {
//! obj.write_to_file("0001_new.dcm")?;
//! # Ok(())
//! # }
//! ```
//!
pub mod file;
pub mod loader;
pub mod mem;
pub mod meta;
pub mod pixeldata;
pub mod tokens;

mod util;

pub use crate::file::{from_reader, open_file};
pub use crate::meta::FileMetaTable;
pub use dicom_core::Tag;
pub use dicom_dictionary_std::StandardDataDictionary;

/// The default implementation of a root DICOM object.
pub type DefaultDicomObject = RootDicomObject<mem::InMemDicomObject<StandardDataDictionary>>;

use dicom_core::header::Header;
use dicom_encoding::{text::SpecificCharacterSet, transfer_syntax::TransferSyntaxIndex};
use dicom_parser::dataset::{DataSetWriter, IntoTokens};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Trait type for a DICOM object.
/// This is a high-level abstraction where an object is accessed and
/// manipulated as dictionary of entries indexed by tags, which in
/// turn may contain a DICOM object.
///
/// This trait interface is experimental and prone to sudden changes.
pub trait DicomObject {
    type Element: Header;

    /// Retrieve a particular DICOM element by its tag.
    fn element(&self, tag: Tag) -> Result<Self::Element>;

    /// Retrieve a particular DICOM element by its name.
    fn element_by_name(&self, name: &str) -> Result<Self::Element>;

    /// Retrieve the processed meta information table, if available.
    ///
    /// This table will generally not be reachable from children objects
    /// in another object with a valid meta table. As such, it is recommended
    /// for this method to be called at the root of a DICOM object.
    fn meta(&self) -> Option<&FileMetaTable> {
        None
    }
}

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("Could not open file '{}': {}", filename.display(), source))]
    OpenFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not read from file '{}': {}", filename.display(), source))]
    ReadFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not parse meta group data set"))]
    ParseMetaDataSet {
        #[snafu(backtrace)]
        source: crate::meta::Error,
    },
    #[snafu(display("Could not create data set parser"))]
    CreateParser {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::read::Error,
    },
    #[snafu(display("Could not read data set token"))]
    ReadToken {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::read::Error,
    },
    #[snafu(display("Could not write to file '{}'", filename.display()))]
    WriteFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not write object preamble"))]
    WritePreamble {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not write magic code"))]
    WriteMagicCode {
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not create data set printer"))]
    CreatePrinter {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::write::Error,
    },
    #[snafu(display("Could not print meta group data set"))]
    PrintMetaDataSet {
        #[snafu(backtrace)]
        source: crate::meta::Error,
    },
    #[snafu(display("Could not print data set"))]
    PrintDataSet {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::write::Error,
    },
    #[snafu(display("Unsupported transfer syntax `{}`", uid))]
    UnsupportedTransferSyntax { uid: String, backtrace: Backtrace },
    #[snafu(display("No such data element with tag {}", tag))]
    NoSuchDataElementTag { tag: Tag, backtrace: Backtrace },
    #[snafu(display("No such data element {} (with tag {})", alias, tag))]
    NoSuchDataElementAlias {
        tag: Tag,
        alias: String,
        backtrace: Backtrace,
    },
    #[snafu(display("Unknown data attribute named `{}`", name))]
    NoSuchAttributeName { name: String, backtrace: Backtrace },
    #[snafu(display("Missing element value"))]
    MissingElementValue { backtrace: Backtrace },
    #[snafu(display("Unexpected token {:?}", token))]
    UnexpectedToken {
        token: dicom_parser::dataset::DataToken,
        backtrace: Backtrace,
    },
    #[snafu(display("Premature data set end"))]
    PrematureEnd { backtrace: Backtrace },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/** A root DICOM object contains additional meta information about the object
 * (such as the DICOM file's meta header).
 */
#[derive(Debug, Clone, PartialEq)]
pub struct RootDicomObject<T> {
    meta: FileMetaTable,
    obj: T,
}

impl<T> RootDicomObject<T> {
    /// Retrieve the processed meta header table.
    pub fn meta(&self) -> &FileMetaTable {
        &self.meta
    }

    /// Retrieve the inner DICOM object structure, discarding the meta table.
    pub fn into_inner(self) -> T {
        self.obj
    }
}

impl<T> RootDicomObject<T>
where
    for<'a> &'a T: IntoTokens,
{
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let file = File::create(path).context(WriteFile { filename: path })?;
        let mut to = BufWriter::new(file);

        // write preamble
        to.write_all(&[0_u8; 128][..])
            .context(WriteFile { filename: path })?;

        // write magic sequence
        to.write_all(b"DICM")
            .context(WriteFile { filename: path })?;

        // write meta group
        self.meta.write(&mut to).context(PrintMetaDataSet)?;

        // prepare encoder
        let registry = TransferSyntaxRegistry::default();
        let ts = registry.get(&self.meta.transfer_syntax).with_context(|| {
            UnsupportedTransferSyntax {
                uid: self.meta.transfer_syntax.clone(),
            }
        })?;
        let cs = SpecificCharacterSet::Default;
        let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinter)?;

        // write object
        dset_writer
            .write_sequence((&self.obj).into_tokens())
            .context(PrintDataSet)?;

        Ok(())
    }

    /// Write the entire object as a DICOM file
    /// into the given writer.
    pub fn write_all<W: Write>(&self, to: W) -> Result<()> {
        let mut to = BufWriter::new(to);

        // write preamble
        to.write_all(&[0_u8; 128][..]).context(WritePreamble)?;

        // write magic sequence
        to.write_all(b"DICM").context(WriteMagicCode)?;

        // write meta group
        self.meta.write(&mut to).context(PrintMetaDataSet)?;

        // prepare encoder
        let registry = TransferSyntaxRegistry::default();
        let ts = registry.get(&self.meta.transfer_syntax).with_context(|| {
            UnsupportedTransferSyntax {
                uid: self.meta.transfer_syntax.clone(),
            }
        })?;
        let cs = SpecificCharacterSet::Default;
        let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinter)?;

        // write object
        dset_writer
            .write_sequence((&self.obj).into_tokens())
            .context(PrintDataSet)?;

        Ok(())
    }

    /// Write the inner data set into the given writer,
    /// without preamble, magic code, nor file meta group.
    ///
    /// The transfer syntax is selected from the file meta table.
    pub fn write_dataset<W: Write>(&self, to: W) -> Result<()> {
        let to = BufWriter::new(to);

        // prepare encoder
        let registry = TransferSyntaxRegistry::default();
        let ts = registry.get(&self.meta.transfer_syntax).with_context(|| {
            UnsupportedTransferSyntax {
                uid: self.meta.transfer_syntax.clone(),
            }
        })?;
        let cs = SpecificCharacterSet::Default;
        let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinter)?;

        // write object
        dset_writer
            .write_sequence((&self.obj).into_tokens())
            .context(PrintDataSet)?;

        Ok(())
    }
}

impl<T> ::std::ops::Deref for RootDicomObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<T> ::std::ops::DerefMut for RootDicomObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}

impl<T> DicomObject for RootDicomObject<T>
where
    T: DicomObject,
{
    type Element = <T as DicomObject>::Element;

    fn element(&self, tag: Tag) -> Result<Self::Element> {
        self.obj.element(tag)
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        self.obj.element_by_name(name)
    }

    fn meta(&self) -> Option<&FileMetaTable> {
        Some(&self.meta)
    }
}

impl<'a, T: 'a> DicomObject for &'a RootDicomObject<T>
where
    T: DicomObject,
{
    type Element = <T as DicomObject>::Element;

    fn element(&self, tag: Tag) -> Result<Self::Element> {
        self.obj.element(tag)
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        self.obj.element_by_name(name)
    }
}

impl<T> IntoIterator for RootDicomObject<T>
where
    T: IntoIterator,
{
    type Item = <T as IntoIterator>::Item;
    type IntoIter = <T as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.obj.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::meta::FileMetaTableBuilder;
    use crate::RootDicomObject;

    #[test]
    fn smoke_test() {
        const FILE_NAME: &str = ".smoke-test.dcm";

        let meta = FileMetaTableBuilder::new()
            .transfer_syntax(
                dicom_transfer_syntax_registry::entries::EXPLICIT_VR_LITTLE_ENDIAN.uid(),
            )
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
            .media_storage_sop_instance_uid("1.2.3.456")
            .implementation_class_uid("1.2.345.6.7890.1.234")
            .build()
            .unwrap();
        let obj = RootDicomObject::new_empty_with_meta(meta);

        obj.write_to_file(FILE_NAME).unwrap();

        let obj2 = RootDicomObject::open_file(FILE_NAME).unwrap();

        assert_eq!(obj, obj2);

        let _ = std::fs::remove_file(FILE_NAME);
    }
}
