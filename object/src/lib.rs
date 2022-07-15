//! This crate contains a high-level abstraction for reading and manipulating
//! DICOM objects.
//! At this level, objects are comparable to a dictionary of elements,
//! in which some of them can have DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM
//! objects.
//!
//! Loading a DICOM file can be done with easily via the function [`open_file`].
//! For additional file reading options, use [`OpenFileOptions`].
//!
//! # Examples
//!
//! Read an object and fetch some attributes by their standard alias:
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
//! The current default implementation places the full DICOM object in memory.
//! The pixel data and following elements can be ignored
//! by using [`OpenFileOptions`]:
//!
//! ```no_run
//! use dicom_object::OpenFileOptions;
//!
//! let obj = OpenFileOptions::new()
//!     .read_until(dicom_dictionary_std::tags::PIXEL_DATA)
//!     .open_file("0002.dcm")?;
//! # Result::<(), dicom_object::Error>::Ok(())
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
//! Finally, DICOM objects can be serialized back into DICOM encoded bytes.
//! A method is provided for writing a file DICOM object into a new DICOM file.
//!
//! ```no_run
//! # use dicom_object::{DefaultDicomObject, Tag};
//! # fn something(obj: DefaultDicomObject) -> Result<(), Box<dyn std::error::Error>> {
//! obj.write_to_file("0001_new.dcm")?;
//! # Ok(())
//! # }
//! ```
//!
//! This method requires you to write a [file meta table] first.
//! When creating a new DICOM object from scratch,
//! use a [`FileMetaTableBuilder`] to construct the file meta group,
//! then use `with_meta` or `with_exact_meta`:
//!
//! [file meta table]: crate::meta::FileMetaTable
//! [`FileMetaTableBuilder`]: crate::meta::FileMetaTableBuilder
//!
//! ```no_run
//! # use dicom_object::{InMemDicomObject, FileMetaTableBuilder};
//! # fn something(obj: InMemDicomObject) -> Result<(), Box<dyn std::error::Error>> {
//! let file_obj = obj.with_meta(
//!     FileMetaTableBuilder::new()
//!         // Implicit VR Little Endian
//!         .transfer_syntax("1.2.840.10008.1.2")
//!         // Computed Radiography image storage
//!         .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
//! )?;
//! file_obj.write_to_file("0001_new.dcm")?;
//! # Ok(())
//! # }
//! ```
//!
//! In order to write a plain DICOM data set,
//! use one of the various `write_dataset` methods.
//!
//! ```
//! # use dicom_object::InMemDicomObject;
//! # use dicom_core::{DataElement, Tag, VR, dicom_value};
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // build your object
//! let mut obj = InMemDicomObject::new_empty();
//! let patient_name = DataElement::new(
//!     Tag(0x0010, 0x0010),
//!     VR::PN,
//!     dicom_value!(Str, "Doe^John"),
//! );
//! obj.put(patient_name);
//!
//! // write the object's data set
//! let mut serialized = Vec::new();
//! let ts = dicom_transfer_syntax_registry::entries::EXPLICIT_VR_LITTLE_ENDIAN.erased();
//! obj.write_dataset_with_ts(&mut serialized, &ts)?;
//! assert!(!serialized.is_empty());
//! # Ok(())
//! # }
//! # run().unwrap();
//! ```
pub mod file;
pub mod mem;
pub mod meta;
#[deprecated(
    since = "0.5.0",
    note = "This is a stub, use the `dicom-pixeldata` crate instead"
)]
pub mod pixeldata;
pub mod tokens;

mod util;

pub use crate::file::{from_reader, open_file, OpenFileOptions};
pub use crate::mem::InMemDicomObject;
pub use crate::meta::{FileMetaTable, FileMetaTableBuilder};
use dicom_core::DataDictionary;
pub use dicom_core::Tag;
pub use dicom_dictionary_std::StandardDataDictionary;

/// The default implementation of a root DICOM object.
pub type DefaultDicomObject<D = StandardDataDictionary> = FileDicomObject<mem::InMemDicomObject<D>>;

