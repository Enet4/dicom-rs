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
    /// `(GGGG,EEEE)` if `Tag`,
    /// `(GGGG,EEEE)[i]` if `Nested`
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeSelectorStep::Tag(tag) => std::fmt::Display::fmt(tag, f),
            AttributeSelectorStep::Nested { tag, item } => write!(f, "{tag}[{item}]"),
        }
    }
}

/// An attribute selector.
///
/// This type defines the path to an element in a DICOM data set,
/// even at an arbitrary depth of nested data sets.
/// A selector may be perceived as a series of navigation steps
/// to reach a certain data element,
/// where all steps but the last one refer to data set sequences.
///
/// Attribute selectors can be created through
/// one of the various [`From`] conversions,
/// the dynamic constructor function [`new`],
/// or through parsing.
///
/// # Syntax
///
/// A syntax is defined for the unambiguous conversion
/// between a string and an `AttributeSelector` value,
/// in both directions.
/// Attribute selectors are defined by the syntax
/// `( «key»([«item»])? . )* «key» `
/// where:
///
/// - _`«key»`_ is either a DICOM tag in a supported textual form,
///   or a tag keyword as accepted by the [data dictionary][dict] in use;
/// - _`«item»`_ is an unsigned integer representing the item index,
///   which is always surrounded by square brackets in the input;
/// - _`[`_, _`]`_, and _`.`_ are literally their own characters
///   as part of the input.
///
/// [dict]: crate::dictionary::DataDictionary
///
/// The first part in parentheses may appear zero or more times.
/// The `[«item»]` part can be omitted,
/// in which case it is assumed that the first item is selected.
/// Whitespace is not admitted in any position.
/// Displaying a selector through the [`Display`](std::fmt::Display) trait
/// produces a string that is compliant with this syntax.
///
/// ### Examples of attribute selectors in text:
///
/// - `(0002,00010)`:
///   selects _Transfer Syntax UID_
/// - `00101010`:
///   selects _Patient Age_
/// - `0040A168[0].CodeValue`:
///   selects _Code Value_ within the first item of _Concept Code Sequence_
/// - `0040,A730[1].ContentSequence`:
///   selects _Content Sequence_ in second item of _Content Sequence_
/// - `SequenceOfUltrasoundRegions.RegionSpatialFormat`:
///   _Region Spatial Format_ in first item of _Sequence of Ultrasound Regions_
///
/// # Example
///
/// In most cases, you might only wish to select an attribute
/// that is sitting at the root of the data set.
/// This can be done by converting a [DICOM tag](crate::Tag) via [`From<Tag>`]:
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
/// if the last step refers to a sequence item.
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
/// ]).ok_or_else(|| "should be a valid sequence")?;
/// # let selector: AttributeSelector = selector;
/// # Result::<_, &'static str>::Ok(())
/// ```
///
/// A data dictionary's [`parse_selector`][parse] method
/// can be used if you want to describe these selectors in text.
///
/// ```no_run
/// # // compile only: we don't have the std dict here
/// # use dicom_core::{Tag, ops::AttributeSelector};
/// use dicom_core::dictionary::DataDictionary;
/// # use dicom_core::dictionary::stub::StubDataDictionary;
/// # /* faking an import
/// use dicom_dictionary_std::StandardDataDictionary;
/// # */
///
/// # let StandardDataDictionary = StubDataDictionary;
/// assert_eq!(
///     StandardDataDictionary.parse_selector(
///         "PerFrameFunctionalGroupsSequence[1].(0018,9074)"
///     )?,
///     AttributeSelector::from((
///         // Per-frame functional groups sequence
///         Tag(0x5200, 0x9230),
///         // item #1
///         1,
///         // Frame Acquisition Date Time (DT)
///         Tag(0x0018, 0x9074)
///     )),
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
///
/// [parse]: crate::dictionary::DataDictionary::parse_selector
///
/// Selectors can be decomposed back into its constituent steps
/// by turning it into an iterator:
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
    /// Intermediate steps of variant [`Tag`][1]
    /// (which do not specify an item index)
    /// are automatically reinterpreted as item selectors for item index 0.
    ///
    /// Returns `None` if the sequence is empty
    /// or the last step is not a tag selector step.
    ///
    /// [1]: AttributeSelectorStep::Tag
    pub fn new(steps: impl IntoIterator<Item = AttributeSelectorStep>) -> Option<Self> {
        let mut steps: SmallVec<_> = steps.into_iter().collect();
        debug_assert!(steps.len() < 256);
        let (last, rest) = steps.split_last_mut()?;
        if matches!(last, AttributeSelectorStep::Nested { .. }) {
            return None;
        }
        // transform intermediate `Tag` steps into the `Nested` variant
        for step in rest {
            if let AttributeSelectorStep::Tag(tag) = step {
                *step = AttributeSelectorStep::Nested { tag: *tag, item: 0 };
            }
        }
        Some(AttributeSelector(steps))
    }

    /// Split this attribute selector into the first step
    /// and its remainder.
    ///
    /// If the first part of the tuple is the last step of the selector,
    /// the first item will be of the variant [`Tag`](AttributeSelectorStep::Tag)
    /// and the second item of the tuple will be `None`.
    /// Otherwise,
    /// the first item is guaranteed to be of the variant
    /// [`Nested`](AttributeSelectorStep::Nested).
    pub fn split_first(&self) -> (AttributeSelectorStep, Option<AttributeSelector>) {
        match self.0.split_first() {
            Some((first, rest)) => {
                let rest = if rest.is_empty() {
                    None
                } else {
                    Some(AttributeSelector(rest.into()))
                };
                (first.clone(), rest)
            }
            None => unreachable!("invariant broken: attribute selector should have at least one step"),
        }
    }

    /// Return a non-empty iterator over the steps of attribute selection.
    ///
    /// The iterator is guaranteed to produce a series
    /// starting with zero or more steps of the variant [`Nested`][1],
    /// and terminated by one item guaranteed to be a [tag][2].
    ///
    /// [1]: AttributeSelectorStep::Nested
    /// [2]: AttributeSelectorStep::Tag
    pub fn iter(&self) -> impl Iterator<Item = &AttributeSelectorStep> {
        self.into_iter()
    }

    /// Obtain a reference to the first attribute selection step.
    pub fn first_step(&self) -> &AttributeSelectorStep {
        // guaranteed not to be empty
        self.0
            .first()
            .expect("invariant broken: attribute selector should have at least one step")
    }

    /// Obtain a reference to the last attribute selection step.
    pub fn last_step(&self) -> &AttributeSelectorStep {
        // guaranteed not to be empty
        self.0
            .last()
            .expect("invariant broken: attribute selector should have at least one step")
    }

    /// Obtain the tag of the last attribute selection step.
    pub fn last_tag(&self) -> Tag {
        match self.last_step() {
            AttributeSelectorStep::Tag(tag) => *tag,
            _ => unreachable!("invariant broken: last attribute selector step should be Tag"),
        }
    }

    /// Obtain the number of steps of the selector.
    ///
    /// Since selectors cannot be empty,
    /// the number of steps is always larger than zero.
    pub fn len(&self) -> u32 {
        self.0.len() as u32
    }
}

