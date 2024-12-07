//! Module containing data structures and readers of DICOM file meta information tables.
use byteordered::byteorder::{ByteOrder, LittleEndian};
use dicom_core::dicom_value;
use dicom_core::header::{DataElement, EmptyObject, HasLength, Header};
use dicom_core::ops::{ApplyOp, AttributeAction, AttributeOp, AttributeSelectorStep};
use dicom_core::value::{
    ConvertValueError, DicomValueType, InMemFragment, PrimitiveValue, Value, ValueType,
};
use dicom_core::{Length, Tag, VR};
use dicom_dictionary_std::tags;
use dicom_encoding::decode::{self, DecodeFrom};
use dicom_encoding::encode::explicit_le::ExplicitVRLittleEndianEncoder;
use dicom_encoding::encode::EncoderFor;
use dicom_encoding::text::{self, TextCodec};
use dicom_encoding::TransferSyntax;
use dicom_parser::dataset::{DataSetWriter, IntoTokens};
use snafu::{ensure, Backtrace, OptionExt, ResultExt, Snafu};
use std::borrow::Cow;
use std::io::{Read, Write};

use crate::ops::{
    ApplyError, ApplyResult, IllegalExtendSnafu, IncompatibleTypesSnafu, MandatorySnafu,
    UnsupportedActionSnafu, UnsupportedAttributeSnafu,
};
use crate::{
    AttributeError, DicomAttribute, DicomObject, IMPLEMENTATION_CLASS_UID,
    IMPLEMENTATION_VERSION_NAME,
};

const DICM_MAGIC_CODE: [u8; 4] = [b'D', b'I', b'C', b'M'];

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    /// The file meta group parser could not read
    /// the magic code `DICM` from its source.
    #[snafu(display("Could not start reading DICOM data"))]
    ReadMagicCode {
        backtrace: Backtrace,
        source: std::io::Error,
    },

    /// The file meta group parser could not fetch
    /// the value of a data element from its source.
    #[snafu(display("Could not read data value"))]
    ReadValueData {
        backtrace: Backtrace,
        source: std::io::Error,
    },

    /// The parser could not allocate memory for the
    /// given length of a data element.
    #[snafu(display("Could not allocate memory"))]
    AllocationSize {
        backtrace: Backtrace,
        source: std::collections::TryReserveError,
    },

    /// The file meta group parser could not decode
    /// the text in one of its data elements.
    #[snafu(display("Could not decode text in {}", name))]
    DecodeText {
        name: std::borrow::Cow<'static, str>,
        #[snafu(backtrace)]
        source: dicom_encoding::text::DecodeTextError,
    },

    /// Invalid DICOM data, detected by checking the `DICM` code.
    #[snafu(display("Invalid DICOM file (magic code check failed)"))]
    NotDicom { backtrace: Backtrace },

    /// An issue occurred while decoding the next data element
    /// in the file meta data set.
    #[snafu(display("Could not decode data element"))]
    DecodeElement {
        #[snafu(backtrace)]
        source: dicom_encoding::decode::Error,
    },

    /// A data element with an unexpected tag was retrieved:
    /// the parser was expecting another tag first,
    /// or at least one that is part of the the file meta group.
    #[snafu(display("Unexpected data element tagged {}", tag))]
    UnexpectedTag { tag: Tag, backtrace: Backtrace },

    /// A required file meta data element is missing.
    #[snafu(display("Missing data element `{}`", alias))]
    MissingElement {
        alias: &'static str,
        backtrace: Backtrace,
    },

    /// The value length of a data elements in the file meta group
    /// was unexpected.
    #[snafu(display("Unexpected length {} for data element tagged {}", length, tag))]
    UnexpectedDataValueLength {
        tag: Tag,
        length: Length,
        backtrace: Backtrace,
    },

    /// The value length of a data element is undefined,
    /// but knowing the length is required in its context.
    #[snafu(display("Undefined value length for data element tagged {}", tag))]
    UndefinedValueLength { tag: Tag, backtrace: Backtrace },

    /// The file meta group data set could not be written.
    #[snafu(display("Could not write file meta group data set"))]
    WriteSet {
        #[snafu(backtrace)]
        source: dicom_parser::dataset::write::Error,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// DICOM File Meta Information Table.
///
/// This data type contains the relevant parts of the file meta information table,
/// as specified in [part 6, chapter 7][1] of the standard.
///
/// Creating a new file meta table from scratch
/// is more easily done using a [`FileMetaTableBuilder`].
/// When modifying the struct's public fields,
/// it is possible to update the information group length
/// through method [`update_information_group_length`][2].
///
/// [1]: http://dicom.nema.org/medical/dicom/current/output/chtml/part06/chapter_7.html
/// [2]: FileMetaTable::update_information_group_length
#[derive(Debug, Clone, PartialEq)]
pub struct FileMetaTable {
    /// File Meta Information Group Length
    pub information_group_length: u32,
    /// File Meta Information Version
    pub information_version: [u8; 2],
    /// Media Storage SOP Class UID
    pub media_storage_sop_class_uid: String,
    /// Media Storage SOP Instance UID
    pub media_storage_sop_instance_uid: String,
    /// Transfer Syntax UID
    pub transfer_syntax: String,
    /// Implementation Class UID
    pub implementation_class_uid: String,

    /// Implementation Version Name
    pub implementation_version_name: Option<String>,
    /// Source Application Entity Title
    pub source_application_entity_title: Option<String>,
    /// Sending Application Entity Title
    pub sending_application_entity_title: Option<String>,
    /// Receiving Application Entity Title
    pub receiving_application_entity_title: Option<String>,
    /// Private Information Creator UID
    pub private_information_creator_uid: Option<String>,
    /// Private Information
    pub private_information: Option<Vec<u8>>,
    /*
    Missing attributes:

    (0002,0026) Source Presentation Address Source​Presentation​Address UR 1
    (0002,0027) Sending Presentation Address Sending​Presentation​Address UR 1
    (0002,0028) Receiving Presentation Address Receiving​Presentation​Address UR 1
    (0002,0031) RTV Meta Information Version RTV​Meta​Information​Version OB 1
    (0002,0032) RTV Communication SOP Class UID RTV​Communication​SOP​Class​UID UI 1
    (0002,0033) RTV Communication SOP Instance UID RTV​Communication​SOP​Instance​UID UI 1
    (0002,0035) RTV Source Identifier RTV​Source​Identifier OB 1
    (0002,0036) RTV Flow Identifier RTV​Flow​Identifier OB 1
    (0002,0037) RTV Flow RTP Sampling Rate RTV​Flow​RTP​Sampling​Rate UL 1
    (0002,0038) RTV Flow Actual Frame Duration RTV​Flow​Actual​Frame​Duration FD 1
    */
}

/// Utility function for reading the body of the DICOM element as a UID.
fn read_str_body<'s, S, T>(source: &'s mut S, text: &T, len: u32) -> Result<String>
where
    S: Read + 's,
    T: TextCodec,
{
    let mut v = Vec::new();
    v.try_reserve_exact(len as usize)
        .context(AllocationSizeSnafu)?;
    v.resize(len as usize, 0);
    source.read_exact(&mut v).context(ReadValueDataSnafu)?;

    text.decode(&v)
        .context(DecodeTextSnafu { name: text.name() })
}

impl FileMetaTable {
    /// Construct a file meta group table
    /// by parsing a DICOM data set from a reader.
    /// 
    /// This method fails if the first four bytes
    /// are not the DICOM magic code `DICM`.
    pub fn from_reader<R: Read>(file: R) -> Result<Self> {
        FileMetaTable::read_from(file)
    }

    /// Getter for the transfer syntax UID,
    /// with trailing characters already excluded.
    pub fn transfer_syntax(&self) -> &str {
        self.transfer_syntax
            .trim_end_matches(|c: char| c.is_whitespace() || c == '\0')
    }

