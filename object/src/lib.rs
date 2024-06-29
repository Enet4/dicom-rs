#![allow(clippy::derive_partial_eq_without_eq)]
//! This crate contains a high-level abstraction for reading and manipulating
//! DICOM objects.
//! At this level, objects are comparable to a dictionary of elements,
//! in which some of them can have DICOM objects themselves.
//! The end user should prefer using this abstraction when dealing with DICOM
//! objects.
//!
//! Loading a DICOM file can be done with ease via the function [`open_file`].
//! For additional file reading options, use [`OpenFileOptions`].
//! New DICOM instances can be built from scratch using [`InMemDicomObject`]
//! (see the [`mem`] module for more details).
//!
//! # Encodings
//!
//! By default, `dicom-object` supports reading and writing DICOM files
//! in any transfer syntax without data set compression.
//! This includes _Explicit VR Little Endian_
//! (with or without encapsulated pixel data),
//! _Implicit VR Little Endian_,
//! and _Explicit VR Big Endian_ (retired).
//! To enable support for reading and writing deflated data sets
//! (such as _Deflated Explicit VR Little Endian_),
//! enable **Cargo feature `deflated`**.
//! 
//! When working with imaging data,
//! consider using the [`dicom-pixeldata`] crate,
//! which offers methods to convert pixel data from objects
//! into images or multi-dimensional arrays.
//!
//! # Examples
//!
//! ## Reading a DICOM file
//!
//! To read an object from a DICOM file and inspect some attributes:
//!
//! ```no_run
//! use dicom_dictionary_std::tags;
//! use dicom_object::open_file;
//! # fn foo() -> Result<(), Box<dyn std::error::Error>> {
//! let obj = open_file("0001.dcm")?;
//!
//! let patient_name = obj.element(tags::PATIENT_NAME)?.to_str()?;
//! let modality = obj.element_by_name("Modality")?.to_str()?;
//! # Ok(())
//! # }
//! ```
//!
//! Elements can be fetched by tag,
//! either by creating a [`Tag`]
//! or by using one of the [readily available constants][const]
//! from the [`dicom-dictionary-std`][dictionary-std] crate.
//!
//! [const]: dicom_dictionary_std::tags
//! [dictionary-std]: https://docs.rs/dicom-dictionary-std
//!
//! By default, the entire data set is fully loaded into memory.
//! The pixel data and following elements can be ignored
//! by using [`OpenFileOptions`]:
//!
//! ```no_run
//! use dicom_object::OpenFileOptions;
//!
//! let obj = OpenFileOptions::new()
//!     .read_until(dicom_dictionary_std::tags::PIXEL_DATA)
//!     .open_file("0002.dcm")?;
//! # Result::<(), dicom_object::ReadError>::Ok(())
//! ```
//!
//! When reading from other data sources without file meta information,
//! you may need to use [`InMemDicomObject::read_dataset`]
//! and specify the transfer syntax explicitly.
//!
//! ## Fetching data in a DICOM file
//!
//! Once a data set element is looked up,
//! one will typically wish to inspect the value within.
//! Methods are available for converting the element's DICOM value
//! into something more usable in Rust.
//!
//! ```
//! # use dicom_dictionary_std::tags;
//! # use dicom_object::{DefaultDicomObject, Tag};
//! # fn something(obj: DefaultDicomObject) -> Result<(), Box<dyn std::error::Error>> {
//! let patient_date = obj.element(tags::PATIENT_BIRTH_DATE)?.to_date()?;
//! let pixel_data_bytes = obj.element(tags::PIXEL_DATA)?.to_bytes()?;
//! # Ok(())
//! # }
//! ```
//! 
//! ### Writing DICOM
//!
//! **Note:** the code above will only work if you are fetching native pixel data.
//! If you are potentially working with encapsulated pixel data,
//! see the [`dicom-pixeldata`] crate.
//!
//! [`dicom-pixeldata`]: https://docs.rs/dicom-pixeldata
//!
//! ## Writing DICOM data
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
//! then use [`with_meta`] or [`with_exact_meta`]:
//!
//! [file meta table]: crate::meta::FileMetaTable
//! [`FileMetaTableBuilder`]: crate::meta::FileMetaTableBuilder
//! [`with_meta`]: crate::InMemDicomObject::with_meta
//! [`with_exact_meta`]: crate::InMemDicomObject::with_exact_meta
//!
//! ```no_run
//! # use dicom_object::{InMemDicomObject, FileMetaTableBuilder};
//! # fn something(obj: InMemDicomObject) -> Result<(), Box<dyn std::error::Error>> {
//! use dicom_dictionary_std::uids;
//!
//! let file_obj = obj.with_meta(
//!     FileMetaTableBuilder::new()
//!         // Implicit VR Little Endian
//!         .transfer_syntax(uids::IMPLICIT_VR_LITTLE_ENDIAN)
//!         // Computed Radiography image storage
//!         .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
//! )?;
//! file_obj.write_to_file("0001_new.dcm")?;
//! # Ok(())
//! # }
//! ```
//!
//! In order to write a plain DICOM data set,
//! use one of the various data set writing methods
//! such as [`write_dataset_with_ts`]:
//!
//! [`write_dataset_with_ts`]: InMemDicomObject::write_dataset_with_ts
//!
//! ```
//! # use dicom_object::InMemDicomObject;
//! # use dicom_core::{DataElement, Tag, VR};
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // build your object
//! let mut obj = InMemDicomObject::from_element_iter([
//!     DataElement::new(
//!         Tag(0x0010, 0x0010),
//!         VR::PN,
//!         "Doe^John",
//!     ),
//! ]);
//!
//! // write the object's data set
//! let mut serialized = Vec::new();
//! let ts = dicom_transfer_syntax_registry::entries::EXPLICIT_VR_LITTLE_ENDIAN.erased();
//! obj.write_dataset_with_ts(&mut serialized, &ts)?;
//! assert_eq!(
//!     serialized.len(),
//!     16, // 4 (tag) + 2 (VR) + 2 (length) + 8 (data)
//! );
//! # Ok(())
//! # }
//! # run().unwrap();
//! ```
pub mod file;
pub mod mem;
pub mod meta;
pub mod ops;
pub mod collector;
pub mod tokens;

