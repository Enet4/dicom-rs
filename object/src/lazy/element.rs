use std::collections::BTreeMap;

use super::{PositionToValue as PositionToValueSnafu, ReadValue as ReadValueSnafu, ReadFragment as ReadFragmentSnafu, UnloadedFragment as UnloadedFragmentSnafu};
use dicom_core::{DataDictionary, DataElementHeader, DicomValue, Length, Tag, header::HasLength};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_parser::StatefulDecode;
use snafu::{OptionExt, ResultExt};
use smallvec::SmallVec;

use crate::{
    mem::{InMemElement, InMemFragment},
    util::ReadSeek,
    InMemDicomObject,
};

type Result<T, E = super::Error> = std::result::Result<T, E>;

/// A lazy element, which may be loaded in memory or not.
#[derive(Debug, Clone)]
pub struct LazyElement<D = StandardDataDictionary> {
    header: DataElementHeader,
    position: u64,
    value: MaybeValue<D>,
}

impl<D> LazyElement<D>
where
    D: DataDictionary,
    D: Clone,
{
    /// Create a new lazy element with the given properties,
    /// without loading its value in memory.
    pub fn new_unloaded(header: DataElementHeader, position: u64) -> Self {
        LazyElement {
            header,
            position,
            value: MaybeValue::Unloaded,
        }
    }

    /// Create a new lazy element with the given properties,
    /// already loaded with an in-memory value.
    pub fn new_loaded(header: DataElementHeader, position: u64, value: LoadedValue<D>) -> Self {
        LazyElement {
            header,
            position,
            value: MaybeValue::Loaded {
                value,
                dirty: false,
            },
        }
    }

    /// Ensure that the value is loaded in memory,
    /// fetching it from the given source if necessary.
    ///
    /// The operation is a no-op if the value is already loaded.
    pub fn load<S: ?Sized>(&mut self, source: &mut S) -> Result<()>
    where
        S: StatefulDecode,
        <S as StatefulDecode>::Reader: ReadSeek,
    {
        match &mut self.value {
            MaybeValue::Loaded { .. } => Ok(()),
            MaybeValue::PixelSequence { fragments, .. } => {
                // load each fragment individually
                for fragment in fragments {
                    if fragment.data.is_some() {
                        continue;
                    }
                    source.seek(fragment.position).context(PositionToValueSnafu)?;
                    let mut data = Vec::with_capacity(fragment.length as usize);
                    source.read_to_vec(fragment.length, &mut data).context(ReadFragmentSnafu)?;
                    fragment.data = Some(data);
                }
                Ok(())
            },
            MaybeValue::Unloaded => {
                source.seek(self.position).context(PositionToValueSnafu)?;
                let value = source
                    .read_value_preserved(&self.header)
                    .context(ReadValueSnafu)?;
                self.value = MaybeValue::Loaded {
                    value: DicomValue::from(value),
                    dirty: false,
                };
                Ok(())
            }
        }
    }

    /// Convert the lazy element into an in-memory element,
    /// loading it from the given source if necessary.
    pub fn into_mem<S: ?Sized>(mut self, source: &mut S) -> Result<InMemElement<D>>
    where
        S: StatefulDecode,
        <S as StatefulDecode>::Reader: ReadSeek,
    {
        self.load(source)?;

        let value = self.value.into_mem(source)?;

        Ok(InMemElement::new(self.header.tag, self.header.vr, value))
    }
}

/// A DICOM value which may be loaded in memory or not.
///
/// Loading the value can only be done through the respective [`LazyElement`].
///
#[derive(Debug, Clone)]
pub enum MaybeValue<D = StandardDataDictionary> {
    /// A DICOM value that is at least partially loaded in memory.
    ///
    ///
    /// Its nested DICOM data sets or fragments might not be all loaded
    /// in the case of sequences.
    Loaded {
        /// the value proper
        value: LoadedValue<D>,
        dirty: bool,
    },
    /// a DICOM value that is a pixel sequence,
    /// where each fragment can be loaded independently
    PixelSequence {
        /// the offset table for each pixel data frame
        offset_table: SmallVec<[u32; 2]>,
        /// the sequence of fragments
        fragments: SmallVec<[MaybeFragment; 2]>,
    },
    /// a DICOM value which is not loaded,
    /// and so is unreachable from here
    Unloaded,
}