    /// Getter for the media storage SOP instance UID,
    /// with trailing characters already excluded.
    pub fn media_storage_sop_instance_uid(&self) -> &str {
        self.media_storage_sop_instance_uid
            .trim_end_matches(|c: char| c.is_whitespace() || c == '\0')
    }

    /// Getter for the media storage SOP class UID,
    /// with trailing characters already excluded.
    pub fn media_storage_sop_class_uid(&self) -> &str {
        self.media_storage_sop_class_uid
            .trim_end_matches(|c: char| c.is_whitespace() || c == '\0')
    }

    /// Getter for the implementation class UID,
    /// with trailing characters already excluded.
    pub fn implementation_class_uid(&self) -> &str {
        self.implementation_class_uid
            .trim_end_matches(|c: char| c.is_whitespace() || c == '\0')
    }

    /// Getter for the private information creator UID,
    /// with trailing characters already excluded.
    pub fn private_information_creator_uid(&self) -> Option<&str> {
        self.private_information_creator_uid
            .as_ref()
            .map(|s| s.trim_end_matches(|c: char| c.is_whitespace() || c == '\0'))
    }

    /// Set the file meta table's transfer syntax
    /// according to the given transfer syntax descriptor.
    ///
    /// This replaces the table's transfer syntax UID
    /// to the given transfer syntax, without padding to even length.
    /// The information group length field is automatically recalculated.
    pub fn set_transfer_syntax<D, R, W>(&mut self, ts: &TransferSyntax<D, R, W>) {
        self.transfer_syntax = ts
            .uid()
            .trim_end_matches(|c: char| c.is_whitespace() || c == '\0')
            .to_string();
        self.update_information_group_length();
    }

    /// Calculate the expected file meta group length
    /// according to the file meta attributes currently set,
    /// and assign it to the field `information_group_length`.
    pub fn update_information_group_length(&mut self) {
        self.information_group_length = self.calculate_information_group_length();
    }

    /// Apply the given attribute operation on this file meta information table.
    ///
    /// See the [`dicom_core::ops`] module
    /// for more information.
    fn apply(&mut self, op: AttributeOp) -> ApplyResult {
        let AttributeSelectorStep::Tag(tag) = op.selector.first_step() else {
            return UnsupportedAttributeSnafu.fail();
        };

        match *tag {
            tags::TRANSFER_SYNTAX_UID => Self::apply_required_string(op, &mut self.transfer_syntax),
            tags::MEDIA_STORAGE_SOP_CLASS_UID => {
                Self::apply_required_string(op, &mut self.media_storage_sop_class_uid)
            }
            tags::MEDIA_STORAGE_SOP_INSTANCE_UID => {
                Self::apply_required_string(op, &mut self.media_storage_sop_instance_uid)
            }
            tags::IMPLEMENTATION_CLASS_UID => {
                Self::apply_required_string(op, &mut self.implementation_class_uid)
            }
            tags::IMPLEMENTATION_VERSION_NAME => {
                Self::apply_optional_string(op, &mut self.implementation_version_name)
            }
            tags::SOURCE_APPLICATION_ENTITY_TITLE => {
                Self::apply_optional_string(op, &mut self.source_application_entity_title)
            }
            tags::SENDING_APPLICATION_ENTITY_TITLE => {
                Self::apply_optional_string(op, &mut self.sending_application_entity_title)
            }
            tags::RECEIVING_APPLICATION_ENTITY_TITLE => {
                Self::apply_optional_string(op, &mut self.receiving_application_entity_title)
            }
            tags::PRIVATE_INFORMATION_CREATOR_UID => {
                Self::apply_optional_string(op, &mut self.private_information_creator_uid)
            }
            _ if matches!(
                op.action,
                AttributeAction::Remove | AttributeAction::Empty | AttributeAction::Truncate(_)
            ) =>
            {
                // any other attribute is not supported
                // (ignore Remove, Empty, Truncate)
                Ok(())
            }
            _ => UnsupportedAttributeSnafu.fail(),
        }?;

        self.update_information_group_length();

        Ok(())
    }

    fn apply_required_string(op: AttributeOp, target_attribute: &mut String) -> ApplyResult {
        match op.action {
            AttributeAction::Remove | AttributeAction::Empty => MandatorySnafu.fail(),
            AttributeAction::SetVr(_) | AttributeAction::Truncate(_) => {
                // ignore
                Ok(())
            }
            AttributeAction::Set(value) | AttributeAction::Replace(value) => {
                // require value to be textual
                if let Ok(value) = value.string() {
                    *target_attribute = value.to_string();
                    Ok(())
                } else {
                    IncompatibleTypesSnafu {
                        kind: ValueType::Str,
                    }
                    .fail()
                }
            }
            AttributeAction::SetStr(string) | AttributeAction::ReplaceStr(string) => {
                *target_attribute = string.to_string();
                Ok(())
            }
            AttributeAction::SetIfMissing(_) | AttributeAction::SetStrIfMissing(_) => {
                // no-op
                Ok(())
            }
            AttributeAction::PushStr(_) => IllegalExtendSnafu.fail(),
            AttributeAction::PushI32(_)
            | AttributeAction::PushU32(_)
            | AttributeAction::PushI16(_)
            | AttributeAction::PushU16(_)
            | AttributeAction::PushF32(_)
            | AttributeAction::PushF64(_) => IncompatibleTypesSnafu {
                kind: ValueType::Str,
            }
            .fail(),
            _ => UnsupportedActionSnafu.fail(),
        }
    }

    fn apply_optional_string(
        op: AttributeOp,
        target_attribute: &mut Option<String>,
    ) -> ApplyResult {
        match op.action {
            AttributeAction::Remove => {
                target_attribute.take();
                Ok(())
            }
            AttributeAction::Empty => {
                if let Some(s) = target_attribute.as_mut() {
                    s.clear();
                }
                Ok(())
            }
            AttributeAction::SetVr(_) => {
                // ignore
                Ok(())
            }
            AttributeAction::Set(value) => {
                // require value to be textual
                if let Ok(value) = value.string() {
                    *target_attribute = Some(value.to_string());
                    Ok(())
                } else {
                    IncompatibleTypesSnafu {
                        kind: ValueType::Str,
                    }
                    .fail()
                }
            }
            AttributeAction::SetStr(value) => {
                *target_attribute = Some(value.to_string());
                Ok(())
            }
            AttributeAction::SetIfMissing(value) => {
                if target_attribute.is_some() {
                    return Ok(());
                }

                // require value to be textual
                if let Ok(value) = value.string() {
                    *target_attribute = Some(value.to_string());
                    Ok(())
                } else {
                    IncompatibleTypesSnafu {
                        kind: ValueType::Str,
                    }
                    .fail()
                }
            }
            AttributeAction::SetStrIfMissing(value) => {
                if target_attribute.is_none() {
                    *target_attribute = Some(value.to_string());
                }
                Ok(())
            }
            AttributeAction::Replace(value) => {
                if target_attribute.is_none() {
                    return Ok(());
                }

                // require value to be textual
                if let Ok(value) = value.string() {
                    *target_attribute = Some(value.to_string());
                    Ok(())
                } else {
                    IncompatibleTypesSnafu {
                        kind: ValueType::Str,
                    }
                    .fail()
                }
            }
            AttributeAction::ReplaceStr(value) => {
                if target_attribute.is_some() {
                    *target_attribute = Some(value.to_string());
                }
                Ok(())
            }
            AttributeAction::PushStr(_) => IllegalExtendSnafu.fail(),
            AttributeAction::PushI32(_)
            | AttributeAction::PushU32(_)
            | AttributeAction::PushI16(_)
            | AttributeAction::PushU16(_)
            | AttributeAction::PushF32(_)
            | AttributeAction::PushF64(_) => IncompatibleTypesSnafu {
                kind: ValueType::Str,
            }
            .fail(),
            _ => UnsupportedActionSnafu.fail(),
        }
    }