impl IntoIterator for AttributeSelector {
    type Item = AttributeSelectorStep;
    type IntoIter = <SmallVec<[AttributeSelectorStep; 2]> as IntoIterator>::IntoIter;

    /// Returns a non-empty iterator over the steps of attribute selection.
    ///
    /// The iterator is guaranteed to produce a series
    /// starting with zero or more steps of the variant [`Nested`][1],
    /// and terminated by one item guaranteed to be a [tag][2].
    ///
    /// [1]: AttributeSelectorStep::Nested
    /// [2]: AttributeSelectorStep::Tag
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a AttributeSelector {
    type Item = &'a AttributeSelectorStep;
    type IntoIter = <&'a SmallVec<[AttributeSelectorStep; 2]> as IntoIterator>::IntoIter;

    /// Returns a non-empty iterator over the steps of attribute selection.
    ///
    /// The iterator is guaranteed to produce a series
    /// starting with zero or more steps of the variant [`Nested`][1],
    /// and terminated by one item guaranteed to be a [tag][2].
    ///
    /// [1]: AttributeSelectorStep::Nested
    /// [2]: AttributeSelectorStep::Tag
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Creates an attribute selector for just a [`tag`](AttributeSelectorStep::Tag).
impl From<Tag> for AttributeSelector {
    /// Creates a simple attribute selector
    /// by selecting the element at the data set root with the given DICOM tag.
    fn from(tag: Tag) -> Self {
        AttributeSelector(smallvec![tag.into()])
    }
}

/// Creates an attribute selector for `tag[item].tag`
impl From<(Tag, u32, Tag)> for AttributeSelector {
    /// Creates an attribute selector
    /// which navigates to the data set item at index `item`
    /// in the sequence at the first DICOM tag (`tag0`),
    /// then selects the element with the second DICOM tag (`tag1`).
    fn from((tag0, item, tag1): (Tag, u32, Tag)) -> Self {
        AttributeSelector(smallvec![(tag0, item).into(), tag1.into()])
    }
}

/// Creates an attribute selector for `tag.tag`
/// (where the first)
impl From<(Tag, Tag)> for AttributeSelector {
    /// Creates an attribute selector
    /// which navigates to the first data set item
    /// in the sequence at the first DICOM tag (`tag0`),
    /// then selects the element with the second DICOM tag (`tag1`).
    #[inline]
    fn from((tag0, tag1): (Tag, Tag)) -> Self {
        AttributeSelector(smallvec![(tag0, 0).into(), tag1.into()])
    }
}

/// Creates an attribute selector for `tag[item].tag[item].tag`
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

/// Creates an attribute selector for `tag.tag[item].tag`
impl From<(Tag, Tag, u32, Tag)> for AttributeSelector {
    /// Creates an attribute selector
    /// which navigates to the first data set item
    /// in the sequence at `tag0`,
    /// navigates further down to item #`item1` in the sequence at `tag1`,
    /// then selects the element at `tag2`.
    fn from((tag0, tag1, item1, tag2): (Tag, Tag, u32, Tag)) -> Self {
        AttributeSelector(smallvec![
            (tag0, 0).into(),
            (tag1, item1).into(),
            tag2.into()
        ])
    }
}

/// Creates an attribute selector for `tag[item].tag.tag`
impl From<(Tag, u32, Tag, Tag)> for AttributeSelector {
    /// Creates an attribute selector
    /// which navigates to the data set item #`item0`
    /// in the sequence at `tag0`,
    /// navigates further down to the first item in the sequence at `tag1`,
    /// then selects the element at `tag2`.
    fn from((tag0, item0, tag1, tag2): (Tag, u32, Tag, Tag)) -> Self {
        AttributeSelector(smallvec![
            (tag0, item0).into(),
            (tag1, 0).into(),
            tag2.into()
        ])
    }
}

/// Creates an attribute selector for `tag.tag.tag`
impl From<(Tag, Tag, Tag)> for AttributeSelector {
    /// Creates an attribute selector
    /// which navigates to the first data set item
    /// in the sequence at `tag0`,
    /// navigates further down to the first item in the sequence at `tag1`,
    /// then selects the element at `tag2`.
    fn from((tag0, tag1, tag2): (Tag, Tag, Tag)) -> Self {
        AttributeSelector(smallvec![(tag0, 0).into(), (tag1, 0).into(), tag2.into()])
    }
}

/// Creates an attribute selector for `tag[item].tag[item].tag[item].tag`
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
    ///
    /// For objects supporting nested data sets,
    /// passing [`PrimitiveValue::Empty`] will create
    /// an empty data set sequence.
    Set(PrimitiveValue),
    /// Fully reset a textual attribute with the given string,
    /// creating it if it does not exist yet.
    SetStr(Cow<'static, str>),
    /// Provide the attribute with the given DICOM value,
    /// if it does not exist yet.
    ///
    /// For objects supporting nested data sets,
    /// passing [`PrimitiveValue::Empty`] will create
    /// an empty data set sequence.
    SetIfMissing(PrimitiveValue),
    /// Provide the textual attribute with the given string,
    /// creating it if it does not exist yet.
    SetStrIfMissing(Cow<'static, str>),
    /// Fully replace the value with the given DICOM value,
    /// but only if the attribute already exists.
    ///
    /// For objects supporting nested data sets,
    /// passing [`PrimitiveValue::Empty`] will clear the items
    /// of an existing data set sequence.
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
    /// Truncate a value or a sequence to the given number of items,
    /// removing extraneous items from the end of the list.
    ///
    /// On primitive values, this truncates the value
    /// by the number of individual value items
    /// (note that bytes in a [`PrimitiveValue::U8`]
    /// are treated as individual items).
    /// On data set sequences and pixel data fragment sequences,
    /// this operation is applied to
    /// the data set items (or fragments) in the sequence.
    ///
    /// Does nothing if the attribute does not exist
    /// or the cardinality of the element is already lower than or equal to
    /// the given size.
    Truncate(usize),
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
    use crate::{ops::{AttributeSelector, AttributeSelectorStep}, Tag};

    #[test]
    fn display_selectors() {
        let selector: AttributeSelector = Tag(0x0014, 0x5100).into();
        assert_eq!(selector.to_string(), "(0014,5100)");

        let selector: AttributeSelector = (Tag(0x0018, 0x6011), 2, Tag(0x0018, 0x6012)).into();
        assert_eq!(selector.to_string(), "(0018,6011)[2].(0018,6012)");

        let selector = AttributeSelector::from((Tag(0x0040, 0xA730), 1, Tag(0x0040, 0xA730)));
        assert_eq!(selector.to_string(), "(0040,A730)[1].(0040,A730)");
    }

    #[test]
    fn split_selectors() {
        let selector: AttributeSelector = Tag(0x0014, 0x5100).into();
        assert_eq!(
            selector.split_first(),
            (AttributeSelectorStep::Tag(Tag(0x0014, 0x5100)), None)
        );

        let selector: AttributeSelector = (Tag(0x0018, 0x6011), 2, Tag(0x0018, 0x6012)).into();
        assert_eq!(
            selector.split_first(),
            (
                AttributeSelectorStep::Nested { tag: Tag(0x0018, 0x6011), item: 2 },
                Some(Tag(0x0018, 0x6012).into())
            )
        );

        // selector constructor automatically turns the first entry into `Nested`
        let selector = AttributeSelector::from((Tag(0x0040, 0xA730), Tag(0x0040, 0xA730)));
        assert_eq!(
            selector.split_first(),
            (
                AttributeSelectorStep::Nested { tag: Tag(0x0040, 0xA730), item: 0 },
                Some(Tag(0x0040, 0xA730).into())
            )
        );

    }
}
