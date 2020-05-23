//! Interpretation of DICOM data sets as streams of tokens.
use dicom_core::header::{DataElementHeader, Length, VR};
use dicom_core::value::{DicomValueType, PrimitiveValue};
use dicom_core::{value::Value, DataElement, Tag};
use std::fmt;

pub mod read;
pub mod write;

pub use self::read::DataSetReader;
pub use self::write::DataSetWriter;

/// A token of a DICOM data set stream. This is part of the interpretation of a
/// data set as a stream of symbols, which may either represent data headers or
/// actual value data.
#[derive(Debug, Clone)]
pub enum DataToken {
    /// A data header of a primitive value.
    ElementHeader(DataElementHeader),
    /// The beginning of a sequence element.
    SequenceStart { tag: Tag, len: Length },
    /// The beginning of an encapsulated data element (Pixel Data).
    EncapsulatedElementStart,
    /// The ending delimiter of a sequence or encapsulated pixel data.
    SequenceEnd,
    /// The beginning of a new item in the sequence.
    ItemStart { len: Length },
    /// The ending delimiter of an item.
    ItemEnd,
    /// A primitive data element value.
    PrimitiveValue(PrimitiveValue),
    /// An owned piece of raw data representing an item's value.
    ///
    /// This variant is used to represent the value of an offset table or a
    /// compressed fragment. It should not be used to represent nested data
    /// sets.
    ItemValue(Vec<u8>),
}

impl fmt::Display for DataToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DataToken::PrimitiveValue(ref v) => write!(f, "PrimitiveValue({:?})", v.value_type()),
            other => write!(f, "{:?}", other),
        }
    }
}

/// This implementation treats undefined lengths as equal.
impl PartialEq<Self> for DataToken {
    fn eq(&self, other: &Self) -> bool {
        use DataToken::*;
        match (self, other) {
            (
                ElementHeader(DataElementHeader {
                    tag: tag1,
                    vr: vr1,
                    len: len1,
                }),
                ElementHeader(DataElementHeader {
                    tag: tag2,
                    vr: vr2,
                    len: len2,
                }),
            ) => tag1 == tag2 && vr1 == vr2 && len1.inner_eq(*len2),
            (
                SequenceStart {
                    tag: tag1,
                    len: len1,
                },
                SequenceStart {
                    tag: tag2,
                    len: len2,
                },
            ) => tag1 == tag2 && len1.inner_eq(*len2),
            (ItemStart { len: len1 }, ItemStart { len: len2 }) => len1.inner_eq(*len2),
            (PrimitiveValue(v1), PrimitiveValue(v2)) => v1 == v2,
            (ItemValue(v1), ItemValue(v2)) => v1 == v2,
            (ItemEnd, ItemEnd)
            | (SequenceEnd, SequenceEnd)
            | (EncapsulatedElementStart, EncapsulatedElementStart) => true,
            _ => false,
        }
    }
}

impl From<DataElementHeader> for DataToken {
    fn from(header: DataElementHeader) -> Self {
        match header.vr() {
            VR::SQ => DataToken::SequenceStart {
                tag: header.tag,
                len: header.len,
            },
            _ => DataToken::ElementHeader(header),
        }
    }
}

impl DataToken {
    /// Check whether this token represents the start of a sequence
    /// of nested data sets.
    pub fn is_sequence_start(&self) -> bool {
        match self {
            DataToken::SequenceStart { .. } => true,
            _ => false,
        }
    }

    /// Check whether this token represents the end of a sequence
    /// or the end of an encapsulated element.
    pub fn is_sequence_end(&self) -> bool {
        match self {
            DataToken::SequenceEnd => true,
            _ => false,
        }
    }
}

/// The type of delimiter: sequence or item.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SeqTokenType {
    Sequence,
    Item,
}

/// A trait for converting structured DICOM data into a stream of data tokens.
pub trait IntoTokens {
    /// The iterator type through which tokens are obtained.
    type Iter: Iterator<Item = DataToken>;

    /// Convert the value into tokens.
    fn into_tokens(self) -> Self::Iter;
}

impl IntoTokens for dicom_core::header::EmptyObject {
    type Iter = std::iter::Empty<DataToken>;

    fn into_tokens(self) -> Self::Iter {
        unreachable!()
    }
}

/// Token generator from a DICOM data element.
pub enum DataElementTokens<I>
where
    I: IntoTokens,
{
    /// initial state, at the beginning of the element
    Start(
        // Option is used for easy taking from a &mut,
        // should always be Some in practice
        Option<DataElement<I>>,
    ),
    /// header was read
    Header(
        // Option is used for easy taking from a &mut,
        // should always be Some in practice
        Option<DataElement<I>>,
    ),
    /// reading tokens from items
    Items(
        FlattenTokens<
            <dicom_core::value::C<AsItem<I>> as IntoIterator>::IntoIter,
            ItemTokens<I::Iter>,
        >,
    ),
    /// no more elements
    End,
}