pub use crate::file::{from_reader, open_file, OpenFileOptions};
pub use crate::mem::InMemDicomObject;
pub use crate::collector::{DicomCollectorOptions, DicomCollector};
pub use crate::meta::{FileMetaTable, FileMetaTableBuilder};
use dicom_core::ops::AttributeSelector;
use dicom_core::value::{DicomValueType, ValueType};
pub use dicom_core::Tag;
use dicom_core::{DataDictionary, DicomValue};
use dicom_dictionary_std::uids;
pub use dicom_dictionary_std::StandardDataDictionary;

/// The default implementation of a root DICOM object.
pub type DefaultDicomObject<D = StandardDataDictionary> = FileDicomObject<mem::InMemDicomObject<D>>;

use dicom_core::header::{GroupNumber, HasLength};
use dicom_encoding::adapters::{PixelDataObject, RawPixelData};
use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_encoding::Codec;
use dicom_parser::dataset::{DataSetWriter, IntoTokens};
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use smallvec::SmallVec;
use snafu::{Backtrace, ResultExt, Snafu};
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// The current implementation class UID generically referring to DICOM-rs.
///
/// Automatically generated as per the standard, part 5, section B.2.
///
/// This UID may change in future versions,
/// even between patch versions.
pub const IMPLEMENTATION_CLASS_UID: &str = "2.25.262086406829110419931297894772577063974";

/// The current implementation version name generically referring to DICOM-rs.
///
/// This name may change in future versions,
/// even between patch versions.
pub const IMPLEMENTATION_VERSION_NAME: &str = "DICOM-rs 0.8.1";

/// An error which occurs when fetching a value
#[derive(Debug, Snafu)]
pub enum AttributeError {
    /// could not fetch value behind attribute {tag}
    FetchValue {
        tag: Tag,
        keyword: Option<Cow<'static, str>>,
        source: dicom_core::value::ConvertValueError,
    },

    /// the value cannot be converted to the requested type
    ConvertValue {
        source: dicom_core::value::ConvertValueError,
    },

    /// the value is not a data set sequence
    NotDataSet,

    /// the requested item index does not exist
    DataSetItemOutOfBounds,

    /// the value is not a pixel data element
    NotPixelData,

    /// the requested pixel data fragment index does not exist
    FragmentOutOfBounds,
}

/// Interface type for access to a DICOM object' attribute in a DICOM object.
///
/// Accessing an attribute via the [`DicomObject`] trait
/// will generally result in a value of this type,
/// which can then be converted into a more usable form.
///
/// Both [`DicomValue`] and references to it implement this trait.
pub trait DicomAttribute: DicomValueType {
    /// The data type of an item in a data set sequence
    type Item<'a>: HasLength
    where
        Self: 'a;

    /// The data type of a single contiguous pixel data fragment
    type PixelData<'a>
    where
        Self: 'a;