    /// Calculate the expected file meta group length,
    /// ignoring `information_group_length`.
    fn calculate_information_group_length(&self) -> u32 {
        // determine the expected meta group size based on the given fields.
        // attribute FileMetaInformationGroupLength is not included
        // in the calculations intentionally
        14 + 8
            + dicom_len(&self.media_storage_sop_class_uid)
            + 8
            + dicom_len(&self.media_storage_sop_instance_uid)
            + 8
            + dicom_len(&self.transfer_syntax)
            + 8
            + dicom_len(&self.implementation_class_uid)
            + self
                .implementation_version_name
                .as_ref()
                .map(|s| 8 + dicom_len(s))
                .unwrap_or(0)
            + self
                .source_application_entity_title
                .as_ref()
                .map(|s| 8 + dicom_len(s))
                .unwrap_or(0)
            + self
                .sending_application_entity_title
                .as_ref()
                .map(|s| 8 + dicom_len(s))
                .unwrap_or(0)
            + self
                .receiving_application_entity_title
                .as_ref()
                .map(|s| 8 + dicom_len(s))
                .unwrap_or(0)
            + self
                .private_information_creator_uid
                .as_ref()
                .map(|s| 8 + dicom_len(s))
                .unwrap_or(0)
            + self
                .private_information
                .as_ref()
                .map(|x| 12 + ((x.len() as u32 + 1) & !1))
                .unwrap_or(0)
    }

