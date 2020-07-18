//! Module containing data structures and readers of DICOM file meta information tables.
use byteordered::byteorder::{ByteOrder, LittleEndian};
use dicom_core::dicom_value;
use dicom_core::header::{DataElement, EmptyObject, HasLength, Header};
use dicom_core::value::{PrimitiveValue, Value};
use dicom_core::{Length, Tag, VR};
use dicom_encoding::decode::{self, DecodeFrom};
use dicom_encoding::encode::EncoderFor;
use dicom_encoding::text::{self, DefaultCharacterSetCodec, TextCodec};
use dicom_encoding::transfer_syntax::explicit_le::ExplicitVRLittleEndianEncoder;
use dicom_parser::dataset::{DataSetWriter, IntoTokens};
use snafu::{Backtrace, GenerateBacktrace, ResultExt, Snafu};
use std::io::{Read, Write};

const DICM_MAGIC_CODE: [u8; 4] = [b'D', b'I', b'C', b'M'];

#[derive(Debug, Snafu)]
pub enum Error {
    /// The file meta group parser could not read
    /// the magic code `DICM` from its source.
    #[snafu(display("Could not start reading DICOM data: {}", source))]
    ReadMagicCode { source: std::io::Error },

    /// The file meta group parser could not fetch
    /// the value of a data element from its source.
    #[snafu(display("Could not read data value: {}", source))]
    ReadValueData { source: std::io::Error },

    /// The file meta group parser could not decode
    /// the text in one of its data elements.
    #[snafu(display("Could not decode text in {}: {}", name, source))]
    DecodeText {
        name: &'static str,
        source: dicom_encoding::error::Error,
    },

    /// Invalid DICOM data, detected from checking the `DICM` code.
    #[snafu(display("Invalid DICOM data"))]
    NotDicom { backtrace: Backtrace },

    /// An issue occurred while decoding the next data element
    /// in the file meta data set.
    #[snafu(display("Could not decode data element: {}", source))]
    DecodeElement {
        source: dicom_encoding::error::Error,
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
    #[snafu(display("Could not write file meta group data set: {}", source))]
    WriteSet { source: dicom_parser::error::Error },
}

type Result<T> = std::result::Result<T, Error>;

/// DICOM File Meta Information Table.
///
/// This data type contains the relevant parts of the file meta information table, as
/// specified in [1].
///
/// [1]: http://dicom.nema.org/medical/dicom/current/output/chtml/part06/chapter_7.html
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
fn read_str_body<'s, S: 's, T>(
    source: &'s mut S,
    text: &T,
    group_length_remaining: &mut u32,
    len: u32,
) -> Result<String>
where
    S: Read,
    T: TextCodec,
{
    let mut v = vec![0; len as usize];
    source.read_exact(&mut v).context(ReadValueData)?;
    *group_length_remaining -= 8 + len;
    text.decode(&v).context(DecodeText { name: text.name() })
}

impl FileMetaTable {
    pub fn from_reader<R: Read>(file: R) -> Result<Self> {
        FileMetaTable::read_from(file)
    }