    /// Obtain an in-memory representation of the value,
    /// cloning the value one level deep if necessary.
    fn to_dicom_value<'a>(
        &'a self,
    ) -> Result<DicomValue<Self::Item<'a>, Self::PixelData<'a>>, AttributeError>;

    /// Obtain one of the items in the attribute,
    /// if the attribute value represents a data set sequence.
    ///
    /// Returns an error if the attribute is not a sequence.
    fn item(&self, index: u32) -> Result<Self::Item<'_>, AttributeError>;

    /// Obtain the number of data set items or pixel data fragments,
    /// if the attribute value represents a data set sequence or pixel data.
    /// Returns `None` otherwise.
    fn num_items(&self) -> Option<u32>;

    /// Obtain one of the fragments in the attribute,
    /// if the attribute value represents a pixel data fragment sequence.
    ///
    /// Returns an error if the attribute is not a pixel data sequence.
    fn fragment(&self, index: u32) -> Result<Self::PixelData<'_>, AttributeError>;

    /// Obtain the number of pixel data fragments,
    /// if the attribute value represents pixel data.
    ///
    /// It should equivalent to `num_items`,
    /// save for returning `None` if the attribute is a data set sequence.
    fn num_fragments(&self) -> Option<u32> {
        if DicomValueType::value_type(self) == ValueType::PixelSequence {
            self.num_items()
        } else {
            None
        }
    }

    /// Obtain the attribute's value as a string,
    /// converting it if necessary.
    fn to_str<'a>(&'a self) -> Result<Cow<'a, str>, AttributeError> {
        Ok(Cow::Owned(
            self.to_dicom_value()?
                .to_str()
                .context(ConvertValueSnafu)?
                .to_string(),
        ))
    }

    /// Obtain the attribute's value as a 16-bit unsigned integer,
    /// converting it if necessary.
    fn to_u16(&self) -> Result<u16, AttributeError> {
        self.to_dicom_value()?.to_int().context(ConvertValueSnafu)
    }

    /// Obtain the attribute's value as a 32-bit signed integer,
    /// converting it if necessary.
    fn to_i32(&self) -> Result<i32, AttributeError> {
        self.to_dicom_value()?.to_int().context(ConvertValueSnafu)
    }

    /// Obtain the attribute's value as a 32-bit unsigned integer,
    /// converting it if necessary.
    fn to_u32(&self) -> Result<u32, AttributeError> {
        self.to_dicom_value()?.to_int().context(ConvertValueSnafu)
    }

    /// Obtain the attribute's value as a 32-bit floating-point number,
    /// converting it if necessary.
    fn to_f32(&self) -> Result<f32, AttributeError> {
        self.to_dicom_value()?
            .to_float32()
            .context(ConvertValueSnafu)
    }

    /// Obtain the attribute's value as a 64-bit floating-point number,
    /// converting it if necessary.
    fn to_f64(&self) -> Result<f64, AttributeError> {
        self.to_dicom_value()?
            .to_float64()
            .context(ConvertValueSnafu)
    }

    /// Obtain the attribute's value as bytes,
    /// converting it if necessary.
    fn to_bytes<'a>(&'a self) -> Result<Cow<'a, [u8]>, AttributeError> {
        Ok(Cow::Owned(
            self.to_dicom_value()?
                .to_bytes()
                .context(ConvertValueSnafu)?
                .to_vec(),
        ))
    }
}

/// Trait type for a DICOM object.
/// This is a high-level abstraction where an object is accessed and
/// manipulated as dictionary of entries indexed by tags, which in
/// turn may contain a DICOM object.
///
/// This trait interface is experimental and prone to sudden changes.
pub trait DicomObject {
    /// The type representing a DICOM attribute in the object
    /// and/or the necessary means to retrieve the value from it.
    type Attribute<'a>: DicomAttribute
    where
        Self: 'a;

    /// Retrieve a particular DICOM attribute
    /// by looking up the given tag at the object's root.
    ///
    /// `Ok(None)` is returned when the object was successfully looked up
    /// but the element is not present.
    /// This is not a recursive search.
    fn get_opt<'a>(&'a self, tag: Tag) -> Result<Option<Self::Attribute<'a>>, AccessError>;

    /// Retrieve a particular DICOM element by its name (keyword).
    ///
    /// `Ok(None)` is returned when the object was successfully looked up
    /// but the element is not present.
    ///
    /// If the DICOM tag is already known,
    /// prefer calling [`get_opt`](Self::get_opt).
    fn get_by_name_opt<'a>(
        &'a self,
        name: &str,
    ) -> Result<Option<Self::Attribute<'a>>, AccessByNameError>;

    /// Retrieve a particular DICOM attribute
    /// by looking up the given tag at the object's root.
    ///
    /// Unlike [`get_opt`](Self::get_opt),
    /// this method returns an error if the element is not present.
    fn get<'a>(&'a self, tag: Tag) -> Result<Self::Attribute<'a>, AccessError> {
        self.get_opt(tag)?
            .context(NoSuchDataElementTagSnafu { tag })
    }

    /// Retrieve a particular DICOM element by its name (keyword).
    ///
    /// Unlike [`get_by_name_opt`](Self::get_by_name_opt),
    /// this method returns an error if the element is not present.
    fn get_by_name<'a>(&'a self, name: &str) -> Result<Self::Attribute<'a>, AccessByNameError> {
        self.get_by_name_opt(name)?
            .context(NoSuchAttributeNameSnafu { name })
    }

    /// Retrieve the processed meta information table,
    /// if available at this level.
    ///
    /// This table will generally only be reachable from the root of a DICOM object,
    /// either from opening a file or collecting the data set from a known source.
    /// Attempts to retrieve file meta information
    /// from nested data sets will likely fail.
    fn meta(&self) -> Option<&FileMetaTable> {
        None
    }
}

