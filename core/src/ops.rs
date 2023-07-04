//! Module for the attribute operations API.
//!
//! This allows consumers to specify and implement
//! operations on DICOM objects
//! as part of a larger process,
//! such as anonymization or transcoding.
//!
//! The most important type here is [`AttributeOp`],
//! which indicates which attribute is affected ([`AttributeSelector`]),
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
//! # use dicom_core::Tag;
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
//! obj.apply(AttributeOp::new(
//!     Tag(0x0010, 0x0010),
//!     AttributeAction::SetStr("Patient^Anonymous".into()),
//! ))?;
//! # Ok(())
//! # }
//! ```
use std::{borrow::Cow, fmt::Write};

use smallvec::{smallvec, SmallVec};

use crate::{PrimitiveValue, Tag, VR};

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
    /// the selector for the attribute to apply
    pub selector: AttributeSelector,
    /// the effective action to apply
    pub action: AttributeAction,
}

impl AttributeOp {
    /// Construct an attribute operation.
    ///
    /// This constructor function may be easier to use
    /// than writing a public struct expression directly,
    /// due to its automatic conversion of `selector`.
    ///
    /// # Example
    ///
    /// ```
    /// # use dicom_core::Tag;
    /// # use dicom_core::ops::{AttributeAction, AttributeOp};
    /// let op = AttributeOp::new(
    ///     // ImageType
    ///     Tag(0x0008, 0x0008),
    ///     AttributeAction::SetStr("DERIVED\\SECONDARY\\DOSE_INFO".into()),
    /// );
    /// ```
    pub fn new(selector: impl Into<AttributeSelector>, action: AttributeAction) -> Self {
        AttributeOp {
            selector: selector.into(),
            action,
        }
    }
}

/// A single step of an attribute selection.
///
/// A selector step may either select an element directly at the root (`Tag`)
/// or a specific item in a sequence to navigate into (`Nested`).
///
/// A full attribute selector can be specified
/// by using a sequence of these steps
/// (but should always end with the `Tag` variant,
/// otherwise the operation would be unspecified).
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
pub enum AttributeSelectorStep {
    /// Select the element with the tag reachable at the root of this data set
    Tag(Tag),
    /// Select an item in a data set sequence,
    /// as an intermediate step
    Nested { tag: Tag, item: u32 },
}

impl From<Tag> for AttributeSelectorStep {
    /// Creates an attribute selector step by data element tag.
    fn from(value: Tag) -> Self {
        AttributeSelectorStep::Tag(value)
    }
}

impl From<(Tag, u32)> for AttributeSelectorStep {
    /// Creates a sequence item selector step
    /// by data element tag and item index.
    fn from((tag, item): (Tag, u32)) -> Self {
        AttributeSelectorStep::Nested { tag, item }
    }
}

impl std::fmt::Display for AttributeSelectorStep {
    /// Displays the attribute selector step:
    /// `(GGGG,EEEE)` if `Tag`,,
    /// `(GGGG,EEEE)[i]` if `Nested`
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeSelectorStep::Tag(tag) => std::fmt::Display::fmt(tag, f),
            AttributeSelectorStep::Nested { tag, item } => write!(f, "{}[{}]", tag, item),
        }
    }
}