    /// Read the DICOM magic code (`b"DICM"`)
    /// and the whole file meta group from the given reader.
    fn read_from<S: Read>(mut file: S) -> Result<Self> {
        let mut buff: [u8; 4] = [0; 4];
        {
            // check magic code
            file.read_exact(&mut buff).context(ReadMagicCodeSnafu)?;

            ensure!(buff == DICM_MAGIC_CODE, NotDicomSnafu);
        }

        let decoder = decode::file_header_decoder();
        let text = text::DefaultCharacterSetCodec;

        let builder = FileMetaTableBuilder::new();

        let group_length: u32 = {
            let (elem, _bytes_read) = decoder
                .decode_header(&mut file)
                .context(DecodeElementSnafu)?;
            if elem.tag() != Tag(0x0002, 0x0000) {
                return UnexpectedTagSnafu { tag: elem.tag() }.fail();
            }
            if elem.length() != Length(4) {
                return UnexpectedDataValueLengthSnafu {
                    tag: elem.tag(),
                    length: elem.length(),
                }
                .fail();
            }
            let mut buff: [u8; 4] = [0; 4];
            file.read_exact(&mut buff).context(ReadValueDataSnafu)?;
            LittleEndian::read_u32(&buff)
        };

        let mut total_bytes_read = 0;
        let mut builder = builder.group_length(group_length);

        // Fetch optional data elements
        while total_bytes_read < group_length {
            let (elem, header_bytes_read) = decoder
                .decode_header(&mut file)
                .context(DecodeElementSnafu)?;
            let elem_len = match elem.length().get() {
                None => {
                    return UndefinedValueLengthSnafu { tag: elem.tag() }.fail();
                }
                Some(len) => len,
            };
            builder = match elem.tag() {
                Tag(0x0002, 0x0001) => {
                    // Implementation Version
                    if elem.length() != Length(2) {
                        return UnexpectedDataValueLengthSnafu {
                            tag: elem.tag(),
                            length: elem.length(),
                        }
                        .fail();
                    }
                    let mut hbuf = [0u8; 2];
                    file.read_exact(&mut hbuf[..]).context(ReadValueDataSnafu)?;

                    builder.information_version(hbuf)
                }
                // Media Storage SOP Class UID
                Tag(0x0002, 0x0002) => {
                    builder.media_storage_sop_class_uid(read_str_body(&mut file, &text, elem_len)?)
                }
                // Media Storage SOP Instance UID
                Tag(0x0002, 0x0003) => builder
                    .media_storage_sop_instance_uid(read_str_body(&mut file, &text, elem_len)?),
                // Transfer Syntax
                Tag(0x0002, 0x0010) => {
                    builder.transfer_syntax(read_str_body(&mut file, &text, elem_len)?)
                }
                // Implementation Class UID
                Tag(0x0002, 0x0012) => {
                    builder.implementation_class_uid(read_str_body(&mut file, &text, elem_len)?)
                }
                Tag(0x0002, 0x0013) => {
                    // Implementation Version Name
                    let mut v = Vec::new();
                    v.try_reserve_exact(elem_len as usize)
                        .context(AllocationSizeSnafu)?;
                    v.resize(elem_len as usize, 0);
                    file.read_exact(&mut v).context(ReadValueDataSnafu)?;

                    builder.implementation_version_name(
                        text.decode(&v)
                            .context(DecodeTextSnafu { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0016) => {
                    // Source Application Entity Title
                    let mut v = Vec::new();
                    v.try_reserve_exact(elem_len as usize)
                        .context(AllocationSizeSnafu)?;
                    v.resize(elem_len as usize, 0);
                    file.read_exact(&mut v).context(ReadValueDataSnafu)?;

                    builder.source_application_entity_title(
                        text.decode(&v)
                            .context(DecodeTextSnafu { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0017) => {
                    // Sending Application Entity Title
                    let mut v = Vec::new();
                    v.try_reserve_exact(elem_len as usize)
                        .context(AllocationSizeSnafu)?;
                    v.resize(elem_len as usize, 0);
                    file.read_exact(&mut v).context(ReadValueDataSnafu)?;

                    builder.sending_application_entity_title(
                        text.decode(&v)
                            .context(DecodeTextSnafu { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0018) => {
                    // Receiving Application Entity Title
                    let mut v = Vec::new();
                    v.try_reserve_exact(elem_len as usize)
                        .context(AllocationSizeSnafu)?;
                    v.resize(elem_len as usize, 0);
                    file.read_exact(&mut v).context(ReadValueDataSnafu)?;

                    builder.receiving_application_entity_title(
                        text.decode(&v)
                            .context(DecodeTextSnafu { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0100) => {
                    // Private Information Creator UID
                    let mut v = Vec::new();
                    v.try_reserve_exact(elem_len as usize)
                        .context(AllocationSizeSnafu)?;
                    v.resize(elem_len as usize, 0);
                    file.read_exact(&mut v).context(ReadValueDataSnafu)?;

                    builder.private_information_creator_uid(
                        text.decode(&v)
                            .context(DecodeTextSnafu { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0102) => {
                    // Private Information
                    let mut v = Vec::new();
                    v.try_reserve_exact(elem_len as usize)
                        .context(AllocationSizeSnafu)?;
                    v.resize(elem_len as usize, 0);
                    file.read_exact(&mut v).context(ReadValueDataSnafu)?;

                    builder.private_information(v)
                }
                tag @ Tag(0x0002, _) => {
                    // unknown tag, do nothing
                    // could be an unsupported or non-standard attribute
                    tracing::info!("Unknown tag {}", tag);
                    // consume value without saving it
                    let bytes_read =
                        std::io::copy(&mut (&mut file).take(elem_len as u64), &mut std::io::sink())
                            .context(ReadValueDataSnafu)?;
                    if bytes_read != elem_len as u64 {
                        // reported element length longer than actual stream
                        return UnexpectedDataValueLengthSnafu {
                            tag: elem.tag(),
                            length: elem_len,
                        }
                        .fail();
                    }
                    builder
                }
                tag => {
                    // unexpected tag from another group! do nothing for now,
                    // but this could pose an issue up ahead (see #50)
                    tracing::warn!("Unexpected off-group tag {}", tag);
                    // consume value without saving it
                    let bytes_read =
                        std::io::copy(&mut (&mut file).take(elem_len as u64), &mut std::io::sink())
                            .context(ReadValueDataSnafu)?;
                    if bytes_read != elem_len as u64 {
                        // reported element length longer than actual stream
                        return UnexpectedDataValueLengthSnafu {
                            tag: elem.tag(),
                            length: elem_len,
                        }
                        .fail();
                    }
                    builder
                }
            };
            total_bytes_read = total_bytes_read
                .saturating_add(header_bytes_read as u32)
                .saturating_add(elem_len);
        }

        builder.build()
    }

    /// Create an iterator over the defined data elements
    /// of the file meta group,
    /// consuming the file meta table.
    ///
    /// See [`to_element_iter`](FileMetaTable::to_element_iter)
    /// for a version which copies the element from the table.
    pub fn into_element_iter(self) -> impl Iterator<Item = DataElement<EmptyObject, [u8; 0]>> {
        let mut elems = vec![
            // file information group length
            DataElement::new(
                Tag(0x0002, 0x0000),
                VR::UL,
                Value::Primitive(self.information_group_length.into()),
            ),
            DataElement::new(
                Tag(0x0002, 0x0001),
                VR::OB,
                Value::Primitive(dicom_value!(
                    U8,
                    [self.information_version[0], self.information_version[1]]
                )),
            ),
            DataElement::new(
                Tag(0x0002, 0x0002),
                VR::UI,
                Value::Primitive(self.media_storage_sop_class_uid.into()),
            ),
            DataElement::new(
                Tag(0x0002, 0x0003),
                VR::UI,
                Value::Primitive(self.media_storage_sop_instance_uid.into()),
            ),
            DataElement::new(
                Tag(0x0002, 0x0010),
                VR::UI,
                Value::Primitive(self.transfer_syntax.into()),
            ),
            DataElement::new(
                Tag(0x0002, 0x0012),
                VR::UI,
                Value::Primitive(self.implementation_class_uid.into()),
            ),
        ];
        if let Some(v) = self.implementation_version_name {
            elems.push(DataElement::new(
                Tag(0x0002, 0x0013),
                VR::SH,
                Value::Primitive(v.into()),
            ));
        }
        if let Some(v) = self.source_application_entity_title {
            elems.push(DataElement::new(
                Tag(0x0002, 0x0016),
                VR::AE,
                Value::Primitive(v.into()),
            ));
        }
        if let Some(v) = self.sending_application_entity_title {
            elems.push(DataElement::new(
                Tag(0x0002, 0x0017),
                VR::AE,
                Value::Primitive(v.into()),
            ));
        }
        if let Some(v) = self.receiving_application_entity_title {
            elems.push(DataElement::new(
                Tag(0x0002, 0x0018),
                VR::AE,
                Value::Primitive(v.into()),
            ));
        }
        if let Some(v) = self.private_information_creator_uid {
            elems.push(DataElement::new(
                Tag(0x0002, 0x0100),
                VR::UI,
                Value::Primitive(v.into()),
            ));
        }
        if let Some(v) = self.private_information {
            elems.push(DataElement::new(
                Tag(0x0002, 0x0102),
                VR::OB,
                Value::Primitive(PrimitiveValue::U8(v.into())),
            ));
        }

        elems.into_iter()
    }

    /// Create an iterator of data elements copied from the file meta group.
    ///
    /// See [`into_element_iter`](FileMetaTable::into_element_iter)
    /// for a version which consumes the table.
    pub fn to_element_iter(&self) -> impl Iterator<Item = DataElement<EmptyObject, [u8; 0]>> + '_ {
        self.clone().into_element_iter()
    }

    pub fn write<W: Write>(&self, writer: W) -> Result<()> {
        let mut dset = DataSetWriter::new(
            writer,
            EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
        );
        //There are no sequences in the `FileMetaTable`, so the value of `invalidate_sq_len` is
        //not important
        dset.write_sequence(
            self.clone()
                .into_element_iter()
                .flat_map(IntoTokens::into_tokens),
        )
        .context(WriteSetSnafu)
    }
}

/// An attribute selector for a file meta information table.
#[derive(Debug)]
pub struct FileMetaAttribute<'a> {
    meta: &'a FileMetaTable,
    tag_e: u16,
}

impl HasLength for FileMetaAttribute<'_> {
    fn length(&self) -> Length {
        match Tag(0x0002, self.tag_e) {
            tags::FILE_META_INFORMATION_GROUP_LENGTH => Length(4),
            tags::MEDIA_STORAGE_SOP_CLASS_UID => {
                Length(self.meta.media_storage_sop_class_uid.len() as u32)
            }
            tags::MEDIA_STORAGE_SOP_INSTANCE_UID => {
                Length(self.meta.media_storage_sop_instance_uid.len() as u32)
            }
            tags::IMPLEMENTATION_CLASS_UID => {
                Length(self.meta.implementation_class_uid.len() as u32)
            }
            tags::IMPLEMENTATION_VERSION_NAME => Length(
                self.meta
                    .implementation_version_name
                    .as_ref()
                    .map(|s| s.len() as u32)
                    .unwrap_or(0),
            ),
            tags::SOURCE_APPLICATION_ENTITY_TITLE => Length(
                self.meta
                    .source_application_entity_title
                    .as_ref()
                    .map(|s| s.len() as u32)
                    .unwrap_or(0),
            ),
            tags::SENDING_APPLICATION_ENTITY_TITLE => Length(
                self.meta
                    .sending_application_entity_title
                    .as_ref()
                    .map(|s| s.len() as u32)
                    .unwrap_or(0),
            ),
            tags::TRANSFER_SYNTAX_UID => Length(self.meta.transfer_syntax.len() as u32),
            tags::PRIVATE_INFORMATION_CREATOR_UID => Length(
                self.meta
                    .private_information_creator_uid
                    .as_ref()
                    .map(|s| s.len() as u32)
                    .unwrap_or(0),
            ),
            _ => unreachable!(),
        }
    }
}

impl DicomValueType for FileMetaAttribute<'_> {
    fn value_type(&self) -> ValueType {
        match Tag(0x0002, self.tag_e) {
            tags::MEDIA_STORAGE_SOP_CLASS_UID
            | tags::MEDIA_STORAGE_SOP_INSTANCE_UID
            | tags::TRANSFER_SYNTAX_UID
            | tags::IMPLEMENTATION_CLASS_UID
            | tags::IMPLEMENTATION_VERSION_NAME
            | tags::SOURCE_APPLICATION_ENTITY_TITLE
            | tags::SENDING_APPLICATION_ENTITY_TITLE
            | tags::RECEIVING_APPLICATION_ENTITY_TITLE
            | tags::PRIVATE_INFORMATION_CREATOR_UID => ValueType::Str,
            tags::FILE_META_INFORMATION_GROUP_LENGTH => ValueType::U32,
            tags::FILE_META_INFORMATION_VERSION => ValueType::U8,
            tags::PRIVATE_INFORMATION => ValueType::U8,
            _ => unreachable!(),
        }
    }

    fn cardinality(&self) -> usize {
        match Tag(0x0002, self.tag_e) {
            tags::MEDIA_STORAGE_SOP_CLASS_UID
            | tags::MEDIA_STORAGE_SOP_INSTANCE_UID
            | tags::SOURCE_APPLICATION_ENTITY_TITLE
            | tags::SENDING_APPLICATION_ENTITY_TITLE
            | tags::RECEIVING_APPLICATION_ENTITY_TITLE
            | tags::TRANSFER_SYNTAX_UID
            | tags::IMPLEMENTATION_CLASS_UID
            | tags::IMPLEMENTATION_VERSION_NAME
            | tags::PRIVATE_INFORMATION_CREATOR_UID => 1,
            tags::FILE_META_INFORMATION_GROUP_LENGTH => 1,
            tags::PRIVATE_INFORMATION => 1,
            tags::FILE_META_INFORMATION_VERSION => 2,
            _ => 1,
        }
    }
}

impl DicomAttribute for FileMetaAttribute<'_> {
    type Item<'b> = EmptyObject
        where Self: 'b;
    type PixelData<'b> = InMemFragment
        where Self: 'b;

    fn to_primitive_value(&self) -> Result<PrimitiveValue, AttributeError> {
        Ok(match Tag(0x0002, self.tag_e) {
            tags::FILE_META_INFORMATION_GROUP_LENGTH => {
                PrimitiveValue::from(self.meta.information_group_length)
            }
            tags::FILE_META_INFORMATION_VERSION => {
                PrimitiveValue::from(self.meta.information_version)
            }
            tags::MEDIA_STORAGE_SOP_CLASS_UID => {
                PrimitiveValue::from(self.meta.media_storage_sop_class_uid.clone())
            }
            tags::MEDIA_STORAGE_SOP_INSTANCE_UID => {
                PrimitiveValue::from(self.meta.media_storage_sop_instance_uid.clone())
            }
            tags::SOURCE_APPLICATION_ENTITY_TITLE => {
                PrimitiveValue::from(self.meta.source_application_entity_title.clone().unwrap())
            }
            tags::SENDING_APPLICATION_ENTITY_TITLE => {
                PrimitiveValue::from(self.meta.sending_application_entity_title.clone().unwrap())
            }
            tags::RECEIVING_APPLICATION_ENTITY_TITLE => PrimitiveValue::from(
                self.meta
                    .receiving_application_entity_title
                    .clone()
                    .unwrap(),
            ),
            tags::TRANSFER_SYNTAX_UID => PrimitiveValue::from(self.meta.transfer_syntax.clone()),
            tags::IMPLEMENTATION_CLASS_UID => {
                PrimitiveValue::from(self.meta.implementation_class_uid.clone())
            }
            tags::IMPLEMENTATION_VERSION_NAME => {
                PrimitiveValue::from(self.meta.implementation_version_name.clone().unwrap())
            }
            tags::PRIVATE_INFORMATION_CREATOR_UID => {
                PrimitiveValue::from(self.meta.private_information_creator_uid.clone().unwrap())
            }
            tags::PRIVATE_INFORMATION => {
                PrimitiveValue::from(self.meta.private_information.clone().unwrap())
            }
            _ => unreachable!(),
        })
    }

    fn to_str(&self) -> std::result::Result<std::borrow::Cow<'_, str>, AttributeError> {
        match Tag(0x0002, self.tag_e) {
            tags::FILE_META_INFORMATION_GROUP_LENGTH => {
                Ok(self.meta.information_group_length.to_string().into())
            }
            tags::FILE_META_INFORMATION_VERSION => Ok(format!(
                "{:02X}{:02X}",
                self.meta.information_version[0], self.meta.information_version[1]
            )
            .into()),
            tags::MEDIA_STORAGE_SOP_CLASS_UID => {
                Ok(Cow::Borrowed(self.meta.media_storage_sop_class_uid()))
            }
            tags::MEDIA_STORAGE_SOP_INSTANCE_UID => {
                Ok(Cow::Borrowed(self.meta.media_storage_sop_instance_uid()))
            }
            tags::TRANSFER_SYNTAX_UID => Ok(Cow::Borrowed(self.meta.transfer_syntax())),
            tags::IMPLEMENTATION_CLASS_UID => {
                Ok(Cow::Borrowed(self.meta.implementation_class_uid()))
            }
            tags::IMPLEMENTATION_VERSION_NAME => Ok(self
                .meta
                .implementation_version_name
                .as_deref()
                .map(Cow::Borrowed)
                .unwrap_or_default()),
            tags::SOURCE_APPLICATION_ENTITY_TITLE => Ok(self
                .meta
                .source_application_entity_title
                .as_deref()
                .map(Cow::Borrowed)
                .unwrap_or_default()),
            tags::SENDING_APPLICATION_ENTITY_TITLE => Ok(self
                .meta
                .sending_application_entity_title
                .as_deref()
                .map(Cow::Borrowed)
                .unwrap_or_default()),
            tags::RECEIVING_APPLICATION_ENTITY_TITLE => Ok(self
                .meta
                .receiving_application_entity_title
                .as_deref()
                .map(Cow::Borrowed)
                .unwrap_or_default()),
            tags::PRIVATE_INFORMATION_CREATOR_UID => Ok(self
                .meta
                .private_information_creator_uid
                .as_deref()
                .map(|v| {
                    Cow::Borrowed(v.trim_end_matches(|c: char| c.is_whitespace() || c == '\0'))
                })
                .unwrap_or_default()),
            tags::PRIVATE_INFORMATION => Err(AttributeError::ConvertValue {
                source: ConvertValueError {
                    cause: None,
                    original: ValueType::U8,
                    requested: "str",
                },
            }),
            _ => unreachable!(),
        }
    }

    fn item(&self, _index: u32) -> Result<Self::Item<'_>, AttributeError> {
        Err(AttributeError::NotDataSet)
    }

    fn num_items(&self) -> Option<u32> {
        None
    }

    fn fragment(&self, _index: u32) -> Result<Self::PixelData<'_>, AttributeError> {
        Err(AttributeError::NotPixelData)
    }

    fn num_fragments(&self) -> Option<u32> {
        None
    }
}

impl DicomObject for FileMetaTable {
    type Attribute<'a> = FileMetaAttribute<'a>
    where
        Self: 'a;

    fn get_opt(
        &self,
        tag: Tag,
    ) -> std::result::Result<Option<Self::Attribute<'_>>, crate::AccessError> {
        // check that the attribute value is in the table,
        // then return a suitable `FileMetaAttribute`

        if match tag {
            // mandatory attributes
            tags::FILE_META_INFORMATION_GROUP_LENGTH
            | tags::FILE_META_INFORMATION_VERSION
            | tags::MEDIA_STORAGE_SOP_CLASS_UID
            | tags::MEDIA_STORAGE_SOP_INSTANCE_UID
            | tags::TRANSFER_SYNTAX_UID
            | tags::IMPLEMENTATION_CLASS_UID
            | tags::IMPLEMENTATION_VERSION_NAME => true,
            // optional attributes
            tags::SOURCE_APPLICATION_ENTITY_TITLE
                if self.source_application_entity_title.is_some() =>
            {
                true
            }
            tags::SENDING_APPLICATION_ENTITY_TITLE
                if self.sending_application_entity_title.is_some() =>
            {
                true
            }
            tags::RECEIVING_APPLICATION_ENTITY_TITLE
                if self.receiving_application_entity_title.is_some() =>
            {
                true
            }
            tags::PRIVATE_INFORMATION_CREATOR_UID
                if self.private_information_creator_uid.is_some() =>
            {
                true
            }
            tags::PRIVATE_INFORMATION if self.private_information.is_some() => true,
            _ => false,
        } {
            Ok(Some(FileMetaAttribute {
                meta: self,
                tag_e: tag.element(),
            }))
        } else {
            Ok(None)
        }
    }

    fn get_by_name_opt<'a>(
        &'a self,
        name: &str,
    ) -> std::result::Result<Option<Self::Attribute<'a>>, crate::AccessByNameError> {
        let tag = match name {
            "FileMetaInformationGroupLength" => tags::FILE_META_INFORMATION_GROUP_LENGTH,
            "FileMetaInformationVersion" => tags::FILE_META_INFORMATION_VERSION,
            "MediaStorageSOPClassUID" => tags::MEDIA_STORAGE_SOP_CLASS_UID,
            "MediaStorageSOPInstanceUID" => tags::MEDIA_STORAGE_SOP_INSTANCE_UID,
            "TransferSyntaxUID" => tags::TRANSFER_SYNTAX_UID,
            "ImplementationClassUID" => tags::IMPLEMENTATION_CLASS_UID,
            "ImplementationVersionName" => tags::IMPLEMENTATION_VERSION_NAME,
            "SourceApplicationEntityTitle" => tags::SOURCE_APPLICATION_ENTITY_TITLE,
            "SendingApplicationEntityTitle" => tags::SENDING_APPLICATION_ENTITY_TITLE,
            "ReceivingApplicationEntityTitle" => tags::RECEIVING_APPLICATION_ENTITY_TITLE,
            "PrivateInformationCreatorUID" => tags::PRIVATE_INFORMATION_CREATOR_UID,
            "PrivateInformation" => tags::PRIVATE_INFORMATION,
            _ => return Ok(None),
        };
        self.get_opt(tag)
            .map_err(|_| crate::NoSuchAttributeNameSnafu { name }.build())
    }
}

impl ApplyOp for FileMetaTable {
    type Err = ApplyError;

    /// Apply the given attribute operation on this file meta information table.
    ///
    /// See the [`dicom_core::ops`] module
    /// for more information.
    fn apply(&mut self, op: AttributeOp) -> ApplyResult {
        self.apply(op)
    }
}

/// A builder for DICOM meta information tables.
#[derive(Debug, Default, Clone)]
pub struct FileMetaTableBuilder {
    /// File Meta Information Group Length (UL)
    information_group_length: Option<u32>,
    /// File Meta Information Version (OB)
    information_version: Option<[u8; 2]>,
    /// Media Storage SOP Class UID (UI)
    media_storage_sop_class_uid: Option<String>,
    /// Media Storage SOP Instance UID (UI)
    media_storage_sop_instance_uid: Option<String>,
    /// Transfer Syntax UID (UI)
    transfer_syntax: Option<String>,
    /// Implementation Class UID (UI)
    implementation_class_uid: Option<String>,

    /// Implementation Version Name (SH)
    implementation_version_name: Option<String>,
    /// Source Application Entity Title (AE)
    source_application_entity_title: Option<String>,
    /// Sending Application Entity Title (AE)
    sending_application_entity_title: Option<String>,
    /// Receiving Application Entity Title (AE)
    receiving_application_entity_title: Option<String>,
    /// Private Information Creator UID (UI)
    private_information_creator_uid: Option<String>,
    /// Private Information (OB)
    private_information: Option<Vec<u8>>,
}

/// Ensure that the string is even lengthed, by adding a trailing character
/// if not.
#[inline]
fn padded<T>(s: T, pad: char) -> String
where
    T: Into<String>,
{
    let mut s = s.into();
    if s.len() % 2 == 1 {
        s.push(pad);
    }
    s
}

/// Ensure that the string is even lengthed with trailing '\0's.
fn ui_padded<T>(s: T) -> String
where
    T: Into<String>,
{
    padded(s, '\0')
}

/// Ensure that the string is even lengthed with trailing spaces.
fn txt_padded<T>(s: T) -> String
where
    T: Into<String>,
{
    padded(s, ' ')
}

impl FileMetaTableBuilder {
    /// Create a new, empty builder.
    pub fn new() -> FileMetaTableBuilder {
        FileMetaTableBuilder::default()
    }

    /// Define the meta information group length.
    pub fn group_length(mut self, value: u32) -> FileMetaTableBuilder {
        self.information_group_length = Some(value);
        self
    }

    /// Define the meta information version.
    pub fn information_version(mut self, value: [u8; 2]) -> FileMetaTableBuilder {
        self.information_version = Some(value);
        self
    }

    /// Define the media storage SOP class UID.
    pub fn media_storage_sop_class_uid<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.media_storage_sop_class_uid = Some(ui_padded(value));
        self
    }

    /// Define the media storage SOP instance UID.
    pub fn media_storage_sop_instance_uid<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.media_storage_sop_instance_uid = Some(ui_padded(value));
        self
    }

    /// Define the transfer syntax UID.
    pub fn transfer_syntax<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.transfer_syntax = Some(ui_padded(value));
        self
    }

    /// Define the implementation class UID.
    pub fn implementation_class_uid<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.implementation_class_uid = Some(ui_padded(value));
        self
    }

