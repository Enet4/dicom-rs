//! Interpretation of DICOM data sets as streams of tokens.
use crate::stateful::decode;
use dicom_core::header::{DataElementHeader, HasLength, Length, VR};
use dicom_core::value::{DicomValueType, PrimitiveValue};
use dicom_core::{value::Value, DataElement, Tag};
use snafu::{OptionExt, ResultExt, Snafu};
use std::fmt;

pub mod lazy_read;
pub mod read;
pub mod write;

pub use self::read::DataSetReader;
use self::read::ValueReadStrategy;
pub use self::write::DataSetWriter;

#[derive(Debug, Snafu)]
pub enum Error {
    /// Could not read item value
    ReadItemValue { source: decode::Error },
    /// Could not read element value
    ReadElementValue { source: decode::Error },
    /// Could not skip the bytes of a value
    SkipValue { source: decode::Error },
    /// Unexpected token type for operation
    UnexpectedTokenType,
    /// Unexpected undefined value length
    UndefinedLength,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A token of a DICOM data set stream. This is part of the interpretation of a
/// data set as a stream of symbols, which may either represent data headers or
/// actual value data.
#[derive(Debug, Clone)]
pub enum DataToken {
    /// A data header of a primitive value.
    ElementHeader(DataElementHeader),
    /// The beginning of a sequence element.
    SequenceStart { tag: Tag, len: Length },
    /// The beginning of an encapsulated pixel data element.
    PixelSequenceStart,
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
    /// This variant is used to represent
    /// the value of an encoded fragment.
    /// It should not be used to represent nested data sets.
    ItemValue(Vec<u8>),
    /// An owned sequence of unsigned 32 bit integers
    /// representing a pixel data offset table.
    ///
    /// This variant is used to represent
    /// the byte offsets to the first byte of the Item tag of the first fragment
    /// for each frame in the sequence of items,
    /// as per PS 3.5, Section A.4.
    OffsetTable(Vec<u32>),
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
            (OffsetTable(v1), OffsetTable(v2)) => v1 == v2,
            (ItemEnd, ItemEnd)
            | (SequenceEnd, SequenceEnd)
            | (PixelSequenceStart, PixelSequenceStart) => true,
            _ => false,
        }
    }
}