impl<D> MaybeValue<D>
where
    D: DataDictionary,
    D: Clone,
{
    /// Return a reference to the loaded value,
    /// or `None` if the value is not loaded.
    pub fn value(&self) -> Option<&LoadedValue<D>> {
        match self {
            MaybeValue::Loaded { value, .. } => Some(value),
            MaybeValue::PixelSequence { fragments, .. } => todo!("retrieving pixel sequences"),
            MaybeValue::Unloaded => None,
        }
    }

    /// Check whether the element is loaded at this level.
    ///
    /// **Note:**
    /// this method does not check
    /// whether nested data sets or any pixel data fragments
    /// are fully loaded.
    pub fn is_loaded(&self) -> bool {
        match self {
            MaybeValue::Loaded { .. } => true,
            MaybeValue::PixelSequence { .. } => true,
            MaybeValue::Unloaded => false,
        }
    }

    /// **Pre-condition:** the value must be fully loaded.
    fn into_mem<S: ?Sized>(self, source: &mut S) -> Result<DicomValue<InMemDicomObject<D>, InMemFragment>>
    where
        S: StatefulDecode,
        <S as StatefulDecode>::Reader: ReadSeek,
    {
        match self {
            MaybeValue::Loaded { value, .. } => {
                match value {
                    DicomValue::Primitive(primitive) => {
                        // accept primitive value as is
                        Ok(DicomValue::from(primitive))
                    }
                    DicomValue::PixelSequence {
                        offset_table,
                        fragments,
                    } => {
                        let fragments: Result<SmallVec<_>> = fragments.into_iter()
                            .enumerate()
                            .map(|(i, f)| f.data.context(UnloadedFragmentSnafu { index: i as u32 }))
                            .collect();
                        // accept pixel sequence as is
                        Ok(DicomValue::PixelSequence {
                            offset_table,
                            fragments: fragments?,
                        })
                    }
                    DicomValue::Sequence { items, size } => {
                        // recursively turn each item into memory
                        let items: Result<_> = items
                            .into_iter()
                            .map(|item| item.into_mem(source))
                            .collect();
                        let items = items?;
                        Ok(DicomValue::Sequence { items, size })
                    }
                }
            }
            _ => panic!("Value should be loaded"),
        }
    }
}

/// A fragment of a pixel sequence,
/// which may be loaded in memory or not.
#[derive(Debug, Clone)]
pub struct MaybeFragment {
    /// The offset of the fragment data relative to the original source
    position: u64,
    /// The number of data bytes in this fragment
    length: u32,
    /// The actual data proper,
    /// which might not be loaded.
    data: Option<Vec<u8>>,
}

/// Type definition for a value which has been loaded into memory,
/// at least partially,
/// at one level.
///
/// If it is a primitive value,
/// then is sure to be all in memory.
/// In the case of a sequence,
/// the nested objects may or may not be loaded.
/// In the case of a pixel sequence,
/// each fragments may be loaded in memory or not.
pub type LoadedValue<D> = DicomValue<LazyNestedObject<D>, MaybeFragment>;

/// A DICOM object nested in a lazy DICOM object.
///
/// The type parameter `S` represents the borrowed stateful reader,
/// implementing `StatefulDecode`.
/// `D` is for the element dictionary.
#[derive(Debug, Clone)]
pub struct LazyNestedObject<D = StandardDataDictionary> {
    /// the element dictionary
    entries: BTreeMap<Tag, LazyElement<D>>,
    /// the data attribute dictionary
    dict: D,
    /// The length of the DICOM object in bytes.
    /// It is usually undefined, unless it is part of an item
    /// in a sequence with a specified length in its item header.
    len: Length,
}

impl<D> HasLength for LazyNestedObject<D> {
    fn length(&self) -> Length {
        self.len
    }
}

impl<D> LazyNestedObject<D>
where
    D: DataDictionary,
    D: Clone,
{
    /// Load each element in the object.
    pub fn load_all<S: ?Sized>(&mut self, source: &mut S) -> Result<()>
    where
        S: StatefulDecode,
        <S as StatefulDecode>::Reader: ReadSeek,
    {
        for elem in &mut self.entries.values_mut() {
            elem.load(&mut *source)?;
        }
        Ok(())
    }

    /// Load each element in the object and turn it into an in-memory object.
    pub fn into_mem<S: ?Sized>(mut self, source: &mut S) -> Result<InMemDicomObject<D>>
    where
        S: StatefulDecode,
        <S as StatefulDecode>::Reader: ReadSeek,
        D: DataDictionary,
        D: Clone,
    {
        self.load_all(&mut *source)?;

        let entries: Result<_> = self.entries.into_values()
            .map(|elem| elem.into_mem(&mut *source).map(|elem| (elem.header().tag, elem)))
            .collect();
        
        Ok(InMemDicomObject::from_parts(entries?, self.dict, self.len))
    }
}