    /// Define the implementation version name.
    pub fn implementation_version_name<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.implementation_version_name = Some(txt_padded(value));
        self
    }

    /// Define the source application entity title.
    pub fn source_application_entity_title<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.source_application_entity_title = Some(txt_padded(value));
        self
    }

    /// Define the sending application entity title.
    pub fn sending_application_entity_title<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.sending_application_entity_title = Some(txt_padded(value));
        self
    }

    /// Define the receiving application entity title.
    pub fn receiving_application_entity_title<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.receiving_application_entity_title = Some(txt_padded(value));
        self
    }

    /// Define the private information creator UID.
    pub fn private_information_creator_uid<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<String>,
    {
        self.private_information_creator_uid = Some(ui_padded(value));
        self
    }

    /// Define the private information as a vector of bytes.
    pub fn private_information<T>(mut self, value: T) -> FileMetaTableBuilder
    where
        T: Into<Vec<u8>>,
    {
        self.private_information = Some(value.into());
        self
    }

    /// Build the table.
    pub fn build(self) -> Result<FileMetaTable> {
        let information_version = self.information_version.unwrap_or(
            // Missing information version, will assume (00H, 01H). See #28
            [0, 1],
        );
        let media_storage_sop_class_uid = self.media_storage_sop_class_uid.unwrap_or_else(|| {
            tracing::warn!("MediaStorageSOPClassUID is missing. Defaulting to empty string.");
            String::default()
        });
        let media_storage_sop_instance_uid =
            self.media_storage_sop_instance_uid.unwrap_or_else(|| {
                tracing::warn!(
                    "MediaStorageSOPInstanceUID is missing. Defaulting to empty string."
                );
                String::default()
            });
        let transfer_syntax = self.transfer_syntax.context(MissingElementSnafu {
            alias: "TransferSyntax",
        })?;
        let mut implementation_version_name = self.implementation_version_name;
        let implementation_class_uid = self.implementation_class_uid.unwrap_or_else(|| {
            // override implementation version name
            implementation_version_name = Some(IMPLEMENTATION_VERSION_NAME.to_string());

            IMPLEMENTATION_CLASS_UID.to_string()
        });

        let mut table = FileMetaTable {
            // placeholder value which will be replaced on update
            information_group_length: 0x00,
            information_version,
            media_storage_sop_class_uid,
            media_storage_sop_instance_uid,
            transfer_syntax,
            implementation_class_uid,
            implementation_version_name,
            source_application_entity_title: self.source_application_entity_title,
            sending_application_entity_title: self.sending_application_entity_title,
            receiving_application_entity_title: self.receiving_application_entity_title,
            private_information_creator_uid: self.private_information_creator_uid,
            private_information: self.private_information,
        };
        table.update_information_group_length();
        debug_assert!(table.information_group_length > 0);
        Ok(table)
    }
}