use dicom_core::header::Header;
use dicom_encoding::adapters::{PixelDataObject, RawPixelData};
use dicom_encoding::{text::SpecificCharacterSet, transfer_syntax::TransferSyntaxIndex};
use dicom_parser::dataset::{DataSetWriter, IntoTokens};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use smallvec::SmallVec;
use snafu::{Backtrace, OptionExt, ResultExt, Snafu};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// The current implementation class UID generically referring to DICOM-rs.
///
/// Automatically generated as per the standard, part 5, section B.2.
///
/// This UID is subject to changes in future versions.
pub const IMPLEMENTATION_CLASS_UID: &str = "2.25.137038125948464847900039011591283709926";

/// The current implementation version name generically referring to DICOM-rs.
///
/// This names is subject to changes in future versions.
pub const IMPLEMENTATION_VERSION_NAME: &str = "DICOM-rs 0.3";

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
    #[snafu(display("Could not open file '{}'", filename.display()))]
    OpenFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Could not read from file '{}'", filename.display()))]
    ReadFile {
        filename: std::path::PathBuf,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    /// Could not read preamble bytes
    ReadPreambleBytes {
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
    /// Could not build file meta table
    BuildMetaTable {
        #[snafu(backtrace)]
        source: crate::meta::Error,
    },
    /// Could not prepare file meta table
    PrepareMetaTable {
        source: dicom_core::value::CastValueError,
        backtrace: Backtrace,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A root DICOM object contains additional meta information about the object
/// in a separate table.
#[deprecated(since = "0.4.0", note = "use `FileDicomObject` instead")]
pub type RootDicomObject<O> = FileDicomObject<O>;

/// A root DICOM object retrieved from a standard DICOM file,
/// containing additional information from the file meta group
/// in a separate table value.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDicomObject<O> {
    meta: FileMetaTable,
    obj: O,
}

impl<O> FileDicomObject<O> {
    /// Retrieve the processed meta header table.
    pub fn meta(&self) -> &FileMetaTable {
        &self.meta
    }

    /// Retrieve the inner DICOM object structure, discarding the meta table.
    pub fn into_inner(self) -> O {
        self.obj
    }
}

impl<O> FileDicomObject<O>
where
    for<'a> &'a O: IntoTokens,
{
    /// Write the entire object as a DICOM file
    /// into the given file path.
    /// Preamble, magic code, and file meta group will be included
    /// before the inner object.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let file = File::create(path).context(WriteFileSnafu { filename: path })?;
        let mut to = BufWriter::new(file);

        // write preamble
        to.write_all(&[0_u8; 128][..])
            .context(WriteFileSnafu { filename: path })?;

        // write magic sequence
        to.write_all(b"DICM")
            .context(WriteFileSnafu { filename: path })?;

        // write meta group
        self.meta.write(&mut to).context(PrintMetaDataSetSnafu)?;

        // prepare encoder
        let registry = TransferSyntaxRegistry::default();
        let ts = registry.get(&self.meta.transfer_syntax).with_context(|| {
            UnsupportedTransferSyntaxSnafu {
                uid: self.meta.transfer_syntax.clone(),
            }
        })?;
        let cs = SpecificCharacterSet::Default;
        let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinterSnafu)?;

        // write object
        dset_writer
            .write_sequence((&self.obj).into_tokens())
            .context(PrintDataSetSnafu)?;

        Ok(())
    }

    /// Write the entire object as a DICOM file
    /// into the given writer.
    /// Preamble, magic code, and file meta group will be included
    /// before the inner object.
    pub fn write_all<W: Write>(&self, to: W) -> Result<()> {
        let mut to = BufWriter::new(to);

        // write preamble
        to.write_all(&[0_u8; 128][..]).context(WritePreambleSnafu)?;

        // write magic sequence
        to.write_all(b"DICM").context(WriteMagicCodeSnafu)?;

        // write meta group
        self.meta.write(&mut to).context(PrintMetaDataSetSnafu)?;

        // prepare encoder
        let registry = TransferSyntaxRegistry::default();
        let ts = registry.get(&self.meta.transfer_syntax).with_context(|| {
            UnsupportedTransferSyntaxSnafu {
                uid: self.meta.transfer_syntax.clone(),
            }
        })?;
        let cs = SpecificCharacterSet::Default;
        let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinterSnafu)?;

        // write object
        dset_writer
            .write_sequence((&self.obj).into_tokens())
            .context(PrintDataSetSnafu)?;

        Ok(())
    }

    /// Write the file meta group set into the given writer.
    ///
    /// This is equivalent to `self.meta().write(to)`.
    pub fn write_meta<W: Write>(&self, to: W) -> Result<()> {
        self.meta.write(to).context(PrintMetaDataSetSnafu)
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
            UnsupportedTransferSyntaxSnafu {
                uid: self.meta.transfer_syntax.clone(),
            }
        })?;
        let cs = SpecificCharacterSet::Default;
        let mut dset_writer = DataSetWriter::with_ts_cs(to, ts, cs).context(CreatePrinterSnafu)?;

        // write object
        dset_writer
            .write_sequence((&self.obj).into_tokens())
            .context(PrintDataSetSnafu)?;

        Ok(())
    }
}