/// An attribute selector.
///
/// This type defines a unique element in a DICOM data set,
/// even at an arbitrary depth of nested data sets.
///
/// # Example
///
/// In most cases, you might only wish to select an attribute
/// that is sitting at the root of the data set.
/// Conversion from a DICOM tag is possible via [`From<Tag>`]:
///
/// ```
/// # use dicom_core::Tag;
/// # use dicom_core::ops::AttributeSelector;
/// // select Patient Name
/// let selector = AttributeSelector::from(Tag(0x0010, 0x0010));
/// ```
///
/// For working with nested data sets,
/// `From` also supports converting
/// an interleaved sequence of tags and item indices in a tuple.
/// For instance,
/// this is how we can select the second frame's acquisition date time
/// from the per-frame functional groups sequence.
///
/// ```
/// # use dicom_core::Tag;
/// # use dicom_core::ops::AttributeSelector;
/// let selector: AttributeSelector = (
///     // Per-frame functional groups sequence
///     Tag(0x5200, 0x9230),
///     // item #1
///     1,
///     // Frame Acquisition Date Time (DT)
///     Tag(0x0018, 0x9074)
/// ).into();
/// ```
///
/// For a more dynamic construction,
/// the [`new`] function supports an iterator of attribute selector steps
/// (of type [`AttributeSelectorStep`]).
/// Note that the function fails
/// if the last step refers to a sequence item
/// or any of the other steps are not item selectors.
///
/// [`new`]: AttributeSelector::new
///
/// ```
/// # use dicom_core::Tag;
/// # use dicom_core::ops::{AttributeSelector, AttributeSelectorStep};
/// let selector = AttributeSelector::new([
///     // Per-frame functional groups sequence, item #1
///     AttributeSelectorStep::Nested {
///         tag: Tag(0x5200, 0x9230),
///         // item #1
///         item: 1,
///     },
///     // Frame Acquisition Date Time
///     AttributeSelectorStep::Tag(Tag(0x0018, 0x9074)),
/// ]);
/// ```
///
/// A data dictionary's [`parse_selector`][parse] method
/// can be used if you want to describe these selectors in text.
///
/// [parse]: crate::dictionary::DataDictionary::parse_selector
///
/// Selectors can be decomposed back into its parts
/// by using it as an iterator:
///
/// ```
/// # use dicom_core::Tag;
/// # use dicom_core::ops::{AttributeSelector, AttributeSelectorStep};
/// # let selector = AttributeSelector::from(
/// #     (Tag(0x5200, 0x9230), 1, Tag(0x0018, 0x9074)));
/// let steps: Vec<AttributeSelectorStep> = selector.into_iter().collect();
///
/// assert_eq!(
///     &steps,
///     &[
///         AttributeSelectorStep::Nested {
///             tag: Tag(0x5200, 0x9230),
///             item: 1,
///         },
///         AttributeSelectorStep::Tag(Tag(0x0018, 0x9074)),
///     ],
/// );
/// ```
///
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct AttributeSelector(SmallVec<[AttributeSelectorStep; 2]>);

impl AttributeSelector {
    /// Construct an attribute selector
    /// from an arbitrary sequence of selector steps.
    ///
    /// Returns `None` if the sequence is empty,
    /// the intermediate items do not represent item selector steps,
    /// or the last step is not a tag selector step.
    pub fn new(steps: impl IntoIterator<Item = AttributeSelectorStep>) -> Option<Self> {
        let steps: SmallVec<_> = steps.into_iter().collect();
        let Some((last, rest)) = steps.split_last() else {
            return None;
        };
        if matches!(last, AttributeSelectorStep::Nested { .. }) {
            return None;
        }
        if rest.iter().any(|step| matches!(step, AttributeSelectorStep::Tag(_))) {
            return None;
        }
        Some(AttributeSelector(steps))
    }

    /// Return a non-empty iterator over the steps of attribute selection.
    ///
    /// The iterator is guaranteed to produce at least one item,
    /// and the last one is guaranteed to be a [tag][1].
    ///
    /// [1]: AttributeSelectorStep::Tag
    pub fn iter(&self) -> impl Iterator<Item = &AttributeSelectorStep> {
        self.into_iter()
    }

    /// Obtain a reference to the first attribute selection step.
    pub fn first_step(&self) -> &AttributeSelectorStep {
        // guaranteed not to be empty
        &self.0[0]
    }
}

impl IntoIterator for AttributeSelector {
    type Item = AttributeSelectorStep;
    type IntoIter = <SmallVec<[AttributeSelectorStep; 2]> as IntoIterator>::IntoIter;