impl From<DataElementHeader> for DataToken {
    fn from(header: DataElementHeader) -> Self {
        match (header.vr(), header.tag) {
            (VR::OB, Tag(0x7fe0, 0x0010)) if header.len.is_undefined() => {
                DataToken::PixelSequenceStart
            }
            (VR::SQ, _) => DataToken::SequenceStart {
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
        matches!(self, DataToken::SequenceStart { .. })
    }

    /// Check whether this token represents the end of a sequence
    /// or the end of an encapsulated element.
    pub fn is_sequence_end(&self) -> bool {
        matches!(self, DataToken::SequenceEnd)
    }
}

/// A lazy data token for reading a data set
/// without requiring values to be fully read in memory.
/// This is part of the interpretation of a
/// data set as a stream of symbols,
/// which may either represent data headers
/// or actual value data.
///
/// The parameter type `D` represents
/// the original type of the stateful decoder,
/// and through which the values can be retrieved.
#[derive(Debug)]
#[non_exhaustive]
pub enum LazyDataToken<D> {
    /// A data header of a primitive value.
    ElementHeader(DataElementHeader),
    /// The beginning of a sequence element.
    SequenceStart { tag: Tag, len: Length },
    /// The beginning of an encapsulated pixel data element.
    PixelSequenceStart,
    /// The ending delimiter of a sequence or encapsulated pixel data.
    SequenceEnd,
    /// The beginning of a new item in the sequence.
    ItemStart { len: Length },
    /// The ending delimiter of an item.
    ItemEnd,
    /// An element value yet to be fetched
    LazyValue {
        /// the header of the respective value
        header: DataElementHeader,
        /// the stateful decoder for fetching the bytes of the value
        decoder: D,
    },
    /// An item value yet to be fetched
    LazyItemValue {
        /// the full length of the value, always well defined
        len: u32,
        /// the stateful decoder for fetching the bytes of the value
        decoder: D,
    },
}

impl<D> LazyDataToken<D> {
    /// Check whether this token represents the start of a sequence
    /// of nested data sets.
    pub fn is_sequence_start(&self) -> bool {
        matches!(self, LazyDataToken::SequenceStart { .. })
    }

    /// Check whether this token represents the end of a sequence
    /// or the end of an encapsulated element.
    pub fn is_sequence_end(&self) -> bool {
        matches!(self, LazyDataToken::SequenceEnd)
    }
}

impl<D> LazyDataToken<D>
where
    D: decode::StatefulDecode,
{
    pub fn skip(self) -> Result<()> {
        match self {
            LazyDataToken::LazyValue {
                header,
                mut decoder,
            } => decoder.skip_bytes(header.len.0).context(SkipValue),
            LazyDataToken::LazyItemValue { len, mut decoder } => {
                decoder.skip_bytes(len).context(SkipValue)
            }
            _ => Ok(()), // do nothing
        }
    }
    /// Construct the data token into memory,
    /// consuming the reader if necessary.
    ///
    /// If the token represents a lazy element value,
    /// the inner decoder is read with string preservation.
    pub fn into_owned(self) -> Result<DataToken> {
        self.into_owned_with_strategy(ValueReadStrategy::Preserved)
    }

    /// Construct the data token into memory,
    /// consuming the reader if necessary.
    ///
    /// If the token represents a lazy element value,
    /// the inner decoder is read
    /// with the given value reading strategy.
    pub fn into_owned_with_strategy(self, strategy: ValueReadStrategy) -> Result<DataToken> {
        match self {
            LazyDataToken::ElementHeader(header) => Ok(DataToken::ElementHeader(header)),
            LazyDataToken::ItemEnd => Ok(DataToken::ItemEnd),
            LazyDataToken::ItemStart { len } => Ok(DataToken::ItemStart { len }),
            LazyDataToken::PixelSequenceStart => Ok(DataToken::PixelSequenceStart),
            LazyDataToken::SequenceEnd => Ok(DataToken::SequenceEnd),
            LazyDataToken::SequenceStart { tag, len } => Ok(DataToken::SequenceStart { tag, len }),
            LazyDataToken::LazyValue {
                header,
                mut decoder,
            } => {
                // use the stateful decoder to eagerly read the value
                let value = match strategy {
                    ValueReadStrategy::Interpreted => {
                        decoder.read_value(&header).context(ReadElementValue)?
                    }
                    ValueReadStrategy::Preserved => decoder
                        .read_value_preserved(&header)
                        .context(ReadElementValue)?,
                    ValueReadStrategy::Raw => decoder
                        .read_value_bytes(&header)
                        .context(ReadElementValue)?,
                };
                Ok(DataToken::PrimitiveValue(value))
            }
            LazyDataToken::LazyItemValue { len, mut decoder } => {
                let mut data = Vec::new();
                decoder.read_to_vec(len, &mut data).context(ReadItemValue)?;
                Ok(DataToken::ItemValue(data))
            }
        }
    }

    /// Retrieve a primitive element value from the token,
    /// consuming the reader with the given reading strategy.
    ///
    /// The operation fails if the token does not represent an element value.
    pub fn into_value_with_strategy(self, strategy: ValueReadStrategy) -> Result<PrimitiveValue> {
        match self {
            LazyDataToken::LazyValue {
                header,
                mut decoder,
            } => {
                // use the stateful decoder to eagerly read the value
                match strategy {
                    ValueReadStrategy::Interpreted => {
                        decoder.read_value(&header).context(ReadElementValue)
                    }
                    ValueReadStrategy::Preserved => decoder
                        .read_value_preserved(&header)
                        .context(ReadElementValue),
                    ValueReadStrategy::Raw => {
                        decoder.read_value_bytes(&header).context(ReadElementValue)
                    }
                }
            }
            _ => UnexpectedTokenType.fail(),
        }
    }

    /// Retrieve a primitive element value from the token,
    /// consuming the reader with the default reading strategy.
    ///
    /// The operation fails if the token does not represent an element value.
    pub fn into_value(self) -> Result<PrimitiveValue> {
        self.into_value_with_strategy(ValueReadStrategy::Preserved)
    }

    /// Read the bytes of a value into the given writer,
    /// consuming the reader.
    ///
    /// This operation will not interpret the value,
    /// like in the `Bytes` value reading strategy.
    /// It works for both data elements and non-dataset items.
    ///
    /// The operation fails if
    /// the token does not represent an element or item value.
    pub fn read_value_into<W>(self, out: W) -> Result<()>
    where
        W: std::io::Write,
    {
        match self {
            LazyDataToken::LazyValue {
                header,
                mut decoder,
            } => {
                let len = header.len.get().context(UndefinedLength)?;
                decoder.read_to(len, out).context(ReadElementValue)?;
            }
            LazyDataToken::LazyItemValue { len, mut decoder } => {
                decoder.read_to(len, out).context(ReadItemValue)?;
            }
            _other => return UnexpectedTokenType.fail(),
        };
        Ok(())
    }

    /// Convert this token into a structured representation,
    /// for diagnostics and error reporting purposes.
    pub fn into_repr(self) -> LazyDataTokenRepr {
        LazyDataTokenRepr::from(self)
    }

    /// Create a structured representation of this token,
    /// for diagnostics and error reporting purposes.
    pub fn repr(&self) -> LazyDataTokenRepr {
        LazyDataTokenRepr::from(self)
    }
}

impl<D> From<LazyDataToken<D>> for LazyDataTokenRepr {
    fn from(token: LazyDataToken<D>) -> Self {
        match token {
            LazyDataToken::ElementHeader(h) => LazyDataTokenRepr::ElementHeader(h),
            LazyDataToken::SequenceStart { tag, len } => {
                LazyDataTokenRepr::SequenceStart { tag, len }
            }
            LazyDataToken::PixelSequenceStart => LazyDataTokenRepr::PixelSequenceStart,
            LazyDataToken::SequenceEnd => LazyDataTokenRepr::SequenceEnd,
            LazyDataToken::ItemStart { len } => LazyDataTokenRepr::ItemStart { len },
            LazyDataToken::ItemEnd => LazyDataTokenRepr::ItemEnd,
            LazyDataToken::LazyValue { header, decoder: _ } => {
                LazyDataTokenRepr::LazyValue { header }
            }
            LazyDataToken::LazyItemValue { len, decoder: _ } => {
                LazyDataTokenRepr::LazyItemValue { len }
            }
        }
    }
}

impl<D> From<&LazyDataToken<D>> for LazyDataTokenRepr {
    fn from(token: &LazyDataToken<D>) -> Self {
        match *token {
            LazyDataToken::ElementHeader(h) => LazyDataTokenRepr::ElementHeader(h),
            LazyDataToken::SequenceStart { tag, len } => {
                LazyDataTokenRepr::SequenceStart { tag, len }
            }
            LazyDataToken::PixelSequenceStart => LazyDataTokenRepr::PixelSequenceStart,
            LazyDataToken::SequenceEnd => LazyDataTokenRepr::SequenceEnd,
            LazyDataToken::ItemStart { len } => LazyDataTokenRepr::ItemStart { len },
            LazyDataToken::ItemEnd => LazyDataTokenRepr::ItemEnd,
            LazyDataToken::LazyValue { header, decoder: _ } => {
                LazyDataTokenRepr::LazyValue { header }
            }
            LazyDataToken::LazyItemValue { len, decoder: _ } => {
                LazyDataTokenRepr::LazyItemValue { len }
            }
        }
    }
}

/// A structured description of a lazy data token,
/// for diagnostics and error reporting purposes.
#[derive(Debug, Clone, PartialEq)]
pub enum LazyDataTokenRepr {
    /// A data header of a primitive value.
    ElementHeader(DataElementHeader),
    /// The beginning of a sequence element.
    SequenceStart { tag: Tag, len: Length },
    /// The beginning of an encapsulated pixel data element.
    PixelSequenceStart,
    /// The ending delimiter of a sequence or encapsulated pixel data.
    SequenceEnd,
    /// The beginning of a new item in the sequence.
    ItemStart { len: Length },
    /// The ending delimiter of an item.
    ItemEnd,
    /// An element value yet to be fetched
    LazyValue {
        /// the header of the respective value
        header: DataElementHeader,
    },
    /// An item value yet to be fetched
    LazyItemValue {
        /// the full length of the value, always well defined
        len: u32,
    },
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
pub enum DataElementTokens<I, P>
where
    I: IntoTokens,
{
    /// initial state, at the beginning of the element
    Start(
        // Option is used for easy taking from a &mut,
        // should always be Some in practice
        Option<DataElement<I, P>>,
    ),
    /// the header of a plain primitive element was read
    Header(
        // Option is used for easy taking from a &mut,
        // should always be Some in practice
        Option<DataElement<I, P>>,
    ),
    /// reading tokens from items
    Items(
        FlattenTokens<
            <dicom_core::value::C<AsItem<I>> as IntoIterator>::IntoIter,
            ItemTokens<I::Iter>,
        >,
    ),
    /// the header of encapsulated pixel data was read, will read
    /// the offset table next
    PixelData(
        /// Pixel fragments
        ///
        /// Option is used for easy taking from a &mut,
        /// should always be Some in practice
        Option<dicom_core::value::C<P>>,
        /// Frame offset table
        OffsetTableItemTokens<dicom_core::value::C<u32>>,
    ),
    /// the header and offset of encapsulated pixel data was read,
    /// fragments come next
    PixelDataFragments(
        FlattenTokens<
            <dicom_core::value::C<ItemValue<P>> as IntoIterator>::IntoIter,
            ItemValueTokens<P>,
        >,
    ),
    /// no more elements
    End,
}

impl<I, P> Iterator for DataElementTokens<I, P>
where
    I: IntoTokens,
    I: HasLength,
    P: AsRef<[u8]>,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        let (out, next_state) = match self {
            DataElementTokens::Start(elem) => {
                let elem = elem.take().unwrap();
                // data element header token
                let header = *elem.header();

                let token = DataToken::from(header);
                match token {
                    DataToken::SequenceStart { .. } => {
                        // retrieve sequence value, begin item sequence
                        match elem.into_value() {
                            Value::Primitive(_) | Value::PixelSequence { .. } => unreachable!(),
                            Value::Sequence { items, size: _ } => {
                                let items: dicom_core::value::C<_> =
                                    items.into_iter().map(|o| AsItem(o.length(), o)).collect();
                                (Some(token), DataElementTokens::Items(items.into_tokens()))
                            }
                        }
                    }
                    DataToken::PixelSequenceStart => {
                        match elem.into_value() {
                            Value::PixelSequence {
                                fragments,
                                offset_table,
                            } => {
                                (
                                    // begin pixel sequence
                                    Some(DataToken::PixelSequenceStart),
                                    DataElementTokens::PixelData(
                                        Some(fragments),
                                        OffsetTableItem(offset_table).into_tokens(),
                                    ),
                                )
                            }
                            Value::Primitive(_) | Value::Sequence { .. } => unreachable!(),
                        }
                    }
                    _ => (
                        Some(DataToken::ElementHeader(*elem.header())),
                        DataElementTokens::Header(Some(elem)),
                    ),
                }
            }
            DataElementTokens::Header(elem) => {
                let elem = elem.take().unwrap();
                match elem.into_value() {
                    Value::Sequence { .. } | Value::PixelSequence { .. } => unreachable!(),
                    Value::Primitive(value) => {
                        // return primitive value, done
                        let token = DataToken::PrimitiveValue(value);
                        (Some(token), DataElementTokens::End)
                    }
                }
            }
            DataElementTokens::Items(tokens) => {
                if let Some(token) = tokens.next() {
                    // bypass manual state transition
                    return Some(token);
                } else {
                    // sequence end token, end
                    (Some(DataToken::SequenceEnd), DataElementTokens::End)
                }
            }
            DataElementTokens::PixelData(fragments, tokens) => {
                if let Some(token) = tokens.next() {
                    // bypass manual state transition
                    return Some(token);
                }
                // pixel data fragments next
                let fragments = fragments.take().unwrap();
                let tokens: dicom_core::value::C<_> =
                    fragments.into_iter().map(ItemValue).collect();
                *self = DataElementTokens::PixelDataFragments(tokens.into_tokens());
                // recursive call to ensure the retrieval of a data token
                return self.next();
            }
            DataElementTokens::PixelDataFragments(tokens) => {
                if let Some(token) = tokens.next() {
                    // bypass manual state transition
                    return Some(token);
                } else {
                    // sequence end token, end
                    (Some(DataToken::SequenceEnd), DataElementTokens::End)
                }
            }
            DataElementTokens::End => return None,
        };
        *self = next_state;

        out
    }
}

impl<I, P> IntoTokens for DataElement<I, P>
where
    I: IntoTokens,
    I: HasLength,
    P: AsRef<[u8]>,
{
    type Iter = DataElementTokens<I, P>;

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

impl<I> HasLength for AsItem<I> {
    fn length(&self) -> Length {
        self.0
    }
}

/// A newtype for wrapping a piece of raw data into an item.
/// When converting a value of this type into tokens, the algorithm
/// will create an item start with an explicit length, followed by
/// an item value token, then an item delimiter.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemValue<P>(P);

impl<P> IntoTokens for ItemValue<P>
where
    P: AsRef<[u8]>,
{
    type Iter = ItemValueTokens<P>;

    fn into_tokens(self) -> Self::Iter {
        ItemValueTokens::new(self.0)
    }
}

#[derive(Debug)]
pub enum ItemValueTokens<P> {
    /// Just started, an item header token will come next
    Start(Option<P>),
    /// Will return a token of the value
    Value(P),
    /// Will return an end of item token
    Done,
    /// Just ended, no more tokens
    End,
}

impl<P> ItemValueTokens<P> {
    #[inline]
    pub fn new(value: P) -> Self {
        ItemValueTokens::Start(Some(value))
    }
}

impl<P> Iterator for ItemValueTokens<P>
where
    P: AsRef<[u8]>,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        let (out, next_state) = match self {
            ItemValueTokens::Start(value) => {
                let value = value.take().unwrap();
                let len = Length(value.as_ref().len() as u32);

                (
                    Some(DataToken::ItemStart { len }),
                    if len == Length(0) {
                        ItemValueTokens::Done
                    } else {
                        ItemValueTokens::Value(value)
                    },
                )
            }
            ItemValueTokens::Value(value) => (
                Some(DataToken::ItemValue(value.as_ref().to_owned())),
                ItemValueTokens::Done,
            ),
            ItemValueTokens::Done => (Some(DataToken::ItemEnd), ItemValueTokens::End),
            ItemValueTokens::End => return None,
        };

        *self = next_state;
        out
    }
}

