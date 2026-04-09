//! Adaptive VR Little Endian syntax transfer implementation.
//!
//! This decoder handles non-conformant DICOM files that declare
//! Explicit VR Little Endian in their transfer syntax but actually
//! encode the dataset in Implicit VR. On the first non-meta,
//! non-delimiter element, it probes the bytes after the tag to
//! determine whether they form a valid VR code. If they do,
//! it locks to explicit VR for the rest of the file; if not,
//! it switches to implicit VR.

use crate::decode::basic::LittleEndianBasicDecoder;
use crate::decode::{
    BadSequenceHeaderSnafu, BasicDecode, Decode, DecodeFrom, ReadHeaderTagSnafu,
    ReadItemHeaderSnafu, ReadItemLengthSnafu, ReadLengthSnafu, ReadReservedSnafu, ReadTagSnafu,
    ReadVrSnafu, Result,
};
use byteordered::byteorder::{ByteOrder, LittleEndian};
use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry, VirtualVr};
use dicom_core::header::{DataElementHeader, Length, SequenceItemHeader};
use dicom_core::{Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use snafu::ResultExt;
use std::cell::Cell;
use std::fmt;
use std::io::Read;

/// An AdaptiveVRLittleEndianDecoder which uses the standard data dictionary.
pub type StandardAdaptiveVRLittleEndianDecoder =
    AdaptiveVRLittleEndianDecoder<StandardDataDictionary>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum VrState {
    /// Not yet determined — will probe on next non-delimiter element.
    Unknown,
    /// Locked to explicit VR.
    Explicit,
    /// Locked to implicit VR.
    Implicit,
}

/// A data element decoder for Little Endian data that auto-detects
/// whether the dataset uses explicit or implicit VR encoding.
///
/// This is intended for non-conformant files that declare Explicit VR LE
/// in the transfer syntax but actually contain Implicit VR data.
/// On the first non-meta element, the decoder probes the two bytes
/// following the tag: if they form a recognized VR code, it proceeds
/// as explicit VR; otherwise it falls back to implicit VR.
pub struct AdaptiveVRLittleEndianDecoder<D> {
    dict: D,
    basic: LittleEndianBasicDecoder,
    state: Cell<VrState>,
}

impl<D> fmt::Debug for AdaptiveVRLittleEndianDecoder<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AdaptiveVRLittleEndianDecoder")
            .field("dict", &"«omitted»")
            .field("basic", &self.basic)
            .field("state", &self.state)
            .finish()
    }
}

impl AdaptiveVRLittleEndianDecoder<StandardDataDictionary> {
    /// Retrieve this decoder using the standard data dictionary.
    pub fn with_std_dict() -> Self {
        AdaptiveVRLittleEndianDecoder {
            dict: StandardDataDictionary,
            basic: LittleEndianBasicDecoder,
            state: Cell::new(VrState::Unknown),
        }
    }

    /// Retrieve this decoder using the standard data dictionary.
    pub fn new() -> Self {
        Self::with_std_dict()
    }
}

impl Default for AdaptiveVRLittleEndianDecoder<StandardDataDictionary> {
    fn default() -> Self {
        Self::with_std_dict()
    }
}

impl<D> AdaptiveVRLittleEndianDecoder<D>
where
    D: DataDictionary,
{
    /// Retrieve this decoder using a custom data dictionary.
    pub fn with_dict(dictionary: D) -> Self {
        AdaptiveVRLittleEndianDecoder {
            dict: dictionary,
            basic: LittleEndianBasicDecoder,
            state: Cell::new(VrState::Unknown),
        }
    }

    /// Resolve VR for implicit mode using the data dictionary.
    fn resolve_vr(&self, tag: Tag) -> VR {
        if tag == Tag(0x7FE0, 0x0010) || (tag.0 >> 8 == 0x60 && tag.1 == 0x3000) {
            VR::OW
        } else {
            self.dict
                .by_tag(tag)
                .map(|entry| entry.vr().relaxed())
                .unwrap_or(VR::UN)
        }
    }
}

