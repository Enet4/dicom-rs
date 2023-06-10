//! Module for the attribute operations API.
//! 
//! This allows consumers to specify and implement
//! operations on DICOM objects
//! as part of a larger process,
//! such as anonymization or transcoding.
//! 
//! The most important type here is [`AttributeOp`],
//! which indicates which attribute is affected,
//! and the operation to apply ([`AttributeAction`]).
//! All DICOM object types supporting this API
//! implement the [`ApplyOp`] trait.
//! 
//! # Example
//! 
//! Given a DICOM object
//! (opened using [`dicom_object`](https://docs.rs/dicom-object)),
//! construct an [`AttributeOp`]
//! and apply it using [`apply`](ApplyOp::apply).
//! 
//! ```no_run
//! use dicom_core::ops::*;
//! # /* do not really import this
//! use dicom_object::open_file;
//! # */
//!
//! # struct DicomObj;
//! # impl ApplyOp for DicomObj {
//! #     type Err = snafu::Whatever;
//! #     fn apply(&mut self, _: AttributeOp) -> Result<(), Self::Err> {
//! #         panic!("this is just a stub");
//! #     }
//! # }
//! # fn open_file(_: &str) -> Result<DicomObj, Box<dyn std::error::Error>> { Ok(DicomObj) }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut obj = open_file("1/2/0003.dcm")?;
//! // hide patient name
//! obj.apply(AttributeOp {
//!     tag: (0x0010, 0x0010).into(),
//!     action: AttributeAction::SetStr("Patient^Anonymous".into()),
//! })?;
//! # Ok(())
//! # }
//! ```
use std::borrow::Cow;

use crate::{Tag, PrimitiveValue, VR};

/// Descriptor for a single operation
/// to apply over a DICOM data set.
///
/// This type is purely descriptive.
/// It outlines a non-exhaustive set of possible changes around an attribute,
/// as well as set some expectations regarding the outcome of certain actions
/// against the attribute's previous state.
///
/// The operations themselves are provided
/// alongside DICOM object or DICOM data set implementations,
/// such as the `InMemDicomObject` from the [`dicom_object`] crate.
/// 
/// Attribute operations can only select shallow attributes,
/// but the operation may be implemented when applied against nested data sets.
/// 
/// [`dicom_object`]: https://docs.rs/dicom_object
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeOp {
    /// the tag of the attribute to apply
    pub tag: Tag,
    /// the effective action to apply
    pub action: AttributeAction,
}

/// Descriptor for the kind of action to apply over an attribute.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeAction {
    /// Remove the attribute if it exists.
    Remove,
    /// If the attribute exists, clear its value to zero bytes.
    Empty,
    /// If the attribute exists,
    /// set or provide a hint about the attribute's value representation.
    ///
    /// The underlying value is not modified.
    /// Implementations are free to ignore this request if
    /// it cannot be done or it does not make sense
    /// for the given implementation.
    SetVr(VR),
    /// Fully reset the attribute with the given DICOM value,
    /// creating it if it does not exist yet.
    Set(PrimitiveValue),
    /// Fully reset a textual attribute with the given string,
    /// creating it if it does not exist yet.
    SetStr(Cow<'static, str>),
    /// Provide the attribute with the given DICOM value,
    /// if it does not exist yet.
    SetIfMissing(PrimitiveValue),
    /// Provide the textual attribute with the given string,
    /// creating it if it does not exist yet.
    SetStrIfMissing(Cow<'static, str>),
    /// Fully replace the value with the given DICOM value,
    /// but only if the attribute already exists.
    Replace(PrimitiveValue),
    /// Fully replace a textual value with the given string,
    /// but only if the attribute already exists.
    ReplaceStr(Cow<'static, str>),
    /// Append a string as an additional textual value,
    /// creating the attribute if it does not exist yet.
    ///
    /// New value items are recorded as separate text values,
    /// meaning that they are delimited by a backslash (`\`) at encoding time,
    /// regardless of the value representation.
    PushStr(Cow<'static, str>),
    /// Append a 32-bit signed integer as an additional numeric value,
    /// creating the attribute if it does not exist yet.
    PushI32(i32),
    /// Append a 32-bit unsigned integer as an additional numeric value,
    /// creating the attribute if it does not exist yet.
    PushU32(u32),
    /// Append a 16-bit signed integer as an additional numeric value,
    /// creating the attribute if it does not exist yet.
    PushI16(i16),
    /// Append a 16-bit unsigned integer as an additional numeric value,
    /// creating the attribute if it does not exist yet.
    PushU16(u16),
    /// Append a 32-bit floating point number as an additional numeric value,
    /// creating the attribute if it does not exist yet.
    PushF32(f32),
    /// Append a 64-bit floating point number as an additional numeric value,
    /// creating the attribute if it does not exist yet.
    PushF64(f64),
}

/// Trait for applying DICOM attribute operations.
/// 
/// This is typically implemented by DICOM objects and other data set types
/// to serve as a common API for attribute manipulation.
pub trait ApplyOp {
    /// The operation error type
    type Err: std::error::Error + 'static;

    /// Apply the given attribute operation on the receiving object.
    /// 
    /// Effects may slightly differ between implementations,
    /// but should always be compliant with
    /// the expectations defined in [`AttributeAction`] variants.
    /// 
    /// If the action to apply is unsupported,
    /// or not possible for other reasons,
    /// an error is returned and no changes to the receiver are made.
    fn apply(&mut self, op: AttributeOp) -> Result<(), Self::Err>;
}