impl<O, P> DicomAttribute for DicomValue<O, P>
where
    O: HasLength,
    for<'a> &'a O: HasLength,
{
    type Item<'a> = &'a O
    where Self: 'a, O: 'a;
    type PixelData<'a> = &'a P
    where Self: 'a, P: 'a;

    #[inline]
    fn to_dicom_value<'a>(
        &'a self,
    ) -> Result<DicomValue<Self::Item<'a>, Self::PixelData<'a>>, AttributeError> {
        Ok(self.shallow_clone())
    }

    #[inline]
    fn item(&self, index: u32) -> Result<&O, AttributeError> {
        let items = self.items().context(NotDataSetSnafu)?;
        Ok(items
            .get(index as usize)
            .context(DataSetItemOutOfBoundsSnafu)?)
    }

    #[inline]
    fn num_items(&self) -> Option<u32> {
        match self {
            DicomValue::PixelSequence(seq) => Some(seq.fragments().len() as u32),
            DicomValue::Sequence(seq) => Some(seq.multiplicity()),
            _ => None,
        }
    }

    #[inline]
    fn fragment(&self, index: u32) -> Result<Self::PixelData<'_>, AttributeError> {
        match self {
            DicomValue::PixelSequence(seq) => Ok(seq
                .fragments()
                .get(index as usize)
                .context(FragmentOutOfBoundsSnafu)?),
            _ => Err(AttributeError::NotPixelData),
        }
    }

    #[inline]
    fn to_str<'a>(&'a self) -> Result<Cow<'a, str>, AttributeError> {
        DicomValue::to_str(&self).context(ConvertValueSnafu)
    }

    #[inline]
    fn to_bytes<'a>(&'a self) -> Result<Cow<'a, [u8]>, AttributeError> {
        DicomValue::to_bytes(&self).context(ConvertValueSnafu)
    }
}

impl<'b, O, P> DicomAttribute for &'b DicomValue<O, P>
where
    O: HasLength,
    &'b O: HasLength,
    O: Clone,
    P: Clone,
{
    type Item<'a> = &'b O
    where Self: 'a, O: 'a;
    type PixelData<'a> = &'b P
    where Self: 'a, P: 'a;

    #[inline]
    fn to_dicom_value<'a>(
        &'a self,
    ) -> Result<DicomValue<Self::Item<'a>, Self::PixelData<'a>>, AttributeError> {
        Ok(self.shallow_clone())
    }

    #[inline]
    fn item(&self, index: u32) -> Result<Self::Item<'_>, AttributeError> {
        let items = self.items().context(NotDataSetSnafu)?;
        items
            .get(index as usize)
            .context(DataSetItemOutOfBoundsSnafu)
    }

    #[inline]
    fn num_items(&self) -> Option<u32> {
        match self {
            DicomValue::PixelSequence(seq) => Some(seq.fragments().len() as u32),
            DicomValue::Sequence(seq) => Some(seq.multiplicity()),
            _ => None,
        }
    }

    #[inline]
    fn fragment(&self, index: u32) -> Result<Self::PixelData<'_>, AttributeError> {
        match self {
            DicomValue::PixelSequence(seq) => seq
                .fragments()
                .get(index as usize)
                .context(FragmentOutOfBoundsSnafu),
            _ => Err(AttributeError::NotPixelData),
        }
    }

    #[inline]
    fn num_fragments(&self) -> Option<u32> {
        match self {
            DicomValue::PixelSequence(seq) => Some(seq.fragments().len() as u32),
            _ => None,
        }
    }

    #[inline]
    fn to_str<'a>(&'a self) -> Result<Cow<'a, str>, AttributeError> {
        DicomValue::to_str(&self).context(ConvertValueSnafu)
    }

    #[inline]
    fn to_bytes<'a>(&'a self) -> Result<Cow<'a, [u8]>, AttributeError> {
        DicomValue::to_bytes(&self).context(ConvertValueSnafu)
    }
}