/// Check whether a probed VR is compatible with a dictionary VirtualVr.
/// VirtualVr variants like Xs and Ox allow multiple concrete VRs.
fn vr_compatible_with_virtual(probed: VR, dict_vr: VirtualVr) -> bool {
    match dict_vr {
        VirtualVr::Exact(vr) => probed == vr,
        VirtualVr::Xs => matches!(probed, VR::US | VR::SS),
        VirtualVr::Ox => matches!(probed, VR::OB | VR::OW),
        VirtualVr::Px => matches!(probed, VR::OB | VR::OW),
        VirtualVr::Lt => matches!(probed, VR::US | VR::OW),
        // unknown variants: treat as incompatible to avoid
        // false-positive explicit detection
        _ => false,
    }
}

impl<D> Decode for AdaptiveVRLittleEndianDecoder<D>
where
    D: DataDictionary,
{
    fn decode_header<S>(&self, mut source: &mut S) -> Result<(DataElementHeader, usize)>
    where
        S: ?Sized + Read,
    {
        // retrieve tag
        let Tag(group, element) = self
            .basic
            .decode_tag(&mut source)
            .context(ReadHeaderTagSnafu)?;

        let mut buf = [0u8; 4];

        // item delimiters never have VR or reserved fields
        if group == 0xFFFE {
            source.read_exact(&mut buf).context(ReadItemLengthSnafu)?;
            let len = LittleEndian::read_u32(&buf);
            return Ok((
                DataElementHeader::new((group, element), VR::UN, Length(len)),
                8,
            ));
        }

        let tag = Tag(group, element);
        let state = self.state.get();

        match state {
            VrState::Explicit => decode_explicit_header(&mut source, tag, &mut buf),
            VrState::Implicit => {
                let (vr, len, bytes) = decode_implicit_length(&mut source, tag, self, &mut buf)?;
                Ok((DataElementHeader::new(tag, vr, Length(len)), bytes))
            }
            VrState::Unknown => {
                // Probe: read the 2 bytes after the tag.
                // If they form a valid VR, we're in explicit mode.
                // Otherwise, they're the first half of a 4-byte length.
                source.read_exact(&mut buf[0..2]).context(ReadVrSnafu)?;

                if let Some(vr) = VR::from_binary([buf[0], buf[1]]) {
                    // Cross-check against the data dictionary:
                    // if the dictionary knows this tag and the probed VR
                    // is incompatible, the bytes are actually
                    // the start of a 4-byte implicit length.
                    let dict_vvr = self.dict.by_tag(tag).map(|entry| entry.vr());
                    if dict_vvr.is_some_and(|vvr| !vr_compatible_with_virtual(vr, vvr)) {
                        self.state.set(VrState::Implicit);
                        source.read_exact(&mut buf[2..4]).context(ReadLengthSnafu)?;
                        let len = LittleEndian::read_u32(&buf);
                        let resolved = self.resolve_vr(tag);
                        return Ok((DataElementHeader::new(tag, resolved, Length(len)), 8));
                    }

                    self.state.set(VrState::Explicit);
                    let (len, bytes_read) = decode_explicit_length(&mut source, vr, &mut buf)?;
                    Ok((DataElementHeader::new(tag, vr, Length(len)), bytes_read))
                } else {
                    self.state.set(VrState::Implicit);
                    // The 2 bytes we read are the low half of the 4-byte length.
                    // Read the remaining 2 bytes.
                    source.read_exact(&mut buf[2..4]).context(ReadLengthSnafu)?;
                    let len = LittleEndian::read_u32(&buf);
                    let vr = self.resolve_vr(tag);
                    Ok((DataElementHeader::new(tag, vr, Length(len)), 8))
                }
            }
        }
    }

    fn decode_item_header<S>(&self, source: &mut S) -> Result<SequenceItemHeader>
    where
        S: ?Sized + Read,
    {
        // item headers are the same regardless of VR mode
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf).context(ReadItemHeaderSnafu)?;
        let group = LittleEndian::read_u16(&buf[0..2]);
        let element = LittleEndian::read_u16(&buf[2..4]);
        let len = LittleEndian::read_u32(&buf[4..8]);
        SequenceItemHeader::new((group, element), Length(len)).context(BadSequenceHeaderSnafu)
    }

    #[inline]
    fn decode_tag<S>(&self, source: &mut S) -> Result<Tag>
    where
        S: ?Sized + Read,
    {
        self.basic.decode_tag(source).context(ReadTagSnafu)
    }
}