fn dicom_len<T: AsRef<str>>(x: T) -> u32 {
    (x.as_ref().len() as u32 + 1) & !1
}

#[cfg(test)]
mod tests {
    use crate::{IMPLEMENTATION_CLASS_UID, IMPLEMENTATION_VERSION_NAME};

    use super::{dicom_len, FileMetaTable, FileMetaTableBuilder};
    use dicom_core::ops::{AttributeAction, AttributeOp};
    use dicom_core::value::Value;
    use dicom_core::{dicom_value, DataElement, PrimitiveValue, Tag, VR};
    use dicom_dictionary_std::tags;

    const TEST_META_1: &[u8] = &[
        // magic code
        b'D', b'I', b'C', b'M',
        // File Meta Information Group Length: (0000,0002) ; UL ; 4 ; 200
        0x02, 0x00, 0x00, 0x00, b'U', b'L', 0x04, 0x00, 0xc8, 0x00, 0x00, 0x00,
        // File Meta Information Version: (0002, 0001) ; OB ; 2 ; [0x00, 0x01]
        0x02, 0x00, 0x01, 0x00, b'O', b'B', 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01,
        // Media Storage SOP Class UID (0002, 0002) ; UI ; 26 ; "1.2.840.10008.5.1.4.1.1.1\0" (ComputedRadiographyImageStorage)
        0x02, 0x00, 0x02, 0x00, b'U', b'I', 0x1a, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30,
        0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x35, 0x2e, 0x31, 0x2e, 0x34, 0x2e, 0x31, 0x2e,
        0x31, 0x2e, 0x31, 0x00,
        // Media Storage SOP Instance UID (0002, 0003) ; UI ; 56 ; "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0"
        0x02, 0x00, 0x03, 0x00, b'U', b'I', 0x38, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x33, 0x2e, 0x34,
        0x2e, 0x35, 0x2e, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x2e, 0x31, 0x32, 0x33,
        0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x30, 0x2e, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37,
        0x2e, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x2e, 0x31, 0x32, 0x33, 0x34,
        0x35, 0x36, 0x37, 0x00,
        // Transfer Syntax UID (0002, 0010) ; UI ; 20 ; "1.2.840.10008.1.2.1\0" (LittleEndianExplicit)
        0x02, 0x00, 0x10, 0x00, b'U', b'I', 0x14, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x38, 0x34, 0x30,
        0x2e, 0x31, 0x30, 0x30, 0x30, 0x38, 0x2e, 0x31, 0x2e, 0x32, 0x2e, 0x31, 0x00,
        // Implementation Class UID (0002, 0012) ; UI ; 20 ; "1.2.345.6.7890.1.234"
        0x02, 0x00, 0x12, 0x00, b'U', b'I', 0x14, 0x00, 0x31, 0x2e, 0x32, 0x2e, 0x33, 0x34, 0x35,
        0x2e, 0x36, 0x2e, 0x37, 0x38, 0x39, 0x30, 0x2e, 0x31, 0x2e, 0x32, 0x33, 0x34,
        // optional elements:

        // Implementation Version Name (0002,0013) ; SH ; "RUSTY_DICOM_269"
        0x02, 0x00, 0x13, 0x00, b'S', b'H', 0x10, 0x00, 0x52, 0x55, 0x53, 0x54, 0x59, 0x5f, 0x44,
        0x49, 0x43, 0x4f, 0x4d, 0x5f, 0x32, 0x36, 0x39, 0x20,
        // Source Application Entity Title (0002, 0016) ; AE ; 0 (no data)
        0x02, 0x00, 0x16, 0x00, b'A', b'E', 0x00, 0x00,
    ];