    /// Returns a non-empty iterator over the steps of attribute selection.
    ///
    /// The iterator is guaranteed to produce at least one item,
    /// and the last one is guaranteed to be a [tag][1].
    ///
    /// [1]: AttributeSelectorStep::Tag
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a AttributeSelector {
    type Item = &'a AttributeSelectorStep;
    type IntoIter = <&'a SmallVec<[AttributeSelectorStep; 2]> as IntoIterator>::IntoIter;

    /// Returns a non-empty iterator over the steps of attribute selection.
    ///
    /// The iterator is guaranteed to produce at least one item,
    /// and the last one is guaranteed to be a [tag][1].
    ///
    /// [1]: AttributeSelectorStep::Tag
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Creates an attibute selector for `tag`
impl From<Tag> for AttributeSelector {
    /// Creates a simple attribute selector
    /// by selecting the element at the data set root with the given DICOM tag.
    fn from(tag: Tag) -> Self {
        AttributeSelector(smallvec![tag.into()])
    }
}

/// Creates an attibute selector for `tag[item].tag`
impl From<(Tag, u32, Tag)> for AttributeSelector {
    /// Creates an attribute selector
    /// which navigates to the data set item at index `item`
    /// in the sequence at the first DICOM tag (`tag0`),
    /// then selects the element with the second DICOM tag (`tag1`).
    fn from((tag0, item, tag1): (Tag, u32, Tag)) -> Self {
        AttributeSelector(smallvec![(tag0, item).into(), tag1.into()])
    }
}

/// Creates an attibute selector for `tag[item].tag[item].tag`
impl From<(Tag, u32, Tag, u32, Tag)> for AttributeSelector {
    /// Creates an attribute selector
    /// which navigates to data set item #`item0`
    /// in the sequence at `tag0`,
    /// navigates further down to item #`item1` in the sequence at `tag1`,
    /// then selects the element at `tag2`.
    fn from((tag0, item0, tag1, item1, tag2): (Tag, u32, Tag, u32, Tag)) -> Self {
        AttributeSelector(smallvec![
            (tag0, item0).into(),
            (tag1, item1).into(),
            tag2.into()
        ])
    }
}

/// Creates an attibute selector for `tag[item].tag[item].tag[item].tag`
impl From<(Tag, u32, Tag, u32, Tag, u32, Tag)> for AttributeSelector {
    // you should get the gist at this point
    fn from(
        (tag0, item0, tag1, item1, tag2, item2, tag3): (Tag, u32, Tag, u32, Tag, u32, Tag),
    ) -> Self {
        AttributeSelector(smallvec![
            (tag0, item0).into(),
            (tag1, item1).into(),
            (tag2, item2).into(),
            tag3.into()
        ])
    }
}

impl std::fmt::Display for AttributeSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut started = false;
        for step in &self.0 {
            if !started {
                started = true;
            } else {
                // separate each step by a dot
                f.write_char('.')?;
            }
            std::fmt::Display::fmt(step, f)?;
        }
        Ok(())
    }
}

/// Descriptor for the kind of action to apply over an attribute.
///
/// See the [module-level documentation](crate::ops)
/// for more details.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeAction {
    /// Remove the attribute if it exists.
    ///
    /// Do nothing otherwise.
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

impl AttributeAction {
    /// Report whether this is considered a _constructive_ action,
    /// operations of which create new elements if they do not exist yet.
    ///
    /// The actions currently considered to be constructive are
    /// all actions of the families `Set*`, `SetIfMissing`, and `Push*`.
    pub fn is_constructive(&self) -> bool {
        matches!(
            self,
            AttributeAction::Set(_)
                | AttributeAction::SetStr(_)
                | AttributeAction::SetIfMissing(_)
                | AttributeAction::SetStrIfMissing(_)
                | AttributeAction::PushF32(_)
                | AttributeAction::PushF64(_)
                | AttributeAction::PushI16(_)
                | AttributeAction::PushI32(_)
                | AttributeAction::PushStr(_)
                | AttributeAction::PushU16(_)
                | AttributeAction::PushU32(_)
        )
    }
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
    /// While not all kinds of operations may be possible,
    /// generic DICOM data set holders will usually support all actions.
    /// See the respective documentation of the implementing type
    /// for more details.
    fn apply(&mut self, op: AttributeOp) -> Result<(), Self::Err>;
}

#[cfg(test)]
mod tests {
    use crate::{ops::AttributeSelector, Tag};

    #[test]
    fn display_selectors() {
        let selector: AttributeSelector = Tag(0x0014, 0x5100).into();
        assert_eq!(selector.to_string(), "(0014,5100)",);

        let selector: AttributeSelector = (Tag(0x0018, 0x6011), 2, Tag(0x0018, 0x6012)).into();
        assert_eq!(selector.to_string(), "(0018,6011)[2].(0018,6012)",);

        let selector = AttributeSelector::from((Tag(0x0040, 0xA730), 1, Tag(0x0040, 0xA730)));
        assert_eq!(selector.to_string(), "(0040,A730)[1].(0040,A730)",);
    }
}