/// Decode an explicit VR element header after the tag has been read.
fn decode_explicit_header<S>(
    source: &mut S,
    tag: Tag,
    buf: &mut [u8; 4],
) -> Result<(DataElementHeader, usize)>
where
    S: ?Sized + Read,
{
    source.read_exact(&mut buf[0..2]).context(ReadVrSnafu)?;
    let vr = VR::from_binary([buf[0], buf[1]]).unwrap_or(VR::UN);
    let (len, bytes_read) = decode_explicit_length(source, vr, buf)?;
    Ok((DataElementHeader::new(tag, vr, Length(len)), bytes_read))
}

/// Read the length field for an explicit VR element.
/// Returns (length, total_bytes_read_including_tag_and_vr).
fn decode_explicit_length<S>(source: &mut S, vr: VR, buf: &mut [u8; 4]) -> Result<(u32, usize)>
where
    S: ?Sized + Read,
{
    match vr {
        VR::AE
        | VR::AS
        | VR::AT
        | VR::CS
        | VR::DA
        | VR::DS
        | VR::DT
        | VR::FL
        | VR::FD
        | VR::IS
        | VR::LO
        | VR::LT
        | VR::PN
        | VR::SH
        | VR::SL
        | VR::SS
        | VR::ST
        | VR::TM
        | VR::UI
        | VR::UL
        | VR::US => {
            source.read_exact(&mut buf[0..2]).context(ReadLengthSnafu)?;
            Ok((u32::from(LittleEndian::read_u16(&buf[0..2])), 8))
        }
        _ => {
            source
                .read_exact(&mut buf[0..2])
                .context(ReadReservedSnafu)?;
            source.read_exact(buf).context(ReadLengthSnafu)?;
            Ok((LittleEndian::read_u32(buf), 12))
        }
    }
}

/// Read the length field for an implicit VR element (4 bytes)
/// and resolve VR from the dictionary.
fn decode_implicit_length<S, D>(
    source: &mut S,
    tag: Tag,
    dec: &AdaptiveVRLittleEndianDecoder<D>,
    buf: &mut [u8; 4],
) -> Result<(VR, u32, usize)>
where
    S: ?Sized + Read,
    D: DataDictionary,
{
    source.read_exact(buf).context(ReadLengthSnafu)?;
    let len = LittleEndian::read_u32(buf);
    let vr = dec.resolve_vr(tag);
    Ok((vr, len, 8))
}

impl<S: ?Sized, D> DecodeFrom<S> for AdaptiveVRLittleEndianDecoder<D>
where
    S: Read,
    D: DataDictionary,
{
    #[inline]
    fn decode_header(&self, source: &mut S) -> Result<(DataElementHeader, usize)> {
        Decode::decode_header(self, source)
    }

    #[inline]
    fn decode_item_header(&self, source: &mut S) -> Result<SequenceItemHeader> {
        Decode::decode_item_header(self, source)
    }

    #[inline]
    fn decode_tag(&self, source: &mut S) -> Result<Tag> {
        Decode::decode_tag(self, source)
    }
}

#[cfg(test)]
mod tests {
    use super::AdaptiveVRLittleEndianDecoder;
    use crate::decode::Decode;
    use dicom_core::dictionary::stub::StubDataDictionary;
    use dicom_core::header::{HasLength, Header, Length};
    use dicom_core::{Tag, VR};
    use std::io::{Cursor, Read, Seek, SeekFrom};