    fn read_from<S: Read>(mut file: S) -> Result<Self> {
        let mut buff: [u8; 4] = [0; 4];
        {
            // check magic code
            file.read_exact(&mut buff).context(ReadMagicCode)?;

            if buff != DICM_MAGIC_CODE {
                return Err(Error::NotDicom {
                    backtrace: Backtrace::generate(),
                });
            }
        }

        let decoder = decode::file_header_decoder();
        let text = text::DefaultCharacterSetCodec;

        let builder = FileMetaTableBuilder::new();

        let group_length: u32 = {
            let (elem, _bytes_read) = decoder.decode_header(&mut file).context(DecodeElement)?;
            if elem.tag() != (0x0002, 0x0000) {
                return Err(Error::UnexpectedTag {
                    tag: elem.tag(),
                    backtrace: Backtrace::generate(),
                });
            }
            if elem.length() != Length(4) {
                return Err(Error::UnexpectedDataValueLength {
                    tag: elem.tag(),
                    length: elem.length(),
                    backtrace: Backtrace::generate(),
                });
            }
            let mut buff: [u8; 4] = [0; 4];
            file.read_exact(&mut buff).context(ReadValueData)?;
            LittleEndian::read_u32(&buff)
        };

        let mut group_length_remaining = group_length;

        let mut builder = builder.group_length(group_length);

        // Fetch optional data elements
        while group_length_remaining > 0 {
            let (elem, _bytes_read) = decoder.decode_header(&mut file).context(DecodeElement)?;
            let elem_len = match elem.length().get() {
                None => {
                    return Err(Error::UndefinedValueLength {
                        tag: elem.tag(),
                        backtrace: Backtrace::generate(),
                    });
                }
                Some(len) => len,
            };
            builder = match elem.tag() {
                Tag(0x0002, 0x0001) => {
                    // Implementation Version
                    if elem.length() != Length(2) {
                        return Err(Error::UnexpectedDataValueLength {
                            tag: elem.tag(),
                            length: elem.length(),
                            backtrace: Backtrace::generate(),
                        });
                    }
                    let mut hbuf = [0u8; 2];
                    file.read_exact(&mut hbuf[..]).context(ReadValueData)?;
                    group_length_remaining -= 14;

                    builder.information_version(hbuf)
                }
                // Media Storage SOP Class UID
                Tag(0x0002, 0x0002) => builder.media_storage_sop_class_uid(read_str_body(
                    &mut file,
                    &text,
                    &mut group_length_remaining,
                    elem_len,
                )?),
                // Media Storage SOP Instance UID
                Tag(0x0002, 0x0003) => builder.media_storage_sop_instance_uid(read_str_body(
                    &mut file,
                    &text,
                    &mut group_length_remaining,
                    elem_len,
                )?),
                // Transfer Syntax
                Tag(0x0002, 0x0010) => builder.transfer_syntax(read_str_body(
                    &mut file,
                    &text,
                    &mut group_length_remaining,
                    elem_len,
                )?),
                // Implementation Class UID
                Tag(0x0002, 0x0012) => builder.implementation_class_uid(read_str_body(
                    &mut file,
                    &text,
                    &mut group_length_remaining,
                    elem_len,
                )?),
                Tag(0x0002, 0x0013) => {
                    // Implementation Version Name
                    let mut v = vec![0; elem_len as usize];
                    file.read_exact(&mut v).context(ReadValueData)?;
                    group_length_remaining -= 8 + elem_len;
                    builder.implementation_version_name(
                        text.decode(&v).context(DecodeText { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0016) => {
                    // Source Application Entity Title
                    let mut v = vec![0; elem_len as usize];
                    file.read_exact(&mut v).context(ReadValueData)?;
                    group_length_remaining -= 8 + elem_len;
                    builder.source_application_entity_title(
                        text.decode(&v).context(DecodeText { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0017) => {
                    // Sending Application Entity Title
                    let mut v = vec![0; elem_len as usize];
                    file.read_exact(&mut v).context(ReadValueData)?;
                    group_length_remaining -= 8 + elem_len;
                    builder.sending_application_entity_title(
                        text.decode(&v).context(DecodeText { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0018) => {
                    // Receiving Application Entity Title
                    let mut v = vec![0; elem_len as usize];
                    file.read_exact(&mut v).context(ReadValueData)?;
                    group_length_remaining -= 8 + elem_len;
                    builder.receiving_application_entity_title(
                        text.decode(&v).context(DecodeText { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0100) => {
                    // Private Information Creator UID
                    let mut v = vec![0; elem_len as usize];
                    file.read_exact(&mut v).context(ReadValueData)?;
                    group_length_remaining -= 8 + elem_len;
                    builder.private_information_creator_uid(
                        text.decode(&v).context(DecodeText { name: text.name() })?,
                    )
                }
                Tag(0x0002, 0x0102) => {
                    // Private Information
                    let mut v = vec![0; elem_len as usize];
                    file.read_exact(&mut v).context(ReadValueData)?;
                    group_length_remaining -= 12 + elem_len;
                    builder.private_information(v)
                }
                Tag(0x0002, _) => {
                    // unknown tag, do nothing
                    // could be an unsupported or non-standard attribute,
                    // consider logging (#49)
                    builder
                }
                _ => {
                    // unexpected tag from another group! do nothing for now,
                    // but this could pose an issue up ahead (see #50)
                    // and should be logged (#49)
                    builder
                }
            }
        }

        builder.build()
    }

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

    pub fn write<W: Write>(&self, writer: W) -> Result<()> {
        let mut dset = DataSetWriter::new(
            writer,
            EncoderFor::new(ExplicitVRLittleEndianEncoder::default()),
            DefaultCharacterSetCodec,
        );
        dset.write_sequence(
            self.clone()
                .into_element_iter()
                .flat_map(IntoTokens::into_tokens),
        )
        .context(WriteSet)
    }
}

/// A builder for DICOM meta information tables.
#[derive(Debug, Clone)]
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

impl Default for FileMetaTableBuilder {
    fn default() -> FileMetaTableBuilder {
        FileMetaTableBuilder {
            information_group_length: None,
            information_version: None,
            media_storage_sop_class_uid: None,
            media_storage_sop_instance_uid: None,
            transfer_syntax: None,
            implementation_class_uid: None,
            implementation_version_name: None,
            source_application_entity_title: None,
            sending_application_entity_title: None,
            receiving_application_entity_title: None,
            private_information_creator_uid: None,
            private_information: None,
        }
    }
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
        let information_version = self.information_version.unwrap_or_else(|| {
            // Missing information version, will assume (00H, 01H). See #28
            [0, 1]
        });
        let media_storage_sop_class_uid =
            self.media_storage_sop_class_uid
                .ok_or_else(|| Error::MissingElement {
                    alias: "MediaStorageSOPClassUID",
                    backtrace: Backtrace::generate(),
                })?;
        let media_storage_sop_instance_uid =
            self.media_storage_sop_instance_uid
                .ok_or_else(|| Error::MissingElement {
                    alias: "MediaStorageSOPInstanceUID",
                    backtrace: Backtrace::generate(),
                })?;
        let transfer_syntax = self.transfer_syntax.ok_or_else(|| Error::MissingElement {
            alias: "TransferSyntax",
            backtrace: Backtrace::generate(),
        })?;
        let implementation_class_uid =
            self.implementation_class_uid
                .ok_or_else(|| Error::MissingElement {
                    alias: "ImplementationClassUID",
                    backtrace: Backtrace::generate(),
                })?;

        fn dicom_len<T: AsRef<str>>(x: T) -> u32 {
            let o = x.as_ref().len() as u32;
            if o % 2 == 1 {
                o + 1
            } else {
                o
            }
        }

        let information_group_length = match self.information_group_length {
            Some(e) => e,
            None => {
                // determine the expected meta group size based on the given fields.
                // FileMetaInformationGroupLength is not included here

                14 + 8
                    + dicom_len(&media_storage_sop_class_uid)
                    + 8
                    + dicom_len(&media_storage_sop_instance_uid)
                    + 8
                    + dicom_len(&transfer_syntax)
                    + 8
                    + dicom_len(&implementation_class_uid)
                    + self
                        .implementation_version_name
                        .as_ref()
                        .map(|s| 8 + s.len() as u32)
                        .unwrap_or(0)
                    + self
                        .source_application_entity_title
                        .as_ref()
                        .map(|s| 8 + s.len() as u32)
                        .unwrap_or(0)
                    + self
                        .sending_application_entity_title
                        .as_ref()
                        .map(|s| 8 + s.len() as u32)
                        .unwrap_or(0)
                    + self
                        .receiving_application_entity_title
                        .as_ref()
                        .map(|s| 8 + s.len() as u32)
                        .unwrap_or(0)
                    + self
                        .private_information_creator_uid
                        .as_ref()
                        .map(|s| 8 + s.len() as u32)
                        .unwrap_or(0)
                    + self
                        .private_information
                        .as_ref()
                        .map(|x| 12 + x.len() as u32)
                        .unwrap_or(0)
            }
        };

        Ok(FileMetaTable {
            information_group_length,
            information_version,
            media_storage_sop_class_uid,
            media_storage_sop_instance_uid,
            transfer_syntax,
            implementation_class_uid,
            implementation_version_name: self.implementation_version_name,
            source_application_entity_title: self.source_application_entity_title,
            sending_application_entity_title: self.sending_application_entity_title,
            receiving_application_entity_title: self.receiving_application_entity_title,
            private_information_creator_uid: self.private_information_creator_uid,
            private_information: self.private_information,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FileMetaTable, FileMetaTableBuilder};
    use dicom_core::value::Value;
    use dicom_core::{dicom_value, DataElement, Tag, VR};

    const TEST_META_1: &'static [u8] = &[
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

        let gt = vec![
            // Information Group Length
            DataElement::new(Tag(0x0002, 0x0000), VR::UL, dicom_value!(U32, 200).into()),
            // Information Version
            DataElement::new(Tag(0x0002, 0x0001), VR::OB, dicom_value!(U8, [0, 1]).into()),
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
}
