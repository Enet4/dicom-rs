//! This crate contains a high-level abstraction for reading and manipulating
//! DICOM objects.
//! At this level, objects are comparable to a dictionary of elements,
//! in which some of them can have DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM
//! objects.
//!
//! # Examples
//!
//! Loading a file and reading some attributes by their standard alias:
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
//! Elements can also be fetched by tag:
//!
//! ```
//! # use dicom_object::{DicomObject, Tag};
//! # fn something<T: DicomObject>(obj: T) -> Result<(), Box<dyn std::error::Error>> {
//! let e = obj.element(Tag(0x0002, 0x0002))?;
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
use snafu::{Backtrace, ResultExt, Snafu};
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
pub enum Error {
    #[snafu(display("Could not open file '{}': {}", filename.display(), source))]
    OpenFile {
        filename: std::path::PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("Could not read from file '{}': {}", filename.display(), source))]
    ReadFile {
        filename: std::path::PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("Could not parse meta group data set: {}", source))]
    ParseMetaDataSet { source: crate::meta::Error },
    #[snafu(display("Could not create data set parser: {}", source))]
    CreateParser { source: dicom_parser::error::Error },
    #[snafu(display("Could not parse data set: {}", source))]
    ParseDataSet { source: dicom_parser::error::Error },
    #[snafu(display("Could not write to file '{}': {}", filename.display(), source))]
    WriteFile {
        filename: std::path::PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("Could not create data set printer: {}", source))]
    CreatePrinter { source: dicom_parser::error::Error },
    #[snafu(display("Could not print meta group data set: {}", source))]
    PrintMetaDataSet { source: crate::meta::Error },
    #[snafu(display("Could not print data set: {}", source))]
    PrintDataSet { source: dicom_parser::error::Error },
    #[snafu(display("Unsupported transfer syntax `{}`", uid))]
    UnsupportedTransferSyntax { uid: String },
    #[snafu(display("No such data element {}{}", tag, if let Some(a) = alias {
        format!(" ({})", a)
    } else {
        "".to_string()
    }))]
    NoSuchDataElement { tag: Tag, alias: Option<String>, backtrace: Backtrace },
    #[snafu(display("Unknown data attribute named `{}`", name))]
    NoSuchAttributeName { name: String, backtrace: Backtrace },
}

pub type Result<T> = std::result::Result<T, Error>;

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
        let file = File::create(path).context(WriteFile {
            filename: path,
        })?;
        let mut to = BufWriter::new(file);

        // write preamble
        to.write_all(&[0_u8; 128][..]).context(WriteFile {
            filename: path,
        })?;

        // write magic sequence
        to.write_all(b"DICM").context(WriteFile {
            filename: path,
        })?;

        // write meta group
        self.meta.write(&mut to).context(PrintMetaDataSet)?;

        // prepare encoder
        let registry = TransferSyntaxRegistry::default();
        let ts = registry.get(&self.meta.transfer_syntax).ok_or_else(|| {
            Error::UnsupportedTransferSyntax {
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