#[cfg(test)]
mod tests {
    use byteordered::Endianness;
    use dicom_core::DataElementHeader;
    use dicom_core::Length;
    use dicom_core::Tag;
    use dicom_core::VR;
    use dicom_core::dicom_value;
    use dicom_dictionary_std::StandardDataDictionary;
    use dicom_encoding::decode::basic::BasicDecoder;
    use dicom_encoding::decode::explicit_le::ExplicitVRLittleEndianDecoder;
    use dicom_encoding::decode::implicit_le::ImplicitVRLittleEndianDecoder;
    use dicom_encoding::text::DefaultCharacterSetCodec;
    use dicom_parser::StatefulDecode;
    use dicom_parser::StatefulDecoder;

    use crate::mem::InMemElement;
    use crate::InMemDicomObject;

    use super::LazyElement;
    use super::LazyNestedObject;
    use super::MaybeValue;

    #[test]
    fn lazy_element_single() {
        let data_in = [
            0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
            0x08, 0x00, 0x00, 0x00, // Length: 8
            b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
        ];

        // Create a stateful reader for the data
        let decoder = ImplicitVRLittleEndianDecoder::default();
        let text = Box::new(DefaultCharacterSetCodec) as Box<_>;
        let mut cursor = std::io::Cursor::new(data_in);
        let mut parser = StatefulDecoder::new(
            &mut cursor,
            decoder,
            BasicDecoder::new(Endianness::Little),
            text,
        );

        // Create an unloaded lazy element (actual value starts at 8)
        let mut lazy_element: LazyElement<StandardDataDictionary> = LazyElement {
            header: DataElementHeader::new(Tag(0x0010, 0x0010), VR::PN, Length(8)),
            position: 8,
            value: MaybeValue::Unloaded,
        };

        // Load the lazy element
        lazy_element
            .load(&mut parser)
            .expect("Failed to load lazy element");
        match lazy_element.value {
            MaybeValue::Unloaded => panic!("element should be loaded"),
            MaybeValue::PixelSequence { .. } => unreachable!("element is not a pixel sequence"),
            MaybeValue::Loaded { value, dirty } => {
                assert_eq!(value.to_clean_str().unwrap(), "Doe^John");
                assert_eq!(dirty, false);
            }
        }
    }