/// An error which may occur when loading a DICOM object
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum ReadError {
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
    #[snafu(display("Could not parse sop attribute"))]
    ParseSopAttribute {
        #[snafu(source(from(dicom_core::value::ConvertValueError, Box::from)))]
        source: Box<dicom_core::value::ConvertValueError>,
        backtrace: Backtrace,
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
    #[snafu(display("Missing element value after header token"))]
    MissingElementValue { backtrace: Backtrace },
    #[snafu(display("Unrecognized transfer syntax `{}`", uid))]
    ReadUnrecognizedTransferSyntax { uid: String, backtrace: Backtrace },
    #[snafu(display("Unsupported reading for transfer syntax `{}` ({})", uid, name))]
    ReadUnsupportedTransferSyntax { uid: &'static str, name: &'static str, backtrace: Backtrace },
    #[snafu(display(
        "Unsupported reading for transfer syntax `{uid}` ({name}, try enabling feature `{feature_name}`)"
    ))]
    ReadUnsupportedTransferSyntaxWithSuggestion {
        uid: &'static str,
        name: &'static str,
        feature_name: &'static str,
        backtrace: Backtrace,
    },
    #[snafu(display("Unexpected token {:?}", token))]
    UnexpectedToken {
        token: Box<dicom_parser::dataset::DataToken>,
        backtrace: Backtrace,
    },
    #[snafu(display("Premature data set end"))]
    PrematureEnd { backtrace: Backtrace },
}

/// An error which may occur when writing a DICOM object
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum WriteError {
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
    #[snafu(display("Unrecognized transfer syntax `{uid}`"))]
    WriteUnrecognizedTransferSyntax { uid: String, backtrace: Backtrace },
    #[snafu(display("Unsupported transfer syntax `{uid}` ({name})"))]
    WriteUnsupportedTransferSyntax { uid: &'static str, name: &'static str, backtrace: Backtrace },
    #[snafu(display(
        "Unsupported transfer syntax `{uid}` ({name}, try enabling feature `{feature_name}`)"
    ))]
    WriteUnsupportedTransferSyntaxWithSuggestion {
        uid: &'static str,
        name: &'static str,
        feature_name: &'static str,
        backtrace: Backtrace,
    },
}

/// An error which may occur during private element look-up or insertion
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum PrivateElementError {
    /// Group number must be odd
    #[snafu(display("Group number must be odd, found {:#06x}", group))]
    InvalidGroup { group: GroupNumber },
    /// Private creator not found in group
    #[snafu(display("Private creator {} not found in group {:#06x}", creator, group))]
    PrivateCreatorNotFound { creator: String, group: GroupNumber },
    /// Element not found in group
    #[snafu(display(
        "Private Creator {} found in group {:#06x}, but elem {:#06x} not found",
        creator,
        group,
        elem
    ))]
    ElementNotFound {
        creator: String,
        group: GroupNumber,
        elem: u8,
    },
    /// No space available for more private elements in the group
    #[snafu(display("No space available in group {:#06x}", group))]
    NoSpace { group: GroupNumber },
}

/// An error which may occur when looking up a DICOM object's attributes.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum AccessError {
    #[snafu(display("No such data element with tag {}", tag))]
    NoSuchDataElementTag { tag: Tag, backtrace: Backtrace },
}

impl AccessError {
    pub fn into_access_by_name(self, alias: impl Into<String>) -> AccessByNameError {
        match self {
            AccessError::NoSuchDataElementTag { tag, backtrace } => {
                AccessByNameError::NoSuchDataElementAlias {
                    tag,
                    alias: alias.into(),
                    backtrace,
                }
            }
        }
    }
}

/// An error which may occur when looking up a DICOM object's attributes
/// at an arbitrary depth,
/// such as through [`value_at`](crate::InMemDicomObject::value_at).
#[derive(Debug, Snafu)]
#[non_exhaustive]
#[snafu(visibility(pub(crate)))]
pub enum AtAccessError {
    /// Missing intermediate sequence for {selector} at step {step_index}
    MissingSequence {
        selector: AttributeSelector,
        step_index: u32,
    },
    /// Step {step_index} for {selector} is not a data set sequence
    NotASequence {
        selector: AttributeSelector,
        step_index: u32,
    },
    /// Missing element at last step for {selector}
    MissingLeafElement { selector: AttributeSelector },
}

/// An error which may occur when looking up a DICOM object's attributes
/// by a keyword (or alias) instead of by tag.
///
/// These accesses incur a look-up at the data element dictionary,
/// which may fail if no such entry exists.
#[derive(Debug, Snafu)]
pub enum AccessByNameError {
    #[snafu(display("No such data element {} (with tag {})", alias, tag))]
    NoSuchDataElementAlias {
        tag: Tag,
        alias: String,
        backtrace: Backtrace,
    },