    // Explicit VR data: same structure as the explicit_le tests.
    //   Tag: (0002,0002) Media Storage SOP Class UID
    //   VR: UI, Length: 26
    //   Value: "1.2.840.10008.5.1.4.1.1.1\0"
    // --
    //   Tag: (0002,0010) Transfer Syntax UID
    //   VR: UI, Length: 20
    //   Value: "1.2.840.10008.1.2.1\0"
    #[rustfmt::skip]
    const RAW_EXPLICIT: &[u8] = &[
        0x02, 0x00, 0x02, 0x00,     // tag (0002,0002)
            b'U', b'I',             // VR
            0x1A, 0x00,             // length: 26
            0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
            0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e,
            0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e, 0x31, 0x2e,
            0x31, 0x00,
        0x02, 0x00, 0x10, 0x00,     // tag (0002,0010)
            b'U', b'I',             // VR
            0x14, 0x00,             // length: 20
            0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
            0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e,
            0x32, 0x2e, 0x31, 0x00,
    ];

    #[test]
    fn adaptive_reads_explicit_vr() {
        let reader = AdaptiveVRLittleEndianDecoder::with_std_dict();
        let mut cursor = Cursor::new(RAW_EXPLICIT.as_ref());
        {
            let (elem, bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0002));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(26));
            assert_eq!(bytes_read, 8);
            cursor.seek(SeekFrom::Current(26)).unwrap();
        }
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0010));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(20));
        }
    }

    // Implicit VR data: tag + 4-byte length, no VR bytes.
    //   Tag: (0002,0002) Media Storage SOP Class UID
    //   Length: 26
    //   Value: "1.2.840.10008.5.1.4.1.1.1\0"
    // --
    //   Tag: (0002,0010) Transfer Syntax UID
    //   Length: 20
    //   Value: "1.2.840.10008.1.2.1\0"
    #[rustfmt::skip]
    const RAW_IMPLICIT: &[u8] = &[
        0x02, 0x00, 0x02, 0x00,     // tag (0002,0002)
            0x1A, 0x00, 0x00, 0x00, // length: 26
            0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
            0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e,
            0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e, 0x31, 0x2e,
            0x31, 0x00,
        0x02, 0x00, 0x10, 0x00,     // tag (0002,0010)
            0x14, 0x00, 0x00, 0x00, // length: 20
            0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30, 0x2e,
            0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e,
            0x32, 0x2e, 0x31, 0x00,
    ];

    const DICT: &StubDataDictionary = &StubDataDictionary;

    #[test]
    fn adaptive_reads_implicit_vr() {
        let reader = AdaptiveVRLittleEndianDecoder::with_dict(DICT);
        let mut cursor = Cursor::new(RAW_IMPLICIT.as_ref());
        {
            let (elem, bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0002));
            // StubDataDictionary returns UN for unknown tags
            assert_eq!(elem.vr(), VR::UN);
            assert_eq!(elem.length(), Length(26));
            assert_eq!(bytes_read, 8);
            cursor.seek(SeekFrom::Current(26)).unwrap();
        }
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0010));
            assert_eq!(elem.vr(), VR::UN);
            assert_eq!(elem.length(), Length(20));
        }
    }

    #[test]
    fn adaptive_reads_implicit_with_standard_dict() {
        let reader = AdaptiveVRLittleEndianDecoder::with_std_dict();
        let mut cursor = Cursor::new(RAW_IMPLICIT.as_ref());
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0002));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(26));
            cursor.seek(SeekFrom::Current(26)).unwrap();
        }
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0002, 0x0010));
            assert_eq!(elem.vr(), VR::UI);
            assert_eq!(elem.length(), Length(20));
        }
    }

    // Sequence/item delimiters — should work regardless of VR state.
    //  Tag: (FFFE,E000) Item, Length: 0xFFFFFFFF
    //  Tag: (FFFE,E00D) Item Delimitation, Length: 0
    //  Tag: (FFFE,E0DD) Sequence Delimitation, Length: 0
    #[rustfmt::skip]
    const RAW_DELIMITERS: &[u8] = &[
        0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFE, 0xFF, 0x0D, 0xE0, 0x00, 0x00, 0x00, 0x00,
        0xFE, 0xFF, 0xDD, 0xE0, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn adaptive_reads_delimiters() {
        let reader = AdaptiveVRLittleEndianDecoder::with_std_dict();
        let mut cursor = Cursor::new(RAW_DELIMITERS.as_ref());
        {
            let (elem, bytes_read) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE000));
            assert_eq!(elem.vr(), VR::UN);
            assert!(elem.length().is_undefined());
            assert_eq!(bytes_read, 8);
        }
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE00D));
            assert_eq!(elem.length(), Length(0));
        }
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE0DD));
            assert_eq!(elem.length(), Length(0));
        }
    }

    // Mixed: explicit VR element followed by a sequence delimiter.
    // Verifies the state is locked after the first probe
    // and delimiters still work.
    #[rustfmt::skip]
    const RAW_EXPLICIT_THEN_DELIMITER: &[u8] = &[
        // (0008,0060) Modality, VR: CS, Length: 2, Value: "CT"
        0x08, 0x00, 0x60, 0x00,
            b'C', b'S',
            0x02, 0x00,
            b'C', b'T',
        // (FFFE,E0DD) Sequence Delimitation, Length: 0
        0xFE, 0xFF, 0xDD, 0xE0, 0x00, 0x00, 0x00, 0x00,
    ];

    // Implicit VR data where the length field's low bytes happen to
    // match a valid VR code ("UN" = 0x55 0x4E, length = 20053).
    // The dictionary cross-check should catch the mismatch
    // (Modality is CS, not UN) and correctly treat as implicit.
    #[rustfmt::skip]
    const RAW_IMPLICIT_VR_COLLISION: &[u8] = &[
        // (0008,0060) Modality — implicit VR, length 20053 (0x00004E55)
        // Low bytes of length: 0x55 0x4E = "UN"
        0x08, 0x00, 0x60, 0x00,
            0x55, 0x4E, 0x00, 0x00,
            // (value bytes omitted — we only need to parse the header)
    ];

    #[test]
    fn adaptive_rejects_false_positive_vr() {
        let reader = AdaptiveVRLittleEndianDecoder::with_std_dict();
        let mut cursor = Cursor::new(RAW_IMPLICIT_VR_COLLISION.as_ref());
        let (elem, bytes_read) = reader
            .decode_header(&mut cursor)
            .expect("should find an element");
        assert_eq!(elem.tag(), Tag(0x0008, 0x0060));
        // Dictionary says Modality is CS, probed bytes say UN:
        // dictionary wins, implicit VR is used, CS resolved from dict
        assert_eq!(elem.vr(), VR::CS);
        assert_eq!(elem.length(), Length(20053));
        assert_eq!(bytes_read, 8);
    }

    #[test]
    fn adaptive_explicit_then_delimiter() {
        let reader = AdaptiveVRLittleEndianDecoder::with_std_dict();
        let mut cursor = Cursor::new(RAW_EXPLICIT_THEN_DELIMITER.as_ref());
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find an element");
            assert_eq!(elem.tag(), Tag(0x0008, 0x0060));
            assert_eq!(elem.vr(), VR::CS);
            assert_eq!(elem.length(), Length(2));
            let mut val = vec![0u8; 2];
            cursor.read_exact(&mut val).unwrap();
            assert_eq!(&val, b"CT");
        }
        {
            let (elem, _) = reader
                .decode_header(&mut cursor)
                .expect("should find delimiter");
            assert_eq!(elem.tag(), Tag(0xFFFE, 0xE0DD));
            assert_eq!(elem.length(), Length(0));
        }
    }
}