    #[test]
    fn lazy_element_somewhere_in_middle() {
        let data_in = [
            // 30 bytes of irrelevant data
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 10
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 20
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 30
            // actual element is here
            0x10, 0x00, 0x10, 0x00, // Tag(0x0010, 0x0010)
            0x08, 0x00, 0x00, 0x00, // Length: 8
            b'D', b'o', b'e', b'^', b'J', b'o', b'h', b'n',
            // 10 more bytes of irrelevant data (@ 46)
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 66
        ];

        // Create a stateful reader for the data
        let decoder = ImplicitVRLittleEndianDecoder::default();
        let text = Box::new(DefaultCharacterSetCodec) as Box<_>;
        let mut cursor = std::io::Cursor::new(data_in);
        let mut parser = StatefulDecoder::new(
            &mut cursor,
            decoder,
            BasicDecoder::new(Endianness::Little),
            text,
        );

        // move cursor to the end (simulating a full file read)
        parser.seek(66).expect("Failed to seek to end of file");

        // Create an unloaded lazy element
        let mut lazy_element: LazyElement<StandardDataDictionary> = LazyElement {
            header: DataElementHeader::new(Tag(0x0010, 0x0010), VR::PN, Length(8)),
            position: 38,
            value: MaybeValue::Unloaded,
        };

        // Load the lazy element
        lazy_element
            .load(&mut parser)
            .expect("Failed to load lazy element");
        match lazy_element.value {
            MaybeValue::Unloaded => panic!("element should be loaded"),
            MaybeValue::PixelSequence { .. } => unreachable!("element is not a pixel sequence"),
            MaybeValue::Loaded { value, dirty } => {
                assert_eq!(value.to_clean_str().unwrap(), "Doe^John");
                assert_eq!(dirty, false);
            }
        }
    }
    #[test]
    fn lazy_nested_object() {
        static DATA_IN: &[u8] = &[
            // SequenceStart: (0008,2218) ; len = 54 (#=3)
            0x08, 0x00, 0x18, 0x22, b'S', b'Q', 0x00, 0x00, 0x36, 0x00, 0x00, 0x00,
            // -- 12, --
            // ItemStart: len = 46
            0xfe, 0xff, 0x00, 0xe0, 0x2e, 0x00, 0x00, 0x00,
            // -- 20, --
            // ElementHeader: (0008,0100) CodeValue; len = 8
            0x08, 0x00, 0x00, 0x01, b'S', b'H', 0x08, 0x00, // PrimitiveValue
            b'T', b'-', b'D', b'1', b'2', b'1', b'3', b' ',
            // -- 36, --
            // ElementHeader: (0008,0102) CodingSchemeDesignator; len = 4
            0x08, 0x00, 0x02, 0x01, b'S', b'H', 0x04, 0x00, // PrimitiveValue
            b'S', b'R', b'T', b' ',
            // -- 48, --
            // (0008,0104) CodeMeaning; len = 10
            0x08, 0x00, 0x04, 0x01, b'L', b'O', 0x0a, 0x00, // PrimitiveValue
            b'J', b'a', b'w', b' ', b'r', b'e', b'g', b'i', b'o', b'n',
            // -- 66 --
            // SequenceStart: (0040,0555) AcquisitionContextSequence; len = 0
            0x40, 0x00, 0x55, 0x05, b'S', b'Q', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // ElementHeader: (2050,0020) PresentationLUTShape; len = 8
            0x50, 0x20, 0x20, 0x00, b'C', b'S', 0x08, 0x00, // PrimitiveValue
            b'I', b'D', b'E', b'N', b'T', b'I', b'T', b'Y',
        ];

        // Create a stateful reader for the data
        let decoder = ExplicitVRLittleEndianDecoder::default();
        let text = Box::new(DefaultCharacterSetCodec) as Box<_>;
        let mut cursor = std::io::Cursor::new(DATA_IN);
        let mut parser = StatefulDecoder::new(
            &mut cursor,
            decoder,
            BasicDecoder::new(Endianness::Little),
            text,
        );

        // move cursor to the end (simulating a full file read)
        parser.seek(94).expect("Failed to seek to end of file");

        // construct accurate nested object, unloaded
        let mut nested_object: LazyNestedObject<StandardDataDictionary> = LazyNestedObject {
            entries: vec![
                // CodeValue element
                (
                    Tag(0x0008, 0x0100),
                    LazyElement::new_unloaded(
                        DataElementHeader::new(Tag(0x0008, 0x0100), VR::SH, Length(8)),
                        28,
                    ),
                ),
                // CodingSchemeDesignator element
                (
                    Tag(0x0008, 0x0102),
                    LazyElement::new_unloaded(
                        DataElementHeader::new(Tag(0x0008, 0x0102), VR::SH, Length(4)),
                        44,
                    ),
                ),
                // CodeMeaning element
                (
                    Tag(0x0008, 0x0104),
                    LazyElement::new_unloaded(
                        DataElementHeader::new(Tag(0x0008, 0x0104), VR::LO, Length(10)),
                        56,
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            dict: Default::default(),
            len: Length(46),
        };

        // load nested object
        nested_object
            .load_all(&mut parser)
            .expect("Failed to load nested object");

        for e in nested_object.entries.values() {
            assert!(e.value.is_loaded());
        }

        // turn it into an in-memory DICOM object,
        // test with ground truth
        let inmem = nested_object
            .into_mem(&mut parser)
            .expect("Failed to load all object into memory");

        let gt: InMemDicomObject = InMemDicomObject::from_element_iter(vec![
            InMemElement::new(
                Tag(0x0008, 0x0100),
                VR::SH,
                dicom_value!(Strs, ["T-D1213 "]),
            ),
            InMemElement::new(Tag(0x0008, 0x0102), VR::SH, dicom_value!(Strs, ["SRT "])),
            InMemElement::new(
                Tag(0x0008, 0x0104),
                VR::LO,
                dicom_value!(Strs, ["Jaw region"]),
            ),
        ]);

        assert_eq_elements(&inmem, &gt);
    }

    /// Assert that two objects are equal
    /// by traversing their elements in sequence
    /// and checking that those are equal.
    fn assert_eq_elements<D>(obj1: &InMemDicomObject<D>, obj2: &InMemDicomObject<D>)
    where
        D: std::fmt::Debug,
    {
        // iterate through all elements in both objects
        // and check that they are equal
        for (e1, e2) in std::iter::Iterator::zip(obj1.into_iter(), obj2) {
            assert_eq!(e1, e2);
        }
    }
}