impl<O> ::std::ops::Deref for FileDicomObject<O> {
    type Target = O;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<O> ::std::ops::DerefMut for FileDicomObject<O> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}

impl<O> DicomObject for FileDicomObject<O>
where
    O: DicomObject,
{
    type Element = <O as DicomObject>::Element;

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

impl<'a, O: 'a> DicomObject for &'a FileDicomObject<O>
where
    O: DicomObject,
{
    type Element = <O as DicomObject>::Element;

    fn element(&self, tag: Tag) -> Result<Self::Element> {
        self.obj.element(tag)
    }

    fn element_by_name(&self, name: &str) -> Result<Self::Element> {
        self.obj.element_by_name(name)
    }
}

/// This implementation creates an iterator
/// to the elements of the underlying data set,
/// consuming the whole object.
/// The attributes in the file meta group are _not_ included.
impl<O> IntoIterator for FileDicomObject<O>
where
    O: IntoIterator,
{
    type Item = <O as IntoIterator>::Item;
    type IntoIter = <O as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.obj.into_iter()
    }
}

/// This implementation creates an iterator
/// to the elements of the underlying data set.
/// The attributes in the file meta group are _not_ included.
impl<'a, O> IntoIterator for &'a FileDicomObject<O>
where
    &'a O: IntoIterator,
{
    type Item = <&'a O as IntoIterator>::Item;
    type IntoIter = <&'a O as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        (&self.obj).into_iter()
    }
}

