//! Module for attribute operation descriptors.
//! 
//! This allows consumers to specify and implement
//! operations on DICOM objects
//! as part of a larger process,
//! such as anonymization or transcoding.
//! 
//! The most important type here is [`AttributeOp`],
//! which is a full operation descriptor,
//! indicating the attribute to select by its DICOM tag
//! and the operation to apply ([`AttributeAction`]).
use std::borrow::Cow;

use dicom_core::{Tag, PrimitiveValue, VR};

/// Descriptor for a single operation
/// to apply over a DICOM data set.
///
/// This type is purely descriptive.
/// It outlines a non-exhaustive set of possible changes around an attribute,
/// as well as set some expectations  .
///
/// The operations themselves are provided
/// alongside DICOM objec or DICOM data set implementations.
/// 
/// Attribute operations can only select shallow attributes,
/// but the operation may be implemented when applied against nested data sets.
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
    /// Remove the attribute
    Remove,
    /// Clear the attribute,
    /// leaving it empty if it exists already.
    Empty,
    /// Set or provide a hint about an attribute's value representation,
    /// if it exists.
    ///
    /// The underlying value is not modified.
    /// Implementations are free to ignore this request
    /// if it cannot be done or does not make sense for the given tag.
    SetVr(VR),
    /// Fully replace the value with the given DICOM value,
    /// creating the element if it does not exist yet.
    Replace(PrimitiveValue),
    /// Fully replace a textual value with the given string
    ReplaceStr(Cow<'static, str>),
    /// Append a string as an additional textual value,
    /// creating the element if it does not exist yet.
    PushStr(Cow<'static, str>),
    /// Append a 32-bit signed integer as an additional numeric value,
    /// creating the element if it does not exist yet.
    PushI32(i32),
    /// Append a 32-bit unsigned integer as an additional numeric value,
    /// creating the element if it does not exist yet.
    PushU32(u32),
    /// Append a 16-bit signed integer as an additional numeric value,
    /// creating the element if it does not exist yet.
    PushI16(i16),
    /// Append a 16-bit unsigned integer as an additional numeric value,
    /// creating the element if it does not exist yet.
    PushU16(u16),
    /// Append a 32-bit floating point number as an additional numeric value,
    /// creating the element if it does not exist yet.
    PushF32(f32),
    /// Append a 64-bit floating point number as an additional numeric value,
    /// creating the element if it does not exist yet.
    PushF64(f64),
}