    #[test]
    fn read_meta_table_from_reader() {
        let mut source = TEST_META_1;

        let table = FileMetaTable::from_reader(&mut source).unwrap();

        let gt = FileMetaTable {
            information_group_length: 200,
            information_version: [0u8, 1u8],
            media_storage_sop_class_uid: "1.2.840.10008.5.1.4.1.1.1\0".to_owned(),
            media_storage_sop_instance_uid:
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0".to_owned(),
            transfer_syntax: "1.2.840.10008.1.2.1\0".to_owned(),
            implementation_class_uid: "1.2.345.6.7890.1.234".to_owned(),
            implementation_version_name: Some("RUSTY_DICOM_269 ".to_owned()),
            source_application_entity_title: Some("".to_owned()),
            sending_application_entity_title: None,
            receiving_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        };

        assert_eq!(table.information_group_length, 200);
        assert_eq!(table.information_version, [0u8, 1u8]);
        assert_eq!(
            table.media_storage_sop_class_uid,
            "1.2.840.10008.5.1.4.1.1.1\0"
        );
        assert_eq!(
            table.media_storage_sop_instance_uid,
            "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0"
        );
        assert_eq!(table.transfer_syntax, "1.2.840.10008.1.2.1\0");
        assert_eq!(table.implementation_class_uid, "1.2.345.6.7890.1.234");
        assert_eq!(
            table.implementation_version_name,
            Some("RUSTY_DICOM_269 ".to_owned())
        );
        assert_eq!(table.source_application_entity_title, Some("".into()));
        assert_eq!(table.sending_application_entity_title, None);
        assert_eq!(table.receiving_application_entity_title, None);
        assert_eq!(table.private_information_creator_uid, None);
        assert_eq!(table.private_information, None);

        assert_eq!(table, gt);
    }

    #[test]
    fn create_meta_table_with_builder() {
        let table = FileMetaTableBuilder::new()
            .information_version([0, 1])
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
            .media_storage_sop_instance_uid(
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567",
            )
            .transfer_syntax("1.2.840.10008.1.2.1")
            .implementation_class_uid("1.2.345.6.7890.1.234")
            .implementation_version_name("RUSTY_DICOM_269")
            .source_application_entity_title("")
            .build()
            .unwrap();

        let gt = FileMetaTable {
            information_group_length: 200,
            information_version: [0u8, 1u8],
            media_storage_sop_class_uid: "1.2.840.10008.5.1.4.1.1.1\0".to_owned(),
            media_storage_sop_instance_uid:
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0".to_owned(),
            transfer_syntax: "1.2.840.10008.1.2.1\0".to_owned(),
            implementation_class_uid: "1.2.345.6.7890.1.234".to_owned(),
            implementation_version_name: Some("RUSTY_DICOM_269 ".to_owned()),
            source_application_entity_title: Some("".to_owned()),
            sending_application_entity_title: None,
            receiving_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        };

        assert_eq!(table.information_group_length, gt.information_group_length);
        assert_eq!(table, gt);
    }

    /// Build a file meta table with the minimum set of parameters.
    #[test]
    fn create_meta_table_with_builder_minimal() {
        let table = FileMetaTableBuilder::new()
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
            .media_storage_sop_instance_uid(
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567",
            )
            .transfer_syntax("1.2.840.10008.1.2")
            .build()
            .unwrap();

        let gt = FileMetaTable {
            information_group_length: 154
                + dicom_len(IMPLEMENTATION_CLASS_UID)
                + dicom_len(IMPLEMENTATION_VERSION_NAME),
            information_version: [0u8, 1u8],
            media_storage_sop_class_uid: "1.2.840.10008.5.1.4.1.1.1\0".to_owned(),
            media_storage_sop_instance_uid:
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0".to_owned(),
            transfer_syntax: "1.2.840.10008.1.2\0".to_owned(),
            implementation_class_uid: IMPLEMENTATION_CLASS_UID.to_owned(),
            implementation_version_name: Some(IMPLEMENTATION_VERSION_NAME.to_owned()),
            source_application_entity_title: None,
            sending_application_entity_title: None,
            receiving_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        };

        assert_eq!(table.information_group_length, gt.information_group_length);
        assert_eq!(table, gt);
    }

    /// Changing the transfer syntax updates the file meta group length.
    #[test]
    fn change_transfer_syntax_update_table() {
        let mut table = FileMetaTableBuilder::new()
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.1")
            .media_storage_sop_instance_uid(
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567",
            )
            .transfer_syntax("1.2.840.10008.1.2.1")
            .build()
            .unwrap();

        assert_eq!(
            table.information_group_length,
            156 + dicom_len(IMPLEMENTATION_CLASS_UID) + dicom_len(IMPLEMENTATION_VERSION_NAME)
        );

        table.set_transfer_syntax(
            &dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN,
        );
        assert_eq!(
            table.information_group_length,
            154 + dicom_len(IMPLEMENTATION_CLASS_UID) + dicom_len(IMPLEMENTATION_VERSION_NAME)
        );
    }