/// A newtype for wrapping a sequence of `u32`s into an offset table item.
/// When converting a value of this type into tokens,
/// the algorithm will create an item start with an explicit length,
/// followed by an item value token,
/// then an item delimiter.
#[derive(Debug, Clone, PartialEq)]
pub struct OffsetTableItem<P>(P);

impl<P> IntoTokens for OffsetTableItem<P>
where
    P: AsRef<[u32]>,
{
    type Iter = OffsetTableItemTokens<P>;

    fn into_tokens(self) -> Self::Iter {
        OffsetTableItemTokens::new(self.0)
    }
}

#[derive(Debug)]
pub enum OffsetTableItemTokens<P> {
    /// Just started, an item header token will come next
    Start(Option<P>),
    /// Will return a token of the actual offset table
    Value(P),
    /// Will return an end of item token
    Done,
    /// Just ended, no more tokens
    End,
}

impl<P> OffsetTableItemTokens<P> {
    #[inline]
    pub fn new(value: P) -> Self {
        OffsetTableItemTokens::Start(Some(value))
    }
}

impl<P> Iterator for OffsetTableItemTokens<P>
where
    P: AsRef<[u32]>,
{
    type Item = DataToken;

    fn next(&mut self) -> Option<Self::Item> {
        let (out, next_state) = match self {
            OffsetTableItemTokens::Start(value) => {
                let value = value.take().unwrap();
                let len = Length(value.as_ref().len() as u32 * 4);

                (
                    Some(DataToken::ItemStart { len }),
                    if len == Length(0) {
                        OffsetTableItemTokens::Done
                    } else {
                        OffsetTableItemTokens::Value(value)
                    },
                )
            }
            OffsetTableItemTokens::Value(value) => (
                Some(DataToken::OffsetTable(value.as_ref().to_owned())),
                OffsetTableItemTokens::Done,
            ),
            OffsetTableItemTokens::Done => (Some(DataToken::ItemEnd), OffsetTableItemTokens::End),
            OffsetTableItemTokens::End => return None,
        };

        *self = next_state;
        out
    }
}