impl<I> Iterator for DataElementTokens<I>
where
    I: IntoTokens,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        use DataElementTokens::*;
        let (out, next_state) = match self {
            Start(elem) => {
                let elem = elem.take().unwrap();
                // data element header token

                let token = DataToken::from(*elem.header());
                if token.is_sequence_start() {
                    // retrieve sequence value, begin item sequence
                    match elem.into_value() {
                        Value::Primitive(_) => unreachable!(),
                        Value::Sequence { items, size: _ } => {
                            let items: dicom_core::value::C<_> = items
                                .into_iter()
                                .map(|o| AsItem(Length::UNDEFINED, o))
                                .collect();
                            (Some(token), DataElementTokens::Items(items.into_tokens()))
                        }
                    }
                } else {
                    (
                        Some(DataToken::ElementHeader(*elem.header())),
                        Header(Some(elem)),
                    )
                }
            }
            Header(elem) => {
                let elem = elem.take().unwrap();
                match elem.into_value() {
                    Value::Sequence { .. } => unreachable!(),
                    Value::Primitive(value) => {
                        // return primitive value, done
                        let token = DataToken::PrimitiveValue(value);
                        (Some(token), End)
                    }
                }
            }
            Items(tokens) => {
                if let Some(token) = tokens.next() {
                    // bypass manual state transition
                    return Some(token);
                } else {
                    // sequence end token, end
                    (Some(DataToken::SequenceEnd), End)
                }
            }
            End => (None, End),
        };
        *self = next_state;

        out
    }
}

impl<I> IntoTokens for DataElement<I>
where
    I: IntoTokens,
{
    type Iter = DataElementTokens<I>;

    fn into_tokens(self) -> Self::Iter {
        DataElementTokens::Start(Some(self))
    }
}

/// Flatten a sequence of elements into their respective
/// token sequence in order.
#[derive(Debug, PartialEq)]
pub struct FlattenTokens<O, K> {
    seq: O,
    tokens: Option<K>,
}

impl<O, K> Iterator for FlattenTokens<O, K>
where
    O: Iterator,
    O::Item: IntoTokens<Iter = K>,
    K: Iterator<Item = DataToken>,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        // ensure a token sequence
        if self.tokens.is_none() {
            match self.seq.next() {
                Some(entries) => {
                    self.tokens = Some(entries.into_tokens());
                }
                None => return None,
            }
        }

        // retrieve the next token
        match self.tokens.as_mut().map(|s| s.next()) {
            Some(Some(token)) => Some(token),
            Some(None) => {
                self.tokens = None;
                self.next()
            }
            None => unreachable!(),
        }
    }
}

impl<T> IntoTokens for Vec<T>
where
    T: IntoTokens,
{
    type Iter = FlattenTokens<<Vec<T> as IntoIterator>::IntoIter, <T as IntoTokens>::Iter>;

    fn into_tokens(self) -> Self::Iter {
        FlattenTokens {
            seq: self.into_iter(),
            tokens: None,
        }
    }
}

impl<T> IntoTokens for dicom_core::value::C<T>
where
    T: IntoTokens,
{
    type Iter =
        FlattenTokens<<dicom_core::value::C<T> as IntoIterator>::IntoIter, <T as IntoTokens>::Iter>;

    fn into_tokens(self) -> Self::Iter {
        FlattenTokens {
            seq: self.into_iter(),
            tokens: None,
        }
    }
}

// A stream of tokens from a DICOM item.
#[derive(Debug)]
pub enum ItemTokens<T> {
    /// Just started, an item header token will come next
    Start {
        len: Length,
        object_tokens: Option<T>,
    },
    /// Will return tokens from the inner object, then an end of item token
    /// when it ends
    Object { object_tokens: T },
    /// Just ended, no more tokens
    End,
}

impl<T> ItemTokens<T>
where
    T: Iterator<Item = DataToken>,
{
    pub fn new<O>(len: Length, object: O) -> Self
    where
        O: IntoTokens<Iter = T>,
    {
        ItemTokens::Start {
            len,
            object_tokens: Some(object.into_tokens()),
        }
    }
}

impl<T> Iterator for ItemTokens<T>
where
    T: Iterator<Item = DataToken>,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        let (next_state, out) = match self {
            ItemTokens::Start { len, object_tokens } => (
                ItemTokens::Object {
                    object_tokens: object_tokens.take().unwrap(),
                },
                Some(DataToken::ItemStart { len: *len }),
            ),
            ItemTokens::Object { object_tokens } => {
                if let Some(token) = object_tokens.next() {
                    return Some(token);
                } else {
                    (ItemTokens::End, Some(DataToken::ItemEnd))
                }
            }
            ItemTokens::End => {
                return None;
            }
        };

        *self = next_state;
        out
    }
}

/// A newtype for interpreting the given data as an item.
/// When converting a value of this type into tokens, the inner value's tokens
/// will be surrounded by an item start and an item delimiter.
#[derive(Debug, Clone, PartialEq)]
pub struct AsItem<I>(Length, I);

impl<I> IntoTokens for AsItem<I>
where
    I: IntoTokens,
{
    type Iter = ItemTokens<I::Iter>;

    fn into_tokens(self) -> Self::Iter {
        ItemTokens::new(self.0, self.1)
    }
}