    #[test]
    fn read_meta_table_into_iter() {
        let table = FileMetaTable {
            information_group_length: 200,
            information_version: [0u8, 1u8],
            media_storage_sop_class_uid: "1.2.840.10008.5.1.4.1.1.1\0".to_owned(),
            media_storage_sop_instance_uid:
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0".to_owned(),
            transfer_syntax: "1.2.840.10008.1.2.1\0".to_owned(),
            implementation_class_uid: "1.2.345.6.7890.1.234".to_owned(),
            implementation_version_name: Some("RUSTY_DICOM_269 ".to_owned()),
            source_application_entity_title: Some("".to_owned()),
            sending_application_entity_title: None,
            receiving_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        };

        assert_eq!(table.calculate_information_group_length(), 200);

        let gt = vec![
            // Information Group Length
            DataElement::new(Tag(0x0002, 0x0000), VR::UL, dicom_value!(U32, 200)),
            // Information Version
            DataElement::new(Tag(0x0002, 0x0001), VR::OB, dicom_value!(U8, [0, 1])),
            // Media Storage SOP Class UID
            DataElement::new(
                Tag(0x0002, 0x0002),
                VR::UI,
                Value::Primitive("1.2.840.10008.5.1.4.1.1.1\0".into()),
            ),
            // Media Storage SOP Instance UID
            DataElement::new(
                Tag(0x0002, 0x0003),
                VR::UI,
                Value::Primitive(
                    "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0".into(),
                ),
            ),
            // Transfer Syntax
            DataElement::new(
                Tag(0x0002, 0x0010),
                VR::UI,
                Value::Primitive("1.2.840.10008.1.2.1\0".into()),
            ),
            // Implementation Class UID
            DataElement::new(
                Tag(0x0002, 0x0012),
                VR::UI,
                Value::Primitive("1.2.345.6.7890.1.234".into()),
            ),
            // Implementation Version Name
            DataElement::new(
                Tag(0x0002, 0x0013),
                VR::SH,
                Value::Primitive("RUSTY_DICOM_269 ".into()),
            ),
            // Source Application Entity Title
            DataElement::new(Tag(0x0002, 0x0016), VR::AE, Value::Primitive("".into())),
        ];

        let elems: Vec<_> = table.into_element_iter().collect();
        assert_eq!(elems, gt);
    }

    #[test]
    fn update_table_with_length() {
        let mut table = FileMetaTable {
            information_group_length: 55, // dummy value
            information_version: [0u8, 1u8],
            media_storage_sop_class_uid: "1.2.840.10008.5.1.4.1.1.1\0".to_owned(),
            media_storage_sop_instance_uid:
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0".to_owned(),
            transfer_syntax: "1.2.840.10008.1.2.1\0".to_owned(),
            implementation_class_uid: "1.2.345.6.7890.1.234".to_owned(),
            implementation_version_name: Some("RUSTY_DICOM_269 ".to_owned()),
            source_application_entity_title: Some("".to_owned()),
            sending_application_entity_title: None,
            receiving_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        };

        table.update_information_group_length();

        assert_eq!(table.information_group_length, 200);
    }

    #[test]
    fn table_ops() {
        let mut table = FileMetaTable {
            information_group_length: 200,
            information_version: [0u8, 1u8],
            media_storage_sop_class_uid: "1.2.840.10008.5.1.4.1.1.1\0".to_owned(),
            media_storage_sop_instance_uid:
                "1.2.3.4.5.12345678.1234567890.1234567.123456789.1234567\0".to_owned(),
            transfer_syntax: "1.2.840.10008.1.2.1\0".to_owned(),
            implementation_class_uid: "1.2.345.6.7890.1.234".to_owned(),
            implementation_version_name: None,
            source_application_entity_title: None,
            sending_application_entity_title: None,
            receiving_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        };

        // replace does not set missing attributes
        table
            .apply(AttributeOp::new(
                tags::IMPLEMENTATION_VERSION_NAME,
                AttributeAction::ReplaceStr("MY_DICOM_1.1".into()),
            ))
            .unwrap();

        assert_eq!(table.implementation_version_name, None);

        // but SetStr does
        table
            .apply(AttributeOp::new(
                tags::IMPLEMENTATION_VERSION_NAME,
                AttributeAction::SetStr("MY_DICOM_1.1".into()),
            ))
            .unwrap();

        assert_eq!(
            table.implementation_version_name.as_deref(),
            Some("MY_DICOM_1.1"),
        );

        // Set (primitive) also works
        table
            .apply(AttributeOp::new(
                tags::SOURCE_APPLICATION_ENTITY_TITLE,
                AttributeAction::Set(PrimitiveValue::Str("RICOOGLE-STORAGE".into())),
            ))
            .unwrap();

        assert_eq!(
            table.source_application_entity_title.as_deref(),
            Some("RICOOGLE-STORAGE"),
        );

        // set if missing works only if value isn't set yet
        table
            .apply(AttributeOp::new(
                tags::SOURCE_APPLICATION_ENTITY_TITLE,
                AttributeAction::SetStrIfMissing("STORE-SCU".into()),
            ))
            .unwrap();

        assert_eq!(
            table.source_application_entity_title.as_deref(),
            Some("RICOOGLE-STORAGE"),
        );

        table
            .apply(AttributeOp::new(
                tags::SENDING_APPLICATION_ENTITY_TITLE,
                AttributeAction::SetStrIfMissing("STORE-SCU".into()),
            ))
            .unwrap();

        assert_eq!(
            table.sending_application_entity_title.as_deref(),
            Some("STORE-SCU"),
        );

        // replacing mandatory field
        table
            .apply(AttributeOp::new(
                tags::MEDIA_STORAGE_SOP_CLASS_UID,
                AttributeAction::Replace(PrimitiveValue::Str("1.2.840.10008.5.1.4.1.1.7".into())),
            ))
            .unwrap();

        assert_eq!(
            table.media_storage_sop_class_uid(),
            "1.2.840.10008.5.1.4.1.1.7",
        );
    }

    /// writing file meta information and reading it back
    /// should not fail and the the group length should be the same
    #[test]
    fn write_read_does_not_fail() {
        let mut table = FileMetaTable {
            information_group_length: 0,
            information_version: [0u8, 1u8],
            media_storage_sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_owned(),
            media_storage_sop_instance_uid: "2.25.137731752600317795446120660167595746868"
                .to_owned(),
            transfer_syntax: "1.2.840.10008.1.2.4.91".to_owned(),
            implementation_class_uid: "2.25.305828488182831875890203105390285383139".to_owned(),
            implementation_version_name: Some("MYTOOL100".to_owned()),
            source_application_entity_title: Some("RUSTY".to_owned()),
            receiving_application_entity_title: None,
            sending_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        };

        table.update_information_group_length();

        let mut buf = vec![b'D', b'I', b'C', b'M'];
        table.write(&mut buf).unwrap();

        let table2 = FileMetaTable::from_reader(&mut buf.as_slice())
            .expect("Should not fail to read the table from the written data");

        assert_eq!(
            table.information_group_length,
            table2.information_group_length
        );
    }

    /// Can access file meta properties via the DicomObject trait
    #[test]
    fn dicom_object_api() {
        use crate::{DicomAttribute as _, DicomObject as _};
        use dicom_dictionary_std::uids;

        let meta = FileMetaTableBuilder::new()
            .transfer_syntax(uids::RLE_LOSSLESS)
            .media_storage_sop_class_uid(uids::ENHANCED_MR_IMAGE_STORAGE)
            .media_storage_sop_instance_uid("2.25.94766187067244888884745908966163363746")
            .implementation_version_name("RUSTY_DICOM_269")
            .build()
            .unwrap();

        assert_eq!(
            meta.get(tags::TRANSFER_SYNTAX_UID)
                .unwrap()
                .to_str()
                .unwrap(),
            uids::RLE_LOSSLESS
        );

        let sop_class_uid = meta.get_opt(tags::MEDIA_STORAGE_SOP_CLASS_UID).unwrap();
        let sop_class_uid = sop_class_uid.as_ref().map(|v| v.to_str().unwrap());
        assert_eq!(
            sop_class_uid.as_deref(),
            Some(uids::ENHANCED_MR_IMAGE_STORAGE)
        );

        assert_eq!(
            meta.get_by_name("MediaStorageSOPInstanceUID")
                .unwrap()
                .to_str()
                .unwrap(),
            "2.25.94766187067244888884745908966163363746"
        );

        assert!(meta.get_opt(tags::PRIVATE_INFORMATION).unwrap().is_none());
    }
}
