use std::collections::BTreeMap;

use super::{PositionToValue as PositionToValueSnafu, ReadValue as ReadValueSnafu};
use dicom_core::{header::HasLength, DataElementHeader, DicomValue, Length, Tag};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_parser::StatefulDecode;
use snafu::ResultExt;

use crate::{InMemDicomObject, mem::InMemFragment, util::ReadSeek};

type Result<T, E = super::Error> = std::result::Result<T, E>;

/// A lazy element, which may be loaded in memory or not.
#[derive(Debug, Clone)]
pub struct LazyElement<D = StandardDataDictionary> {
    header: DataElementHeader,
    position: u64,
    value: MaybeValue<D>,
}

impl<D> LazyElement<D> {
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
    pub fn load<S>(&mut self, mut source: S) -> Result<()>
    where
        S: StatefulDecode,
        <S as StatefulDecode>::Reader: ReadSeek,
    {
        match self.value {
            MaybeValue::Loaded { .. } => Ok(()),
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
}

/// A DICOM value which may be loaded in memory or not.
///
/// Loading the value can only be done through the respective [`LazyElement`].
///
#[derive(Debug, Clone)]
pub enum MaybeValue<D = StandardDataDictionary> {
    Loaded { value: LoadedValue<D>, dirty: bool },
    Unloaded,
}

impl<D> MaybeValue<D> {
    /// Return a reference to the loaded value,
    /// or `None` if the value is not loaded.
    pub fn value(&self) -> Option<&LoadedValue<D>> {
        match self {
            MaybeValue::Loaded { value, .. } => Some(value),
            MaybeValue::Unloaded => None,
        }
    }

    pub fn is_loaded(&self) -> bool {
        match self {
            MaybeValue::Loaded { .. } => true,
            MaybeValue::Unloaded => false,
        }
    }
}

pub type LoadedValue<D> = DicomValue<LazyNestedObject<D>, InMemFragment>;

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

impl<D> LazyNestedObject<D> {
    /// Load each element in the object.
    pub fn load<S>(&mut self, mut source: S) -> Result<()>
    where
        S: StatefulDecode,
        <S as StatefulDecode>::Reader: ReadSeek,
    {
        for (_tag, elem) in &mut self.entries {
            elem.load(&mut source)?;
        }
        Ok(())
    }

    /// Load each element in the object and turn it into an.
    pub fn into_mem<S>(self, mut source: S) -> Result<InMemDicomObject<D>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use byteordered::Endianness;
    use dicom_core::DataElementHeader;
    use dicom_core::DicomValue;
    use dicom_core::Length;
    use dicom_core::PrimitiveValue;
    use dicom_core::Tag;
    use dicom_core::VR;
    use dicom_dictionary_std::StandardDataDictionary;
    use dicom_encoding::decode::basic::BasicDecoder;
    use dicom_encoding::decode::explicit_le::ExplicitVRLittleEndianDecoder;
    use dicom_encoding::decode::implicit_le::ImplicitVRLittleEndianDecoder;
    use dicom_encoding::text::DefaultCharacterSetCodec;
    use dicom_parser::StatefulDecode;
    use dicom_parser::StatefulDecoder;

    use crate::InMemDicomObject;
    use crate::mem::InMemElement;

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
            MaybeValue::Loaded { value, dirty } => {
                assert_eq!(value.to_clean_str().unwrap(), "Doe^John");
                assert_eq!(dirty, false);
            }
        }
    }
}