#[cfg(test)]
mod tests {
    use dicom_core::{
        dicom_value, header::HasLength, DataElement, DataElementHeader, DicomValue, Length,
        PrimitiveValue, Tag, VR,
    };

    use super::{DataToken, IntoTokens, LazyDataToken};
    use smallvec::smallvec;

    use dicom_encoding::{
        decode::{basic::LittleEndianBasicDecoder, explicit_le::ExplicitVRLittleEndianDecoder},
        text::SpecificCharacterSet,
    };

    use crate::stateful::decode::StatefulDecode;
    use crate::stateful::decode::StatefulDecoder;

    fn is_stateful_decode<D: StatefulDecode>(_: &D) {}

    /// A simple object representing a DICOM data set,
    /// used merely for testing purposes.
    #[derive(Debug, Clone)]
    struct SimpleObject<T>(Length, dicom_core::value::C<T>);

    impl<T> HasLength for SimpleObject<T> {
        fn length(&self) -> Length {
            self.0
        }
    }

    impl<T> IntoTokens for SimpleObject<T>
    where
        T: IntoTokens,
        T: HasLength,
    {
        type Iter = super::FlattenTokens<
            <dicom_core::value::C<T> as IntoIterator>::IntoIter,
            <T as IntoTokens>::Iter,
        >;

        fn into_tokens(self) -> Self::Iter {
            super::FlattenTokens {
                seq: self.1.into_iter(),
                tokens: None,
            }
        }
    }