    /// Could not resolve attribute name from the data dictionary
    #[snafu(display("Unknown data attribute named `{}`", name))]
    NoSuchAttributeName { name: String, backtrace: Backtrace },
}

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum WithMetaError {
    /// Could not build file meta table
    BuildMetaTable {
        #[snafu(backtrace)]
        source: crate::meta::Error,
    },
    /// Could not prepare file meta table
    PrepareMetaTable {
        #[snafu(source(from(dicom_core::value::ConvertValueError, Box::from)))]
        source: Box<dicom_core::value::ConvertValueError>,
        backtrace: Backtrace,
    },
}

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

    /// Retrieve a mutable reference to the processed meta header table.
    ///
    /// Considerable care should be taken when modifying this table,
    /// as it may influence object reading and writing operations.
    /// When modifying the table through this method,
    /// the user is responsible for updating the meta information group length as well,
    /// which can be done by calling
    /// [`update_information_group_length`](FileMetaTable::update_information_group_length).
    ///
    /// See also [`update_meta`](Self::update_meta).
    pub fn meta_mut(&mut self) -> &mut FileMetaTable {
        &mut self.meta
    }

    /// Update the processed meta header table through a function.
    ///
    /// Considerable care should be taken when modifying this table,
    /// as it may influence object reading and writing operations.
    /// The meta information group length is updated automatically.
    pub fn update_meta(&mut self, f: impl FnOnce(&mut FileMetaTable)) {
        f(&mut self.meta);
        self.meta.update_information_group_length();
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
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), WriteError> {
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

        self.write_dataset_impl(to)
    }

    /// Write the entire object as a DICOM file
    /// into the given writer.
    /// Preamble, magic code, and file meta group will be included
    /// before the inner object.
    pub fn write_all(&self, to: impl Write) -> Result<(), WriteError> {
        let mut to = BufWriter::new(to);

        // write preamble
        to.write_all(&[0_u8; 128][..]).context(WritePreambleSnafu)?;

        // write magic sequence
        to.write_all(b"DICM").context(WriteMagicCodeSnafu)?;

        // write meta group
        self.meta.write(&mut to).context(PrintMetaDataSetSnafu)?;

        self.write_dataset_impl(to)
    }

    /// Write the file meta group set into the given writer.
    ///
    /// This is equivalent to `self.meta().write(to)`.
    pub fn write_meta<W: Write>(&self, to: W) -> Result<(), WriteError> {
        self.meta.write(to).context(PrintMetaDataSetSnafu)
    }

    /// Write the inner data set into the given writer,
    /// without preamble, magic code, nor file meta group.
    ///
    /// The transfer syntax is selected from the file meta table.
    pub fn write_dataset<W: Write>(&self, to: W) -> Result<(), WriteError> {
        let to = BufWriter::new(to);

        self.write_dataset_impl(to)
    }

    /// Helper function for writing the DICOM data set in this file DICOM object
    /// with the right transfer syntax.
    /// Automatically retrieves a data set adapter if required and available,
    /// returns an error if the transfer syntax is not supported for data set writing.
    fn write_dataset_impl(&self, to: impl Write) -> Result<(), WriteError> {
        let ts_uid = self.meta.transfer_syntax();
        // prepare encoder
        let ts = if let Some(ts) = TransferSyntaxRegistry.get(ts_uid) {
            ts
        } else {
            return WriteUnrecognizedTransferSyntaxSnafu {
                uid: ts_uid.to_string(),
            }
            .fail();
        };
        match ts.codec() {
            Codec::Dataset(Some(adapter)) => {
                let adapter = adapter.adapt_writer(Box::new(to));
                let mut dset_writer =
                    DataSetWriter::with_ts(adapter, ts).context(CreatePrinterSnafu)?;

                // write object
                dset_writer
                    .write_sequence((&self.obj).into_tokens())
                    .context(PrintDataSetSnafu)?;

                Ok(())
            },
            Codec::Dataset(None) => {
                // dataset adapter needed, but not provided
                if ts_uid == uids::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN
                    || ts_uid == uids::JPIP_REFERENCED_DEFLATE
                    || ts_uid == uids::JPIPHTJ2K_REFERENCED_DEFLATE {
                    return WriteUnsupportedTransferSyntaxWithSuggestionSnafu {
                        uid: ts.uid(),
                        name: ts.name(),
                        feature_name: "dicom-transfer-syntax-registry/deflate",
                    }
                    .fail();
                }
                WriteUnsupportedTransferSyntaxSnafu {
                    uid: ts.uid(),
                    name: ts.name(),
                }
                .fail()
            }
            Codec::None | Codec::EncapsulatedPixelData(..) => {
                // no dataset adapter needed
                let mut dset_writer = DataSetWriter::with_ts(to, ts).context(CreatePrinterSnafu)?;

                // write object
                dset_writer
                    .write_sequence((&self.obj).into_tokens())
                    .context(PrintDataSetSnafu)?;

                Ok(())
            }
        }
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
    type Attribute<'a> = <O as DicomObject>::Attribute<'a>
        where O: 'a;

    #[inline]
    fn get_opt(&self, tag: Tag) -> Result<Option<Self::Attribute<'_>>, AccessError> {
        self.obj.get_opt(tag)
    }

    #[inline]
    fn get_by_name_opt(
        &self,
        name: &str,
    ) -> Result<Option<Self::Attribute<'_>>, AccessByNameError> {
        self.obj.get_by_name_opt(name)
    }

    #[inline]
    fn get(&self, tag: Tag) -> Result<Self::Attribute<'_>, AccessError> {
        self.obj.get(tag)
    }

    #[inline]
    fn get_by_name(&self, name: &str) -> Result<Self::Attribute<'_>, AccessByNameError> {
        self.obj.get_by_name(name)
    }

    fn meta(&self) -> Option<&FileMetaTable> {
        Some(&self.meta)
    }
}