/// Implement basic pixeldata encoder/decoder functionality
impl<D> PixelDataObject for FileDicomObject<InMemDicomObject<D>>
where
    D: DataDictionary + Clone,
{
    /// Return the Rows attribute or None if it is not found
    fn rows(&self) -> Option<u16> {
        self.element(dicom_dictionary_std::tags::ROWS)
            .ok()?
            .uint16()
            .ok()
    }

    /// Return the Columns attribute or None if it is not found
    fn cols(&self) -> Option<u16> {
        self.element(dicom_dictionary_std::tags::COLUMNS)
            .ok()?
            .uint16()
            .ok()
    }

    /// Return the SamplesPerPixel attribute or None if it is not found
    fn samples_per_pixel(&self) -> Option<u16> {
        self.element(dicom_dictionary_std::tags::SAMPLES_PER_PIXEL)
            .ok()?
            .uint16()
            .ok()
    }

    /// Return the BitsAllocated attribute or None if it is not set
    fn bits_allocated(&self) -> Option<u16> {
        self.element(dicom_dictionary_std::tags::BITS_ALLOCATED)
            .ok()?
            .uint16()
            .ok()
    }

    /// Return the NumberOfFrames attribute or None if it is not set
    fn number_of_frames(&self) -> Option<u16> {
        self.element(dicom_dictionary_std::tags::NUMBER_OF_FRAMES)
            .ok()?
            .to_int()
            .ok()
    }

    /// Returns the number of fragments or None for native pixel data
    fn number_of_fragments(&self) -> Option<u32> {
        let pixel_data = self.element(dicom_dictionary_std::tags::PIXEL_DATA).ok()?;
        match pixel_data.value() {
            dicom_core::DicomValue::Primitive(_p) => Some(1),
            dicom_core::DicomValue::PixelSequence {
                offset_table: _,
                fragments,
            } => Some(fragments.len() as u32),
            dicom_core::DicomValue::Sequence { items: _, size: _ } => None,
        }
    }

    /// Return a specific encoded pixel fragment by index as Vec<u8>
    /// or None if no pixel data is found
    fn fragment(&self, fragment: usize) -> Option<Vec<u8>> {
        let pixel_data = self.element(dicom_dictionary_std::tags::PIXEL_DATA).ok()?;
        match pixel_data.value() {
            dicom_core::DicomValue::PixelSequence {
                offset_table: _,
                fragments,
            } => Some(fragments[fragment as usize].clone()),
            _ => None,
        }
    }

    /// Should return either a byte slice/vector if native pixel data
    /// or byte fragments if encapsulated.
    /// Returns None if no pixel data is found
    fn raw_pixel_data(&self) -> Option<RawPixelData> {
        let pixel_data = self.element(dicom_dictionary_std::tags::PIXEL_DATA).ok()?;
        match pixel_data.value() {
            dicom_core::DicomValue::Primitive(p) => {
                // Create 1 fragment with all bytes
                let fragment = p.to_bytes().to_vec();
                let mut fragments = SmallVec::new();
                fragments.push(fragment);
                Some(RawPixelData {
                    fragments,
                    offset_table: SmallVec::new(),
                })
            }
            dicom_core::DicomValue::PixelSequence {
                offset_table,
                fragments,
            } => Some(RawPixelData {
                fragments: fragments.clone(),
                offset_table: offset_table.clone(),
            }),
            dicom_core::DicomValue::Sequence { items: _, size: _ } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use dicom_core::{DataElement, PrimitiveValue, VR};

    use crate::meta::FileMetaTableBuilder;
    use crate::{Error, FileDicomObject, InMemDicomObject};

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
        let obj = FileDicomObject::new_empty_with_meta(meta);

        obj.write_to_file(FILE_NAME).unwrap();

        let obj2 = FileDicomObject::open_file(FILE_NAME).unwrap();

        assert_eq!(obj, obj2);

        let _ = std::fs::remove_file(FILE_NAME);
    }

    /// A FileDicomObject<InMemDicomObject>
    /// can be used like a DICOM object.
    #[test]
    fn file_dicom_object_can_use_inner() {
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            dicom_dictionary_std::tags::PATIENT_NAME,
            VR::PN,
            PrimitiveValue::from("John Doe"),
        ));

        let mut obj = obj
            .with_meta(
                FileMetaTableBuilder::new()
                    .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                    .media_storage_sop_instance_uid("1.2.23456789")
                    .transfer_syntax("1.2.840.10008.1.2.1"),
            )
            .unwrap();

        // contains patient name
        assert_eq!(
            obj.element(dicom_dictionary_std::tags::PATIENT_NAME)
                .unwrap()
                .value()
                .to_str()
                .unwrap(),
            "John Doe",
        );

        // can be removed with take
        obj.take_element(dicom_dictionary_std::tags::PATIENT_NAME)
            .unwrap();

        assert!(matches!(
            obj.element(dicom_dictionary_std::tags::PATIENT_NAME),
            Err(Error::NoSuchDataElementTag { .. }),
        ));
    }

    #[test]
    fn file_dicom_object_can_iterate_over_elements() {
        let mut obj = InMemDicomObject::new_empty();

        obj.put(DataElement::new(
            dicom_dictionary_std::tags::PATIENT_NAME,
            VR::PN,
            PrimitiveValue::from("John Doe"),
        ));
        obj.put(DataElement::new(
            dicom_dictionary_std::tags::SOP_INSTANCE_UID,
            VR::PN,
            PrimitiveValue::from("1.2.987654321"),
        ));
            
        let obj = obj
            .with_meta(
                FileMetaTableBuilder::new()
                    .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                    .media_storage_sop_instance_uid("1.2.987654321")
                    .transfer_syntax("1.2.840.10008.1.2.1"),
            )
            .unwrap();

        // iter
        let mut iter = (&obj).into_iter();
        assert_eq!(iter.next().unwrap().header().tag, dicom_dictionary_std::tags::SOP_INSTANCE_UID);
        assert_eq!(iter.next().unwrap().header().tag, dicom_dictionary_std::tags::PATIENT_NAME);
        assert_eq!(iter.next(), None);

        // into_iter
        let mut iter = obj.into_iter();
        assert_eq!(iter.next().unwrap().header().tag, dicom_dictionary_std::tags::SOP_INSTANCE_UID);
        assert_eq!(iter.next().unwrap().header().tag, dicom_dictionary_std::tags::PATIENT_NAME);
        assert_eq!(iter.next(), None);
    }
}