    #[test]
    fn basic_element_into_tokens() {
        let element = DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            DicomValue::new("Doe^John".into()),
        );

        let tokens: Vec<_> = element.clone().into_tokens().collect();

        assert_eq!(
            &tokens,
            &[
                DataToken::ElementHeader(*element.header()),
                DataToken::PrimitiveValue("Doe^John".into()),
            ],
        )
    }

    #[test]
    fn sequence_implicit_len_into_tokens() {
        let element = DataElement::new(
            Tag(0x0008, 0x2218),
            VR::SQ,
            DicomValue::new_sequence(
                vec![SimpleObject(
                    Length::UNDEFINED,
                    smallvec![
                        DataElement::new(
                            Tag(0x0008, 0x0100),
                            VR::SH,
                            DicomValue::new(dicom_value!(Strs, ["T-D1213 "])),
                        ),
                        DataElement::new(
                            Tag(0x0008, 0x0102),
                            VR::SH,
                            DicomValue::new(dicom_value!(Strs, ["SRT "])),
                        ),
                        DataElement::new(
                            Tag(0x0008, 0x0104),
                            VR::LO,
                            DicomValue::new(dicom_value!(Strs, ["Jaw region"])),
                        ),
                    ],
                )],
                Length::UNDEFINED,
            ),
        );

        let tokens: Vec<_> = element.clone().into_tokens().collect();

        assert_eq!(
            &tokens,
            &[
                DataToken::SequenceStart {
                    tag: Tag(0x0008, 0x2218),
                    len: Length::UNDEFINED,
                },
                DataToken::ItemStart {
                    len: Length::UNDEFINED
                },
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0100),
                    vr: VR::SH,
                    len: Length(8),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Strs(
                    ["T-D1213 ".to_owned()].as_ref().into(),
                )),
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0102),
                    vr: VR::SH,
                    len: Length(4),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Strs(
                    ["SRT ".to_owned()].as_ref().into()
                )),
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0104),
                    vr: VR::LO,
                    len: Length(10),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Strs(
                    ["Jaw region".to_owned()].as_ref().into(),
                )),
                DataToken::ItemEnd,
                DataToken::SequenceEnd,
            ],
        )
    }

    #[test]
    fn sequence_explicit_len_into_tokens() {
        let element = DataElement::new(
            Tag(0x0008, 0x2218),
            VR::SQ,
            DicomValue::new_sequence(
                vec![SimpleObject(
                    Length(46),
                    smallvec![
                        DataElement::new(
                            Tag(0x0008, 0x0100),
                            VR::SH,
                            DicomValue::new(dicom_value!(Strs, ["T-D1213 "])),
                        ),
                        DataElement::new(
                            Tag(0x0008, 0x0102),
                            VR::SH,
                            DicomValue::new(dicom_value!(Strs, ["SRT "])),
                        ),
                        DataElement::new(
                            Tag(0x0008, 0x0104),
                            VR::LO,
                            DicomValue::new(dicom_value!(Strs, ["Jaw region"])),
                        ),
                    ],
                )],
                Length(54),
            ),
        );

        let tokens: Vec<_> = element.clone().into_tokens().collect();

        assert_eq!(
            &tokens,
            &[
                DataToken::SequenceStart {
                    tag: Tag(0x0008, 0x2218),
                    len: Length(54),
                },
                DataToken::ItemStart { len: Length(46) },
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0100),
                    vr: VR::SH,
                    len: Length(8),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Strs(
                    ["T-D1213 ".to_owned()].as_ref().into(),
                )),
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0102),
                    vr: VR::SH,
                    len: Length(4),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Strs(
                    ["SRT ".to_owned()].as_ref().into()
                )),
                DataToken::ElementHeader(DataElementHeader {
                    tag: Tag(0x0008, 0x0104),
                    vr: VR::LO,
                    len: Length(10),
                }),
                DataToken::PrimitiveValue(PrimitiveValue::Strs(
                    ["Jaw region".to_owned()].as_ref().into(),
                )),
                DataToken::ItemEnd,
                DataToken::SequenceEnd,
            ],
        )
    }

    #[test]
    fn lazy_dataset_token_value() {
        let data = b"1.234\0";
        let mut data = &data[..];
        let decoder = StatefulDecoder::new(
            &mut data,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::Default,
        );

        is_stateful_decode(&decoder);

        let token = LazyDataToken::LazyValue {
            header: DataElementHeader {
                tag: Tag(0x0020, 0x000D),
                vr: VR::UI,
                len: Length(6),
            },
            decoder,
        };

        match token.into_owned().unwrap() {
            DataToken::PrimitiveValue(v) => {
                assert_eq!(v.to_raw_str(), "1.234\0",);
            }
            t => panic!("Unexpected type of token {:?}", t),
        }
    }

    #[test]
    fn lazy_dataset_token_value_as_mut() {
        let data = b"1.234\0";
        let mut data = &data[..];
        let mut decoder = StatefulDecoder::new(
            &mut data,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::Default,
        );

        is_stateful_decode(&decoder);

        let token = LazyDataToken::LazyValue {
            header: DataElementHeader {
                tag: Tag(0x0020, 0x000D),
                vr: VR::UI,
                len: Length(6),
            },
            decoder: &mut decoder,
        };

        match token.into_owned().unwrap() {
            DataToken::PrimitiveValue(v) => {
                assert_eq!(v.to_raw_str(), "1.234\0",);
            }
            t => panic!("Unexpected type of token {:?}", t),
        }
        assert_eq!(decoder.position(), 6);
    }

    #[test]
    fn lazy_dataset_token_value_skip() {
        let data = b"1.234\0";
        let mut data = &data[..];
        let mut decoder = StatefulDecoder::new(
            &mut data,
            ExplicitVRLittleEndianDecoder::default(),
            LittleEndianBasicDecoder,
            SpecificCharacterSet::Default,
        );

        is_stateful_decode(&decoder);

        let token = LazyDataToken::LazyValue {
            header: DataElementHeader {
                tag: Tag(0x0020, 0x000D),
                vr: VR::UI,
                len: Length(6),
            },
            decoder: &mut decoder,
        };

        token.skip().unwrap();

        assert_eq!(decoder.position(), 6);
    }
}