impl<'s, O: 's> DicomObject for &'s FileDicomObject<O>
where
    O: DicomObject,
{
    type Attribute<'a> = <O as DicomObject>::Attribute<'a>
        where 's: 'a;

    #[inline]
    fn get_opt(&self, tag: Tag) -> Result<Option<Self::Attribute<'_>>, AccessError> {
        self.obj.get_opt(tag)
    }

    #[inline]
    fn get_by_name_opt(
        &self,
        name: &str,
    ) -> Result<Option<Self::Attribute<'_>>, AccessByNameError> {
        self.obj.get_by_name_opt(name)
    }

    #[inline]
    fn get(&self, tag: Tag) -> Result<Self::Attribute<'_>, AccessError> {
        self.obj.get(tag)
    }

    #[inline]
    fn get_by_name(&self, name: &str) -> Result<Self::Attribute<'_>, AccessByNameError> {
        self.obj.get_by_name(name)
    }
}

/// This implementation creates an iterator
/// to the elements of the underlying data set,
/// consuming the whole object.
/// The attributes in the file meta group are _not_ included.
///
/// To obtain an iterator over the meta elements,
/// use [`meta().to_element_iter()`](FileMetaTable::to_element_iter).
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
///
/// To obtain an iterator over the meta elements,
/// use [`meta().to_element_iter()`](FileMetaTable::to_element_iter).
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
    fn transfer_syntax_uid(&self) -> &str {
        self.meta.transfer_syntax()
    }

    /// Return the Rows attribute or None if it is not found
    fn rows(&self) -> Option<u16> {
        (**self)
            .get(dicom_dictionary_std::tags::ROWS)?
            .uint16()
            .ok()
    }

    /// Return the Columns attribute or None if it is not found
    fn cols(&self) -> Option<u16> {
        (**self)
            .get(dicom_dictionary_std::tags::COLUMNS)?
            .uint16()
            .ok()
    }

    /// Return the SamplesPerPixel attribute or None if it is not found
    fn samples_per_pixel(&self) -> Option<u16> {
        (**self)
            .get(dicom_dictionary_std::tags::SAMPLES_PER_PIXEL)?
            .uint16()
            .ok()
    }

    /// Return the BitsAllocated attribute or None if it is not set
    fn bits_allocated(&self) -> Option<u16> {
        (**self)
            .get(dicom_dictionary_std::tags::BITS_ALLOCATED)?
            .uint16()
            .ok()
    }

    /// Return the BitsStored attribute or None if it is not set
    fn bits_stored(&self) -> Option<u16> {
        (**self)
            .get(dicom_dictionary_std::tags::BITS_STORED)?
            .uint16()
            .ok()
    }

    fn photometric_interpretation(&self) -> Option<&str> {
        (**self)
            .get(dicom_dictionary_std::tags::PHOTOMETRIC_INTERPRETATION)?
            .string()
            .ok()
            .map(|s| s.trim_end())
    }

    /// Return the NumberOfFrames attribute or None if it is not set
    fn number_of_frames(&self) -> Option<u32> {
        (**self)
            .get(dicom_dictionary_std::tags::NUMBER_OF_FRAMES)?
            .to_int()
            .ok()
    }

    /// Returns the number of fragments or None for native pixel data
    fn number_of_fragments(&self) -> Option<u32> {
        let pixel_data = (**self).get(dicom_dictionary_std::tags::PIXEL_DATA)?;
        match pixel_data.value() {
            DicomValue::Primitive(_p) => Some(1),
            DicomValue::PixelSequence(v) => Some(v.fragments().len() as u32),
            DicomValue::Sequence(..) => None,
        }
    }

    /// Return a specific encoded pixel fragment by index as a `Vec<u8>`
    /// or `None` if no pixel data is found.
    ///
    /// Non-encapsulated pixel data can be retrieved by requesting fragment #0.
    ///
    /// Panics if `fragment` is out of bounds for the encapsulated pixel data fragments.
    fn fragment(&self, fragment: usize) -> Option<Cow<[u8]>> {
        let pixel_data = (**self).get(dicom_dictionary_std::tags::PIXEL_DATA)?;
        match pixel_data.value() {
            DicomValue::PixelSequence(v) => Some(Cow::Borrowed(v.fragments()[fragment].as_ref())),
            DicomValue::Primitive(p) if fragment == 0 => Some(p.to_bytes()),
            _ => None,
        }
    }

    fn offset_table(&self) -> Option<Cow<[u32]>> {
        let pixel_data = (**self).get(dicom_dictionary_std::tags::PIXEL_DATA)?;
        match pixel_data.value() {
            DicomValue::Primitive(_) => None,
            DicomValue::Sequence(_) => None,
            DicomValue::PixelSequence(seq) => Some(Cow::from(seq.offset_table())),
        }
    }

    /// Should return either a byte slice/vector if native pixel data
    /// or byte fragments if encapsulated.
    /// Returns None if no pixel data is found
    fn raw_pixel_data(&self) -> Option<RawPixelData> {
        let pixel_data = (**self).get(dicom_dictionary_std::tags::PIXEL_DATA)?;
        match pixel_data.value() {
            DicomValue::Primitive(p) => {
                // Create 1 fragment with all bytes
                let fragment = p.to_bytes().to_vec();
                let mut fragments = SmallVec::new();
                fragments.push(fragment);
                Some(RawPixelData {
                    fragments,
                    offset_table: SmallVec::new(),
                })
            }
            DicomValue::PixelSequence(v) => {
                let (offset_table, fragments) = v.clone().into_parts();
                Some(RawPixelData {
                    fragments,
                    offset_table,
                })
            }
            DicomValue::Sequence(..) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use dicom_core::{DataElement, PrimitiveValue, VR};

    use crate::meta::FileMetaTableBuilder;
    use crate::{AccessError, FileDicomObject, InMemDicomObject};

    fn assert_type_not_too_large<T>(max_size: usize) {
        let size = std::mem::size_of::<T>();
        if size > max_size {
            panic!(
                "Type {} of byte size {} exceeds acceptable size {}",
                std::any::type_name::<T>(),
                size,
                max_size
            );
        }
    }

    #[test]
    fn errors_not_too_large() {
        assert_type_not_too_large::<AccessError>(64);
    }

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
            Err(AccessError::NoSuchDataElementTag { .. }),
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
        assert_eq!(
            iter.next().unwrap().header().tag,
            dicom_dictionary_std::tags::SOP_INSTANCE_UID
        );
        assert_eq!(
            iter.next().unwrap().header().tag,
            dicom_dictionary_std::tags::PATIENT_NAME
        );
        assert_eq!(iter.next(), None);

        // into_iter
        let mut iter = obj.into_iter();
        assert_eq!(
            iter.next().unwrap().header().tag,
            dicom_dictionary_std::tags::SOP_INSTANCE_UID
        );
        assert_eq!(
            iter.next().unwrap().header().tag,
            dicom_dictionary_std::tags::PATIENT_NAME
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    pub fn file_dicom_can_update_meta() {
        let meta = FileMetaTableBuilder::new()
            .transfer_syntax(
                dicom_transfer_syntax_registry::entries::EXPLICIT_VR_LITTLE_ENDIAN.uid(),
            )
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
            .media_storage_sop_instance_uid("2.25.280986007517028771599125034987786349815")
            .implementation_class_uid("1.2.345.6.7890.1.234")
            .build()
            .unwrap();
        let mut obj = FileDicomObject::new_empty_with_meta(meta);

        obj.update_meta(|meta| {
            meta.receiving_application_entity_title = Some("SOMETHING".to_string());
        });

        assert_eq!(
            obj.meta().receiving_application_entity_title.as_deref(),
            Some("SOMETHING"),
        );
    }

    #[test]
    fn attribute_api_on_primitive_values() {
        use crate::DicomAttribute;
        use dicom_dictionary_std::tags;

        let obj = InMemDicomObject::from_element_iter([
            DataElement::new(tags::PATIENT_NAME, VR::PN, PrimitiveValue::from("Doe^John")),
            DataElement::new(tags::INSTANCE_NUMBER, VR::IS, PrimitiveValue::from("5")),
        ]);

        let patient_name = obj.get(tags::PATIENT_NAME).unwrap().value();

        // can get string using DICOM attribute API

        assert_eq!(&DicomAttribute::to_str(patient_name).unwrap(), "Doe^John");

        // can get integer from Instance Number

        let instance_number = obj.get(tags::INSTANCE_NUMBER).unwrap().value();
        assert_eq!(DicomAttribute::to_u32(instance_number).unwrap(), 5);

        // cannot get items

        assert_eq!(patient_name.num_items(), None);
        assert_eq!(patient_name.num_fragments(), None);
        assert!(matches!(
            patient_name.item(0),
            Err(crate::AttributeError::NotDataSet)
        ));

        assert_eq!(instance_number.num_items(), None);
        assert_eq!(instance_number.num_fragments(), None);
        assert!(matches!(
            patient_name.item(0),
            Err(crate::AttributeError::NotDataSet)
        ));
        assert!(matches!(
            instance_number.item(0),
            Err(crate::AttributeError::NotDataSet)
        ));
    }
}
