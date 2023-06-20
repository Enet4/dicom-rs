//! UID declarations
// Automatically generated. Edit at your own risk.

use dicom_core::dictionary::UidDictionaryEntryRef;
#![allow(deprecated)]
/// SOP Class: Verification SOP Class
#[rustfmt::skip]
pub const VERIFICATION: &str = "1.2.840.10008.1.1";
/// Transfer Syntax: Implicit VR Little Endian: Default Transfer Syntax for DICOM
#[rustfmt::skip]
pub const IMPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2";
/// Transfer Syntax: Explicit VR Little Endian
#[rustfmt::skip]
pub const EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1";
/// Transfer Syntax: Encapsulated Uncompressed Explicit VR Little Endian
#[rustfmt::skip]
pub const ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1.98";
/// Transfer Syntax: Deflated Explicit VR Little Endian
#[rustfmt::skip]
pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1.99";
/// Transfer Syntax: Explicit VR Big Endian (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const EXPLICIT_VR_BIG_ENDIAN: &str = "1.2.840.10008.1.2.2";
/// Transfer Syntax: MPEG2 Main Profile / Main Level
#[rustfmt::skip]
pub const MPEG2MPML: &str = "1.2.840.10008.1.2.4.100";
/// Transfer Syntax: Fragmentable MPEG2 Main Profile / Main Level
#[rustfmt::skip]
pub const MPEG2MPMLF: &str = "1.2.840.10008.1.2.4.100.1";
/// Transfer Syntax: MPEG2 Main Profile / High Level
#[rustfmt::skip]
pub const MPEG2MPHL: &str = "1.2.840.10008.1.2.4.101";
/// Transfer Syntax: Fragmentable MPEG2 Main Profile / High Level
#[rustfmt::skip]
pub const MPEG2MPHLF: &str = "1.2.840.10008.1.2.4.101.1";
/// Transfer Syntax: MPEG-4 AVC/H.264 High Profile / Level 4.1
#[rustfmt::skip]
pub const MPEG4HP41: &str = "1.2.840.10008.1.2.4.102";
/// Transfer Syntax: Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.1
#[rustfmt::skip]
pub const MPEG4HP41F: &str = "1.2.840.10008.1.2.4.102.1";
/// Transfer Syntax: MPEG-4 AVC/H.264 BD-compatible High Profile / Level 4.1
#[rustfmt::skip]
pub const MPEG4HP41BD: &str = "1.2.840.10008.1.2.4.103";
/// Transfer Syntax: Fragmentable MPEG-4 AVC/H.264 BD-compatible High Profile / Level 4.1
#[rustfmt::skip]
pub const MPEG4HP41BDF: &str = "1.2.840.10008.1.2.4.103.1";
/// Transfer Syntax: MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video
#[rustfmt::skip]
pub const MPEG4HP422D: &str = "1.2.840.10008.1.2.4.104";
/// Transfer Syntax: Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video
#[rustfmt::skip]
pub const MPEG4HP422DF: &str = "1.2.840.10008.1.2.4.104.1";
/// Transfer Syntax: MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video
#[rustfmt::skip]
pub const MPEG4HP423D: &str = "1.2.840.10008.1.2.4.105";
/// Transfer Syntax: Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video
#[rustfmt::skip]
pub const MPEG4HP423DF: &str = "1.2.840.10008.1.2.4.105.1";
/// Transfer Syntax: MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2
#[rustfmt::skip]
pub const MPEG4HP42STEREO: &str = "1.2.840.10008.1.2.4.106";
/// Transfer Syntax: Fragmentable MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2
#[rustfmt::skip]
pub const MPEG4HP42STEREOF: &str = "1.2.840.10008.1.2.4.106.1";
/// Transfer Syntax: HEVC/H.265 Main Profile / Level 5.1
#[rustfmt::skip]
pub const HEVCMP51: &str = "1.2.840.10008.1.2.4.107";
/// Transfer Syntax: HEVC/H.265 Main 10 Profile / Level 5.1
#[rustfmt::skip]
pub const HEVCM10P51: &str = "1.2.840.10008.1.2.4.108";
/// Transfer Syntax: JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression
#[rustfmt::skip]
pub const JPEG_BASELINE8_BIT: &str = "1.2.840.10008.1.2.4.50";
/// Transfer Syntax: JPEG Extended (Process 2
#[rustfmt::skip]
pub const JPEG_EXTENDED12_BIT: &str = "1.2.840.10008.1.2.4.51";
/// Transfer Syntax: JPEG Extended (Process 3
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_EXTENDED35: &str = "1.2.840.10008.1.2.4.52";
/// Transfer Syntax: JPEG Spectral Selection, Non-Hierarchical (Process 6
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_SPECTRAL_SELECTION_NON_HIERARCHICAL68: &str = "1.2.840.10008.1.2.4.53";
/// Transfer Syntax: JPEG Spectral Selection, Non-Hierarchical (Process 7
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_SPECTRAL_SELECTION_NON_HIERARCHICAL79: &str = "1.2.840.10008.1.2.4.54";
/// Transfer Syntax: JPEG Full Progression, Non-Hierarchical (Process 10
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_FULL_PROGRESSION_NON_HIERARCHICAL1012: &str = "1.2.840.10008.1.2.4.55";
/// Transfer Syntax: JPEG Full Progression, Non-Hierarchical (Process 11
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_FULL_PROGRESSION_NON_HIERARCHICAL1113: &str = "1.2.840.10008.1.2.4.56";
/// Transfer Syntax: JPEG Lossless, Non-Hierarchical (Process 14)
#[rustfmt::skip]
pub const JPEG_LOSSLESS: &str = "1.2.840.10008.1.2.4.57";
/// Transfer Syntax: JPEG Lossless, Non-Hierarchical (Process 15) (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_LOSSLESS_NON_HIERARCHICAL15: &str = "1.2.840.10008.1.2.4.58";
/// Transfer Syntax: JPEG Extended, Hierarchical (Process 16
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_EXTENDED_HIERARCHICAL1618: &str = "1.2.840.10008.1.2.4.59";
/// Transfer Syntax: JPEG Extended, Hierarchical (Process 17
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_EXTENDED_HIERARCHICAL1719: &str = "1.2.840.10008.1.2.4.60";
/// Transfer Syntax: JPEG Spectral Selection, Hierarchical (Process 20
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_SPECTRAL_SELECTION_HIERARCHICAL2022: &str = "1.2.840.10008.1.2.4.61";
/// Transfer Syntax: JPEG Spectral Selection, Hierarchical (Process 21
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_SPECTRAL_SELECTION_HIERARCHICAL2123: &str = "1.2.840.10008.1.2.4.62";
/// Transfer Syntax: JPEG Full Progression, Hierarchical (Process 24
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_FULL_PROGRESSION_HIERARCHICAL2426: &str = "1.2.840.10008.1.2.4.63";
/// Transfer Syntax: JPEG Full Progression, Hierarchical (Process 25
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_FULL_PROGRESSION_HIERARCHICAL2527: &str = "1.2.840.10008.1.2.4.64";
/// Transfer Syntax: JPEG Lossless, Hierarchical (Process 28) (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_LOSSLESS_HIERARCHICAL28: &str = "1.2.840.10008.1.2.4.65";
/// Transfer Syntax: JPEG Lossless, Hierarchical (Process 29) (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const JPEG_LOSSLESS_HIERARCHICAL29: &str = "1.2.840.10008.1.2.4.66";
/// Transfer Syntax: JPEG Lossless, Non-Hierarchical, First-Order Prediction (Process 14 [Selection Value 1]): Default Transfer Syntax for Lossless JPEG Image Compression
#[rustfmt::skip]
pub const JPEG_LOSSLESS_SV1: &str = "1.2.840.10008.1.2.4.70";
/// Transfer Syntax: JPEG-LS Lossless Image Compression
#[rustfmt::skip]
pub const JPEGLS_LOSSLESS: &str = "1.2.840.10008.1.2.4.80";
/// Transfer Syntax: JPEG-LS Lossy (Near-Lossless) Image Compression
#[rustfmt::skip]
pub const JPEGLS_NEAR_LOSSLESS: &str = "1.2.840.10008.1.2.4.81";
/// Transfer Syntax: JPEG 2000 Image Compression (Lossless Only)
#[rustfmt::skip]
pub const JPEG2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
/// Transfer Syntax: JPEG 2000 Image Compression
#[rustfmt::skip]
pub const JPEG2000: &str = "1.2.840.10008.1.2.4.91";
/// Transfer Syntax: JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)
#[rustfmt::skip]
pub const JPEG2000MC_LOSSLESS: &str = "1.2.840.10008.1.2.4.92";
/// Transfer Syntax: JPEG 2000 Part 2 Multi-component Image Compression
#[rustfmt::skip]
pub const JPEG2000MC: &str = "1.2.840.10008.1.2.4.93";
/// Transfer Syntax: JPIP Referenced
#[rustfmt::skip]
pub const JPIP_REFERENCED: &str = "1.2.840.10008.1.2.4.94";
/// Transfer Syntax: JPIP Referenced Deflate
#[rustfmt::skip]
pub const JPIP_REFERENCED_DEFLATE: &str = "1.2.840.10008.1.2.4.95";
/// Transfer Syntax: RLE Lossless
#[rustfmt::skip]
pub const RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
/// Transfer Syntax: RFC 2557 MIME encapsulation (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const RFC2557MIME_ENCAPSULATION: &str = "1.2.840.10008.1.2.6.1";
/// Transfer Syntax: XML Encoding (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const XML_ENCODING: &str = "1.2.840.10008.1.2.6.2";
/// Transfer Syntax: SMPTE ST 2110-20 Uncompressed Progressive Active Video
#[rustfmt::skip]
pub const SMPTEST211020_UNCOMPRESSED_PROGRESSIVE_ACTIVE_VIDEO: &str = "1.2.840.10008.1.2.7.1";
/// Transfer Syntax: SMPTE ST 2110-20 Uncompressed Interlaced Active Video
#[rustfmt::skip]
pub const SMPTEST211020_UNCOMPRESSED_INTERLACED_ACTIVE_VIDEO: &str = "1.2.840.10008.1.2.7.2";
/// Transfer Syntax: SMPTE ST 2110-30 PCM Digital Audio
#[rustfmt::skip]
pub const SMPTEST211030PCM_DIGITAL_AUDIO: &str = "1.2.840.10008.1.2.7.3";
/// Transfer Syntax: Papyrus 3 Implicit VR Little Endian (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PAPYRUS3_IMPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.20";
/// SOP Class: Storage Commitment Push Model SOP Class
#[rustfmt::skip]
pub const STORAGE_COMMITMENT_PUSH_MODEL: &str = "1.2.840.10008.1.20.1";
/// Well-known SOP Instance: Storage Commitment Push Model SOP Instance
#[rustfmt::skip]
pub const STORAGE_COMMITMENT_PUSH_MODEL_INSTANCE: &str = "1.2.840.10008.1.20.1.1";
/// SOP Class: Storage Commitment Pull Model SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STORAGE_COMMITMENT_PULL_MODEL: &str = "1.2.840.10008.1.20.2";
/// Well-known SOP Instance: Storage Commitment Pull Model SOP Instance (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STORAGE_COMMITMENT_PULL_MODEL_INSTANCE: &str = "1.2.840.10008.1.20.2.1";
/// SOP Class: Media Storage Directory Storage
#[rustfmt::skip]
pub const MEDIA_STORAGE_DIRECTORY_STORAGE: &str = "1.2.840.10008.1.3.10";
/// SOP Class: Procedural Event Logging SOP Class
#[rustfmt::skip]
pub const PROCEDURAL_EVENT_LOGGING: &str = "1.2.840.10008.1.40";
/// Well-known SOP Instance: Procedural Event Logging SOP Instance
#[rustfmt::skip]
pub const PROCEDURAL_EVENT_LOGGING_INSTANCE: &str = "1.2.840.10008.1.40.1";
/// SOP Class: Substance Administration Logging SOP Class
#[rustfmt::skip]
pub const SUBSTANCE_ADMINISTRATION_LOGGING: &str = "1.2.840.10008.1.42";
/// Well-known SOP Instance: Substance Administration Logging SOP Instance
#[rustfmt::skip]
pub const SUBSTANCE_ADMINISTRATION_LOGGING_INSTANCE: &str = "1.2.840.10008.1.42.1";
/// Well-known SOP Instance: Hot Iron Color Palette SOP Instance
#[rustfmt::skip]
pub const HOT_IRON_PALETTE: &str = "1.2.840.10008.1.5.1";
/// Well-known SOP Instance: PET Color Palette SOP Instance
#[rustfmt::skip]
pub const PET_PALETTE: &str = "1.2.840.10008.1.5.2";
/// Well-known SOP Instance: Hot Metal Blue Color Palette SOP Instance
#[rustfmt::skip]
pub const HOT_METAL_BLUE_PALETTE: &str = "1.2.840.10008.1.5.3";
/// Well-known SOP Instance: PET 20 Step Color Palette SOP Instance
#[rustfmt::skip]
pub const PET20_STEP_PALETTE: &str = "1.2.840.10008.1.5.4";
/// Well-known SOP Instance: Spring Color Palette SOP Instance
#[rustfmt::skip]
pub const SPRING_PALETTE: &str = "1.2.840.10008.1.5.5";
/// Well-known SOP Instance: Summer Color Palette SOP Instance
#[rustfmt::skip]
pub const SUMMER_PALETTE: &str = "1.2.840.10008.1.5.6";
/// Well-known SOP Instance: Fall Color Palette SOP Instance
#[rustfmt::skip]
pub const FALL_PALETTE: &str = "1.2.840.10008.1.5.7";
/// Well-known SOP Instance: Winter Color Palette SOP Instance
#[rustfmt::skip]
pub const WINTER_PALETTE: &str = "1.2.840.10008.1.5.8";
/// SOP Class: Basic Study Content Notification SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const BASIC_STUDY_CONTENT_NOTIFICATION: &str = "1.2.840.10008.1.9";
/// SOP Class: Video Endoscopic Image Real-Time Communication
#[rustfmt::skip]
pub const VIDEO_ENDOSCOPIC_IMAGE_REAL_TIME_COMMUNICATION: &str = "1.2.840.10008.10.1";
/// SOP Class: Video Photographic Image Real-Time Communication
#[rustfmt::skip]
pub const VIDEO_PHOTOGRAPHIC_IMAGE_REAL_TIME_COMMUNICATION: &str = "1.2.840.10008.10.2";
/// SOP Class: Audio Waveform Real-Time Communication
#[rustfmt::skip]
pub const AUDIO_WAVEFORM_REAL_TIME_COMMUNICATION: &str = "1.2.840.10008.10.3";
/// SOP Class: Rendition Selection Document Real-Time Communication
#[rustfmt::skip]
pub const RENDITION_SELECTION_DOCUMENT_REAL_TIME_COMMUNICATION: &str = "1.2.840.10008.10.4";
/// LDAP OID: dicomDeviceName
#[rustfmt::skip]
pub const DICOM_DEVICE_NAME: &str = "1.2.840.10008.15.0.3.1";
/// LDAP OID: dicomAssociationInitiator
#[rustfmt::skip]
pub const DICOM_ASSOCIATION_INITIATOR: &str = "1.2.840.10008.15.0.3.10";
/// LDAP OID: dicomAssociationAcceptor
#[rustfmt::skip]
pub const DICOM_ASSOCIATION_ACCEPTOR: &str = "1.2.840.10008.15.0.3.11";
/// LDAP OID: dicomHostname
#[rustfmt::skip]
pub const DICOM_HOSTNAME: &str = "1.2.840.10008.15.0.3.12";
/// LDAP OID: dicomPort
#[rustfmt::skip]
pub const DICOM_PORT: &str = "1.2.840.10008.15.0.3.13";
/// LDAP OID: dicomSOPClass
#[rustfmt::skip]
pub const DICOM_SOP_CLASS: &str = "1.2.840.10008.15.0.3.14";
/// LDAP OID: dicomTransferRole
#[rustfmt::skip]
pub const DICOM_TRANSFER_ROLE: &str = "1.2.840.10008.15.0.3.15";
/// LDAP OID: dicomTransferSyntax
#[rustfmt::skip]
pub const DICOM_TRANSFER_SYNTAX: &str = "1.2.840.10008.15.0.3.16";
/// LDAP OID: dicomPrimaryDeviceType
#[rustfmt::skip]
pub const DICOM_PRIMARY_DEVICE_TYPE: &str = "1.2.840.10008.15.0.3.17";
/// LDAP OID: dicomRelatedDeviceReference
#[rustfmt::skip]
pub const DICOM_RELATED_DEVICE_REFERENCE: &str = "1.2.840.10008.15.0.3.18";
/// LDAP OID: dicomPreferredCalledAETitle
#[rustfmt::skip]
pub const DICOM_PREFERRED_CALLED_AE_TITLE: &str = "1.2.840.10008.15.0.3.19";
/// LDAP OID: dicomDescription
#[rustfmt::skip]
pub const DICOM_DESCRIPTION: &str = "1.2.840.10008.15.0.3.2";
/// LDAP OID: dicomTLSCyphersuite
#[rustfmt::skip]
pub const DICOM_TLS_CYPHERSUITE: &str = "1.2.840.10008.15.0.3.20";
/// LDAP OID: dicomAuthorizedNodeCertificateReference
#[rustfmt::skip]
pub const DICOM_AUTHORIZED_NODE_CERTIFICATE_REFERENCE: &str = "1.2.840.10008.15.0.3.21";
/// LDAP OID: dicomThisNodeCertificateReference
#[rustfmt::skip]
pub const DICOM_THIS_NODE_CERTIFICATE_REFERENCE: &str = "1.2.840.10008.15.0.3.22";
/// LDAP OID: dicomInstalled
#[rustfmt::skip]
pub const DICOM_INSTALLED: &str = "1.2.840.10008.15.0.3.23";
/// LDAP OID: dicomStationName
#[rustfmt::skip]
pub const DICOM_STATION_NAME: &str = "1.2.840.10008.15.0.3.24";
/// LDAP OID: dicomDeviceSerialNumber
#[rustfmt::skip]
pub const DICOM_DEVICE_SERIAL_NUMBER: &str = "1.2.840.10008.15.0.3.25";
/// LDAP OID: dicomInstitutionName
#[rustfmt::skip]
pub const DICOM_INSTITUTION_NAME: &str = "1.2.840.10008.15.0.3.26";
/// LDAP OID: dicomInstitutionAddress
#[rustfmt::skip]
pub const DICOM_INSTITUTION_ADDRESS: &str = "1.2.840.10008.15.0.3.27";
/// LDAP OID: dicomInstitutionDepartmentName
#[rustfmt::skip]
pub const DICOM_INSTITUTION_DEPARTMENT_NAME: &str = "1.2.840.10008.15.0.3.28";
/// LDAP OID: dicomIssuerOfPatientID
#[rustfmt::skip]
pub const DICOM_ISSUER_OF_PATIENT_ID: &str = "1.2.840.10008.15.0.3.29";
/// LDAP OID: dicomManufacturer
#[rustfmt::skip]
pub const DICOM_MANUFACTURER: &str = "1.2.840.10008.15.0.3.3";
/// LDAP OID: dicomPreferredCallingAETitle
#[rustfmt::skip]
pub const DICOM_PREFERRED_CALLING_AE_TITLE: &str = "1.2.840.10008.15.0.3.30";
/// LDAP OID: dicomSupportedCharacterSet
#[rustfmt::skip]
pub const DICOM_SUPPORTED_CHARACTER_SET: &str = "1.2.840.10008.15.0.3.31";
/// LDAP OID: dicomManufacturerModelName
#[rustfmt::skip]
pub const DICOM_MANUFACTURER_MODEL_NAME: &str = "1.2.840.10008.15.0.3.4";
/// LDAP OID: dicomSoftwareVersion
#[rustfmt::skip]
pub const DICOM_SOFTWARE_VERSION: &str = "1.2.840.10008.15.0.3.5";
/// LDAP OID: dicomVendorData
#[rustfmt::skip]
pub const DICOM_VENDOR_DATA: &str = "1.2.840.10008.15.0.3.6";
/// LDAP OID: dicomAETitle
#[rustfmt::skip]
pub const DICOM_AE_TITLE: &str = "1.2.840.10008.15.0.3.7";
/// LDAP OID: dicomNetworkConnectionReference
#[rustfmt::skip]
pub const DICOM_NETWORK_CONNECTION_REFERENCE: &str = "1.2.840.10008.15.0.3.8";
/// LDAP OID: dicomApplicationCluster
#[rustfmt::skip]
pub const DICOM_APPLICATION_CLUSTER: &str = "1.2.840.10008.15.0.3.9";
/// LDAP OID: dicomConfigurationRoot
#[rustfmt::skip]
pub const DICOM_CONFIGURATION_ROOT: &str = "1.2.840.10008.15.0.4.1";
/// LDAP OID: dicomDevicesRoot
#[rustfmt::skip]
pub const DICOM_DEVICES_ROOT: &str = "1.2.840.10008.15.0.4.2";
/// LDAP OID: dicomUniqueAETitlesRegistryRoot
#[rustfmt::skip]
pub const DICOM_UNIQUE_AE_TITLES_REGISTRY_ROOT: &str = "1.2.840.10008.15.0.4.3";
/// LDAP OID: dicomDevice
#[rustfmt::skip]
pub const DICOM_DEVICE: &str = "1.2.840.10008.15.0.4.4";
/// LDAP OID: dicomNetworkAE
#[rustfmt::skip]
pub const DICOM_NETWORK_AE: &str = "1.2.840.10008.15.0.4.5";
/// LDAP OID: dicomNetworkConnection
#[rustfmt::skip]
pub const DICOM_NETWORK_CONNECTION: &str = "1.2.840.10008.15.0.4.6";
/// LDAP OID: dicomUniqueAETitle
#[rustfmt::skip]
pub const DICOM_UNIQUE_AE_TITLE: &str = "1.2.840.10008.15.0.4.7";
/// LDAP OID: dicomTransferCapability
#[rustfmt::skip]
pub const DICOM_TRANSFER_CAPABILITY: &str = "1.2.840.10008.15.0.4.8";
/// Synchronization Frame of Reference: Universal Coordinated Time
#[rustfmt::skip]
pub const UTC: &str = "1.2.840.10008.15.1.1";
/// Coding Scheme: Dublin Core
#[rustfmt::skip]
pub const DC: &str = "1.2.840.10008.2.16.10";
/// Coding Scheme: New York University Melanoma Clinical Cooperative Group
#[rustfmt::skip]
pub const NYUMCCG: &str = "1.2.840.10008.2.16.11";
/// Coding Scheme: Mayo Clinic Non-radiological Images Specific Body Structure Anatomical Surface Region Guide
#[rustfmt::skip]
pub const MAYONRISBSASRG: &str = "1.2.840.10008.2.16.12";
/// Coding Scheme: Image Biomarker Standardisation Initiative
#[rustfmt::skip]
pub const IBSI: &str = "1.2.840.10008.2.16.13";
/// Coding Scheme: Radiomics Ontology
#[rustfmt::skip]
pub const RO: &str = "1.2.840.10008.2.16.14";
/// Coding Scheme: RadElement
#[rustfmt::skip]
pub const RADELEMENT: &str = "1.2.840.10008.2.16.15";
/// Coding Scheme: ICD-11
#[rustfmt::skip]
pub const I11: &str = "1.2.840.10008.2.16.16";
/// Coding Scheme: Unified numbering system (UNS) for metals and alloys
#[rustfmt::skip]
pub const UNS: &str = "1.2.840.10008.2.16.17";
/// Coding Scheme: Research Resource Identification
#[rustfmt::skip]
pub const RRID: &str = "1.2.840.10008.2.16.18";
/// Coding Scheme: DICOM Controlled Terminology
#[rustfmt::skip]
pub const DCM: &str = "1.2.840.10008.2.16.4";
/// Coding Scheme: Adult Mouse Anatomy Ontology
#[rustfmt::skip]
pub const MA: &str = "1.2.840.10008.2.16.5";
/// Coding Scheme: Uberon Ontology
#[rustfmt::skip]
pub const UBERON: &str = "1.2.840.10008.2.16.6";
/// Coding Scheme: Integrated Taxonomic Information System (ITIS) Taxonomic Serial Number (TSN)
#[rustfmt::skip]
pub const ITIS_TSN: &str = "1.2.840.10008.2.16.7";
/// Coding Scheme: Mouse Genome Initiative (MGI)
#[rustfmt::skip]
pub const MGI: &str = "1.2.840.10008.2.16.8";
/// Coding Scheme: PubChem Compound CID
#[rustfmt::skip]
pub const PUBCHEM_CID: &str = "1.2.840.10008.2.16.9";
/// DICOM UIDs as a Coding Scheme: DICOM UID Registry
#[rustfmt::skip]
pub const DCMUID: &str = "1.2.840.10008.2.6.1";
/// Application Context Name: DICOM Application Context Name
#[rustfmt::skip]
pub const DICOM_APPLICATION_CONTEXT: &str = "1.2.840.10008.3.1.1.1";
/// SOP Class: Detached Patient Management SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_PATIENT_MANAGEMENT: &str = "1.2.840.10008.3.1.2.1.1";
/// Meta SOP Class: Detached Patient Management Meta SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_PATIENT_MANAGEMENT_META: &str = "1.2.840.10008.3.1.2.1.4";
/// SOP Class: Detached Visit Management SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_VISIT_MANAGEMENT: &str = "1.2.840.10008.3.1.2.2.1";
/// SOP Class: Detached Study Management SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_STUDY_MANAGEMENT: &str = "1.2.840.10008.3.1.2.3.1";
/// SOP Class: Study Component Management SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STUDY_COMPONENT_MANAGEMENT: &str = "1.2.840.10008.3.1.2.3.2";
/// SOP Class: Modality Performed Procedure Step SOP Class
#[rustfmt::skip]
pub const MODALITY_PERFORMED_PROCEDURE_STEP: &str = "1.2.840.10008.3.1.2.3.3";
/// SOP Class: Modality Performed Procedure Step Retrieve SOP Class
#[rustfmt::skip]
pub const MODALITY_PERFORMED_PROCEDURE_STEP_RETRIEVE: &str = "1.2.840.10008.3.1.2.3.4";
/// SOP Class: Modality Performed Procedure Step Notification SOP Class
#[rustfmt::skip]
pub const MODALITY_PERFORMED_PROCEDURE_STEP_NOTIFICATION: &str = "1.2.840.10008.3.1.2.3.5";
/// SOP Class: Detached Results Management SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_RESULTS_MANAGEMENT: &str = "1.2.840.10008.3.1.2.5.1";
/// Meta SOP Class: Detached Results Management Meta SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_RESULTS_MANAGEMENT_META: &str = "1.2.840.10008.3.1.2.5.4";
/// Meta SOP Class: Detached Study Management Meta SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_STUDY_MANAGEMENT_META: &str = "1.2.840.10008.3.1.2.5.5";
/// SOP Class: Detached Interpretation Management SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETACHED_INTERPRETATION_MANAGEMENT: &str = "1.2.840.10008.3.1.2.6.1";
/// Service Class: Storage Service Class
#[rustfmt::skip]
pub const STORAGE: &str = "1.2.840.10008.4.2";
/// SOP Class: Basic Film Session SOP Class
#[rustfmt::skip]
pub const BASIC_FILM_SESSION: &str = "1.2.840.10008.5.1.1.1";
/// SOP Class: Print Job SOP Class
#[rustfmt::skip]
pub const PRINT_JOB: &str = "1.2.840.10008.5.1.1.14";
/// SOP Class: Basic Annotation Box SOP Class
#[rustfmt::skip]
pub const BASIC_ANNOTATION_BOX: &str = "1.2.840.10008.5.1.1.15";
/// SOP Class: Printer SOP Class
#[rustfmt::skip]
pub const PRINTER: &str = "1.2.840.10008.5.1.1.16";
/// SOP Class: Printer Configuration Retrieval SOP Class
#[rustfmt::skip]
pub const PRINTER_CONFIGURATION_RETRIEVAL: &str = "1.2.840.10008.5.1.1.16.376";
/// Well-known SOP Instance: Printer SOP Instance
#[rustfmt::skip]
pub const PRINTER_INSTANCE: &str = "1.2.840.10008.5.1.1.17";
/// Well-known SOP Instance: Printer Configuration Retrieval SOP Instance
#[rustfmt::skip]
pub const PRINTER_CONFIGURATION_RETRIEVAL_INSTANCE: &str = "1.2.840.10008.5.1.1.17.376";
/// Meta SOP Class: Basic Color Print Management Meta SOP Class
#[rustfmt::skip]
pub const BASIC_COLOR_PRINT_MANAGEMENT_META: &str = "1.2.840.10008.5.1.1.18";
/// Meta SOP Class: Referenced Color Print Management Meta SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const REFERENCED_COLOR_PRINT_MANAGEMENT_META: &str = "1.2.840.10008.5.1.1.18.1";
/// SOP Class: Basic Film Box SOP Class
#[rustfmt::skip]
pub const BASIC_FILM_BOX: &str = "1.2.840.10008.5.1.1.2";
/// SOP Class: VOI LUT Box SOP Class
#[rustfmt::skip]
pub const VOILUT_BOX: &str = "1.2.840.10008.5.1.1.22";
/// SOP Class: Presentation LUT SOP Class
#[rustfmt::skip]
pub const PRESENTATION_LUT: &str = "1.2.840.10008.5.1.1.23";
/// SOP Class: Image Overlay Box SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const IMAGE_OVERLAY_BOX: &str = "1.2.840.10008.5.1.1.24";
/// SOP Class: Basic Print Image Overlay Box SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const BASIC_PRINT_IMAGE_OVERLAY_BOX: &str = "1.2.840.10008.5.1.1.24.1";
/// Well-known SOP Instance: Print Queue SOP Instance (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PRINT_QUEUE_INSTANCE: &str = "1.2.840.10008.5.1.1.25";
/// SOP Class: Print Queue Management SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PRINT_QUEUE_MANAGEMENT: &str = "1.2.840.10008.5.1.1.26";
/// SOP Class: Stored Print Storage SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STORED_PRINT_STORAGE: &str = "1.2.840.10008.5.1.1.27";
/// SOP Class: Hardcopy Grayscale Image Storage SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const HARDCOPY_GRAYSCALE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.1.29";
/// SOP Class: Hardcopy Color Image Storage SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const HARDCOPY_COLOR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.1.30";
/// SOP Class: Pull Print Request SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PULL_PRINT_REQUEST: &str = "1.2.840.10008.5.1.1.31";
/// Meta SOP Class: Pull Stored Print Management Meta SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PULL_STORED_PRINT_MANAGEMENT_META: &str = "1.2.840.10008.5.1.1.32";
/// SOP Class: Media Creation Management SOP Class UID
#[rustfmt::skip]
pub const MEDIA_CREATION_MANAGEMENT: &str = "1.2.840.10008.5.1.1.33";
/// SOP Class: Basic Grayscale Image Box SOP Class
#[rustfmt::skip]
pub const BASIC_GRAYSCALE_IMAGE_BOX: &str = "1.2.840.10008.5.1.1.4";
/// SOP Class: Basic Color Image Box SOP Class
#[rustfmt::skip]
pub const BASIC_COLOR_IMAGE_BOX: &str = "1.2.840.10008.5.1.1.4.1";
/// SOP Class: Referenced Image Box SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const REFERENCED_IMAGE_BOX: &str = "1.2.840.10008.5.1.1.4.2";
/// SOP Class: Display System SOP Class
#[rustfmt::skip]
pub const DISPLAY_SYSTEM: &str = "1.2.840.10008.5.1.1.40";
/// Well-known SOP Instance: Display System SOP Instance
#[rustfmt::skip]
pub const DISPLAY_SYSTEM_INSTANCE: &str = "1.2.840.10008.5.1.1.40.1";
/// Meta SOP Class: Basic Grayscale Print Management Meta SOP Class
#[rustfmt::skip]
pub const BASIC_GRAYSCALE_PRINT_MANAGEMENT_META: &str = "1.2.840.10008.5.1.1.9";
/// Meta SOP Class: Referenced Grayscale Print Management Meta SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const REFERENCED_GRAYSCALE_PRINT_MANAGEMENT_META: &str = "1.2.840.10008.5.1.1.9.1";
/// SOP Class: Computed Radiography Image Storage
#[rustfmt::skip]
pub const COMPUTED_RADIOGRAPHY_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.1";
/// SOP Class: Digital X-Ray Image Storage - For Presentation
#[rustfmt::skip]
pub const DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";
/// SOP Class: Digital X-Ray Image Storage - For Processing
#[rustfmt::skip]
pub const DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.1.1.1";
/// SOP Class: Digital Mammography X-Ray Image Storage - For Presentation
#[rustfmt::skip]
pub const DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.2";
/// SOP Class: Digital Mammography X-Ray Image Storage - For Processing
#[rustfmt::skip]
pub const DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.1.2.1";
/// SOP Class: Digital Intra-Oral X-Ray Image Storage - For Presentation
#[rustfmt::skip]
pub const DIGITAL_INTRA_ORAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.3";
/// SOP Class: Digital Intra-Oral X-Ray Image Storage - For Processing
#[rustfmt::skip]
pub const DIGITAL_INTRA_ORAL_X_RAY_IMAGE_STORAGE_FOR_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.1.3.1";
/// SOP Class: Standalone Modality LUT Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STANDALONE_MODALITY_LUT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.10";
/// SOP Class: Encapsulated PDF Storage
#[rustfmt::skip]
pub const ENCAPSULATED_PDF_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.104.1";
/// SOP Class: Encapsulated CDA Storage
#[rustfmt::skip]
pub const ENCAPSULATED_CDA_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.104.2";
/// SOP Class: Encapsulated STL Storage
#[rustfmt::skip]
pub const ENCAPSULATED_STL_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.104.3";
/// SOP Class: Encapsulated OBJ Storage
#[rustfmt::skip]
pub const ENCAPSULATED_OBJ_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.104.4";
/// SOP Class: Encapsulated MTL Storage
#[rustfmt::skip]
pub const ENCAPSULATED_MTL_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.104.5";
/// SOP Class: Standalone VOI LUT Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STANDALONE_VOILUT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11";
/// SOP Class: Grayscale Softcopy Presentation State Storage
#[rustfmt::skip]
pub const GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.1";
/// SOP Class: Segmented Volume Rendering Volumetric Presentation State Storage
#[rustfmt::skip]
pub const SEGMENTED_VOLUME_RENDERING_VOLUMETRIC_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.10";
/// SOP Class: Multiple Volume Rendering Volumetric Presentation State Storage
#[rustfmt::skip]
pub const MULTIPLE_VOLUME_RENDERING_VOLUMETRIC_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.11";
/// SOP Class: Variable Modality LUT Softcopy Presentation State Storage
#[rustfmt::skip]
pub const VARIABLE_MODALITY_LUT_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.12";
/// SOP Class: Color Softcopy Presentation State Storage
#[rustfmt::skip]
pub const COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.2";
/// SOP Class: Pseudo-Color Softcopy Presentation State Storage
#[rustfmt::skip]
pub const PSEUDO_COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.3";
/// SOP Class: Blending Softcopy Presentation State Storage
#[rustfmt::skip]
pub const BLENDING_SOFTCOPY_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.4";
/// SOP Class: XA/XRF Grayscale Softcopy Presentation State Storage
#[rustfmt::skip]
pub const XAXRF_GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.5";
/// SOP Class: Grayscale Planar MPR Volumetric Presentation State Storage
#[rustfmt::skip]
pub const GRAYSCALE_PLANAR_MPR_VOLUMETRIC_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.6";
/// SOP Class: Compositing Planar MPR Volumetric Presentation State Storage
#[rustfmt::skip]
pub const COMPOSITING_PLANAR_MPR_VOLUMETRIC_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.7";
/// SOP Class: Advanced Blending Presentation State Storage
#[rustfmt::skip]
pub const ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.8";
/// SOP Class: Volume Rendering Volumetric Presentation State Storage
#[rustfmt::skip]
pub const VOLUME_RENDERING_VOLUMETRIC_PRESENTATION_STATE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.11.9";
/// SOP Class: X-Ray Angiographic Image Storage
#[rustfmt::skip]
pub const X_RAY_ANGIOGRAPHIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.12.1";
/// SOP Class: Enhanced XA Image Storage
#[rustfmt::skip]
pub const ENHANCED_XA_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.12.1.1";
/// SOP Class: X-Ray Radiofluoroscopic Image Storage
#[rustfmt::skip]
pub const X_RAY_RADIOFLUOROSCOPIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.12.2";
/// SOP Class: Enhanced XRF Image Storage
#[rustfmt::skip]
pub const ENHANCED_XRF_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.12.2.1";
/// SOP Class: X-Ray Angiographic Bi-Plane Image Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const X_RAY_ANGIOGRAPHIC_BI_PLANE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.12.3";
/// SOP Class: Positron Emission Tomography Image Storage
#[rustfmt::skip]
pub const POSITRON_EMISSION_TOMOGRAPHY_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.128";
/// SOP Class: Legacy Converted Enhanced PET Image Storage
#[rustfmt::skip]
pub const LEGACY_CONVERTED_ENHANCED_PET_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.128.1";
/// SOP Class: Standalone PET Curve Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STANDALONE_PET_CURVE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.129";
/// SOP Class: X-Ray 3D Angiographic Image Storage
#[rustfmt::skip]
pub const X_RAY3_D_ANGIOGRAPHIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.13.1.1";
/// SOP Class: X-Ray 3D Craniofacial Image Storage
#[rustfmt::skip]
pub const X_RAY3_D_CRANIOFACIAL_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.13.1.2";
/// SOP Class: Breast Tomosynthesis Image Storage
#[rustfmt::skip]
pub const BREAST_TOMOSYNTHESIS_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.13.1.3";
/// SOP Class: Breast Projection X-Ray Image Storage - For Presentation
#[rustfmt::skip]
pub const BREAST_PROJECTION_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.13.1.4";
/// SOP Class: Breast Projection X-Ray Image Storage - For Processing
#[rustfmt::skip]
pub const BREAST_PROJECTION_X_RAY_IMAGE_STORAGE_FOR_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.13.1.5";
/// SOP Class: Enhanced PET Image Storage
#[rustfmt::skip]
pub const ENHANCED_PET_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.130";
/// SOP Class: Basic Structured Display Storage
#[rustfmt::skip]
pub const BASIC_STRUCTURED_DISPLAY_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.131";
/// SOP Class: Intravascular Optical Coherence Tomography Image Storage - For Presentation
#[rustfmt::skip]
pub const INTRAVASCULAR_OPTICAL_COHERENCE_TOMOGRAPHY_IMAGE_STORAGE_FOR_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.14.1";
/// SOP Class: Intravascular Optical Coherence Tomography Image Storage - For Processing
#[rustfmt::skip]
pub const INTRAVASCULAR_OPTICAL_COHERENCE_TOMOGRAPHY_IMAGE_STORAGE_FOR_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.14.2";
/// SOP Class: CT Image Storage
#[rustfmt::skip]
pub const CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";
/// SOP Class: Enhanced CT Image Storage
#[rustfmt::skip]
pub const ENHANCED_CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2.1";
/// SOP Class: Legacy Converted Enhanced CT Image Storage
#[rustfmt::skip]
pub const LEGACY_CONVERTED_ENHANCED_CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2.2";
/// SOP Class: Nuclear Medicine Image Storage
#[rustfmt::skip]
pub const NUCLEAR_MEDICINE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.20";
/// SOP Class: CT Defined Procedure Protocol Storage
#[rustfmt::skip]
pub const CT_DEFINED_PROCEDURE_PROTOCOL_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.200.1";
/// SOP Class: CT Performed Procedure Protocol Storage
#[rustfmt::skip]
pub const CT_PERFORMED_PROCEDURE_PROTOCOL_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.200.2";
/// SOP Class: Protocol Approval Storage
#[rustfmt::skip]
pub const PROTOCOL_APPROVAL_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.200.3";
/// SOP Class: Protocol Approval Information Model - FIND
#[rustfmt::skip]
pub const PROTOCOL_APPROVAL_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.1.1.200.4";
/// SOP Class: Protocol Approval Information Model - MOVE
#[rustfmt::skip]
pub const PROTOCOL_APPROVAL_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.1.1.200.5";
/// SOP Class: Protocol Approval Information Model - GET
#[rustfmt::skip]
pub const PROTOCOL_APPROVAL_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.1.1.200.6";
/// SOP Class: XA Defined Procedure Protocol Storage
#[rustfmt::skip]
pub const XA_DEFINED_PROCEDURE_PROTOCOL_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.200.7";
/// SOP Class: XA Performed Procedure Protocol Storage
#[rustfmt::skip]
pub const XA_PERFORMED_PROCEDURE_PROTOCOL_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.200.8";
/// SOP Class: Inventory Storage
#[rustfmt::skip]
pub const INVENTORY_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.201.1";
/// Well-known SOP Instance: Storage Management SOP Instance
#[rustfmt::skip]
pub const STORAGE_MANAGEMENT_INSTANCE: &str = "1.2.840.10008.5.1.4.1.1.201.1.1";
/// SOP Class: Inventory - FIND
#[rustfmt::skip]
pub const INVENTORY_FIND: &str = "1.2.840.10008.5.1.4.1.1.201.2";
/// SOP Class: Inventory - MOVE
#[rustfmt::skip]
pub const INVENTORY_MOVE: &str = "1.2.840.10008.5.1.4.1.1.201.3";
/// SOP Class: Inventory - GET
#[rustfmt::skip]
pub const INVENTORY_GET: &str = "1.2.840.10008.5.1.4.1.1.201.4";
/// SOP Class: Inventory Creation
#[rustfmt::skip]
pub const INVENTORY_CREATION: &str = "1.2.840.10008.5.1.4.1.1.201.5";
/// SOP Class: Repository Query
#[rustfmt::skip]
pub const REPOSITORY_QUERY: &str = "1.2.840.10008.5.1.4.1.1.201.6";
/// SOP Class: Ultrasound Multi-frame Image Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE_RETIRED: &str = "1.2.840.10008.5.1.4.1.1.3";
/// SOP Class: Ultrasound Multi-frame Image Storage
#[rustfmt::skip]
pub const ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.3.1";
/// SOP Class: Parametric Map Storage
#[rustfmt::skip]
pub const PARAMETRIC_MAP_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.30";
/// SOP Class: MR Image Storage
#[rustfmt::skip]
pub const MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
/// SOP Class: Enhanced MR Image Storage
#[rustfmt::skip]
pub const ENHANCED_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4.1";
/// SOP Class: MR Spectroscopy Storage
#[rustfmt::skip]
pub const MR_SPECTROSCOPY_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4.2";
/// SOP Class: Enhanced MR Color Image Storage
#[rustfmt::skip]
pub const ENHANCED_MR_COLOR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4.3";
/// SOP Class: Legacy Converted Enhanced MR Image Storage
#[rustfmt::skip]
pub const LEGACY_CONVERTED_ENHANCED_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4.4";
/// SOP Class: RT Image Storage
#[rustfmt::skip]
pub const RT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.1";
/// SOP Class: RT Physician Intent Storage
#[rustfmt::skip]
pub const RT_PHYSICIAN_INTENT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.10";
/// SOP Class: RT Segment Annotation Storage
#[rustfmt::skip]
pub const RT_SEGMENT_ANNOTATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.11";
/// SOP Class: RT Radiation Set Storage
#[rustfmt::skip]
pub const RT_RADIATION_SET_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.12";
/// SOP Class: C-Arm Photon-Electron Radiation Storage
#[rustfmt::skip]
pub const C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.13";
/// SOP Class: Tomotherapeutic Radiation Storage
#[rustfmt::skip]
pub const TOMOTHERAPEUTIC_RADIATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.14";
/// SOP Class: Robotic-Arm Radiation Storage
#[rustfmt::skip]
pub const ROBOTIC_ARM_RADIATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.15";
/// SOP Class: RT Radiation Record Set Storage
#[rustfmt::skip]
pub const RT_RADIATION_RECORD_SET_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.16";
/// SOP Class: RT Radiation Salvage Record Storage
#[rustfmt::skip]
pub const RT_RADIATION_SALVAGE_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.17";
/// SOP Class: Tomotherapeutic Radiation Record Storage
#[rustfmt::skip]
pub const TOMOTHERAPEUTIC_RADIATION_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.18";
/// SOP Class: C-Arm Photon-Electron Radiation Record Storage
#[rustfmt::skip]
pub const C_ARM_PHOTON_ELECTRON_RADIATION_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.19";
/// SOP Class: RT Dose Storage
#[rustfmt::skip]
pub const RT_DOSE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.2";
/// SOP Class: Robotic Radiation Record Storage
#[rustfmt::skip]
pub const ROBOTIC_RADIATION_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.20";
/// SOP Class: RT Radiation Set Delivery Instruction Storage
#[rustfmt::skip]
pub const RT_RADIATION_SET_DELIVERY_INSTRUCTION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.21";
/// SOP Class: RT Treatment Preparation Storage
#[rustfmt::skip]
pub const RT_TREATMENT_PREPARATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.22";
/// SOP Class: Enhanced RT Image Storage
#[rustfmt::skip]
pub const ENHANCED_RT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.23";
/// SOP Class: Enhanced Continuous RT Image Storage
#[rustfmt::skip]
pub const ENHANCED_CONTINUOUS_RT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.24";
/// SOP Class: RT Patient Position Acquisition Instruction Storage
#[rustfmt::skip]
pub const RT_PATIENT_POSITION_ACQUISITION_INSTRUCTION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.25";
/// SOP Class: RT Structure Set Storage
#[rustfmt::skip]
pub const RT_STRUCTURE_SET_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.3";
/// SOP Class: RT Beams Treatment Record Storage
#[rustfmt::skip]
pub const RT_BEAMS_TREATMENT_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.4";
/// SOP Class: RT Plan Storage
#[rustfmt::skip]
pub const RT_PLAN_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.5";
/// SOP Class: RT Brachy Treatment Record Storage
#[rustfmt::skip]
pub const RT_BRACHY_TREATMENT_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.6";
/// SOP Class: RT Treatment Summary Record Storage
#[rustfmt::skip]
pub const RT_TREATMENT_SUMMARY_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.7";
/// SOP Class: RT Ion Plan Storage
#[rustfmt::skip]
pub const RT_ION_PLAN_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.8";
/// SOP Class: RT Ion Beams Treatment Record Storage
#[rustfmt::skip]
pub const RT_ION_BEAMS_TREATMENT_RECORD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.481.9";
/// SOP Class: Nuclear Medicine Image Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const NUCLEAR_MEDICINE_IMAGE_STORAGE_RETIRED: &str = "1.2.840.10008.5.1.4.1.1.5";
/// SOP Class: DICOS CT Image Storage
#[rustfmt::skip]
pub const DICOSCT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.501.1";
/// SOP Class: DICOS Digital X-Ray Image Storage - For Presentation
#[rustfmt::skip]
pub const DICOS_DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.501.2.1";
/// SOP Class: DICOS Digital X-Ray Image Storage - For Processing
#[rustfmt::skip]
pub const DICOS_DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.501.2.2";
/// SOP Class: DICOS Threat Detection Report Storage
#[rustfmt::skip]
pub const DICOS_THREAT_DETECTION_REPORT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.501.3";
/// SOP Class: DICOS 2D AIT Storage
#[rustfmt::skip]
pub const DICOS2DAIT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.501.4";
/// SOP Class: DICOS 3D AIT Storage
#[rustfmt::skip]
pub const DICOS3DAIT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.501.5";
/// SOP Class: DICOS Quadrupole Resonance (QR) Storage
#[rustfmt::skip]
pub const DICOS_QUADRUPOLE_RESONANCE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.501.6";
/// SOP Class: Ultrasound Image Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const ULTRASOUND_IMAGE_STORAGE_RETIRED: &str = "1.2.840.10008.5.1.4.1.1.6";
/// SOP Class: Ultrasound Image Storage
#[rustfmt::skip]
pub const ULTRASOUND_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.6.1";
/// SOP Class: Enhanced US Volume Storage
#[rustfmt::skip]
pub const ENHANCED_US_VOLUME_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.6.2";
/// SOP Class: Eddy Current Image Storage
#[rustfmt::skip]
pub const EDDY_CURRENT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.601.1";
/// SOP Class: Eddy Current Multi-frame Image Storage
#[rustfmt::skip]
pub const EDDY_CURRENT_MULTI_FRAME_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.601.2";
/// SOP Class: Raw Data Storage
#[rustfmt::skip]
pub const RAW_DATA_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66";
/// SOP Class: Spatial Registration Storage
#[rustfmt::skip]
pub const SPATIAL_REGISTRATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66.1";
/// SOP Class: Spatial Fiducials Storage
#[rustfmt::skip]
pub const SPATIAL_FIDUCIALS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66.2";
/// SOP Class: Deformable Spatial Registration Storage
#[rustfmt::skip]
pub const DEFORMABLE_SPATIAL_REGISTRATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66.3";
/// SOP Class: Segmentation Storage
#[rustfmt::skip]
pub const SEGMENTATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66.4";
/// SOP Class: Surface Segmentation Storage
#[rustfmt::skip]
pub const SURFACE_SEGMENTATION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66.5";
/// SOP Class: Tractography Results Storage
#[rustfmt::skip]
pub const TRACTOGRAPHY_RESULTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.66.6";
/// SOP Class: Real World Value Mapping Storage
#[rustfmt::skip]
pub const REAL_WORLD_VALUE_MAPPING_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.67";
/// SOP Class: Surface Scan Mesh Storage
#[rustfmt::skip]
pub const SURFACE_SCAN_MESH_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.68.1";
/// SOP Class: Surface Scan Point Cloud Storage
#[rustfmt::skip]
pub const SURFACE_SCAN_POINT_CLOUD_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.68.2";
/// SOP Class: Secondary Capture Image Storage
#[rustfmt::skip]
pub const SECONDARY_CAPTURE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.7";
/// SOP Class: Multi-frame Single Bit Secondary Capture Image Storage
#[rustfmt::skip]
pub const MULTI_FRAME_SINGLE_BIT_SECONDARY_CAPTURE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.7.1";
/// SOP Class: Multi-frame Grayscale Byte Secondary Capture Image Storage
#[rustfmt::skip]
pub const MULTI_FRAME_GRAYSCALE_BYTE_SECONDARY_CAPTURE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
/// SOP Class: Multi-frame Grayscale Word Secondary Capture Image Storage
#[rustfmt::skip]
pub const MULTI_FRAME_GRAYSCALE_WORD_SECONDARY_CAPTURE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.7.3";
/// SOP Class: Multi-frame True Color Secondary Capture Image Storage
#[rustfmt::skip]
pub const MULTI_FRAME_TRUE_COLOR_SECONDARY_CAPTURE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.7.4";
/// SOP Class: VL Image Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const VL_IMAGE_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.1.1.77.1";
/// SOP Class: VL Endoscopic Image Storage
#[rustfmt::skip]
pub const VL_ENDOSCOPIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.1";
/// SOP Class: Video Endoscopic Image Storage
#[rustfmt::skip]
pub const VIDEO_ENDOSCOPIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.1.1";
/// SOP Class: VL Microscopic Image Storage
#[rustfmt::skip]
pub const VL_MICROSCOPIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.2";
/// SOP Class: Video Microscopic Image Storage
#[rustfmt::skip]
pub const VIDEO_MICROSCOPIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.2.1";
/// SOP Class: VL Slide-Coordinates Microscopic Image Storage
#[rustfmt::skip]
pub const VL_SLIDE_COORDINATES_MICROSCOPIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.3";
/// SOP Class: VL Photographic Image Storage
#[rustfmt::skip]
pub const VL_PHOTOGRAPHIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.4";
/// SOP Class: Video Photographic Image Storage
#[rustfmt::skip]
pub const VIDEO_PHOTOGRAPHIC_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.4.1";
/// SOP Class: Ophthalmic Photography 8 Bit Image Storage
#[rustfmt::skip]
pub const OPHTHALMIC_PHOTOGRAPHY8_BIT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.1";
/// SOP Class: Ophthalmic Photography 16 Bit Image Storage
#[rustfmt::skip]
pub const OPHTHALMIC_PHOTOGRAPHY16_BIT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.2";
/// SOP Class: Stereometric Relationship Storage
#[rustfmt::skip]
pub const STEREOMETRIC_RELATIONSHIP_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.3";
/// SOP Class: Ophthalmic Tomography Image Storage
#[rustfmt::skip]
pub const OPHTHALMIC_TOMOGRAPHY_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.4";
/// SOP Class: Wide Field Ophthalmic Photography Stereographic Projection Image Storage
#[rustfmt::skip]
pub const WIDE_FIELD_OPHTHALMIC_PHOTOGRAPHY_STEREOGRAPHIC_PROJECTION_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.5";
/// SOP Class: Wide Field Ophthalmic Photography 3D Coordinates Image Storage
#[rustfmt::skip]
pub const WIDE_FIELD_OPHTHALMIC_PHOTOGRAPHY3_D_COORDINATES_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.6";
/// SOP Class: Ophthalmic Optical Coherence Tomography En Face Image Storage
#[rustfmt::skip]
pub const OPHTHALMIC_OPTICAL_COHERENCE_TOMOGRAPHY_EN_FACE_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.7";
/// SOP Class: Ophthalmic Optical Coherence Tomography B-scan Volume Analysis Storage
#[rustfmt::skip]
pub const OPHTHALMIC_OPTICAL_COHERENCE_TOMOGRAPHY_BSCAN_VOLUME_ANALYSIS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.8";
/// SOP Class: VL Whole Slide Microscopy Image Storage
#[rustfmt::skip]
pub const VL_WHOLE_SLIDE_MICROSCOPY_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";
/// SOP Class: Dermoscopic Photography Image Storage
#[rustfmt::skip]
pub const DERMOSCOPIC_PHOTOGRAPHY_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.7";
/// SOP Class: VL Multi-frame Image Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const VL_MULTI_FRAME_IMAGE_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.1.1.77.2";
/// SOP Class: Lensometry Measurements Storage
#[rustfmt::skip]
pub const LENSOMETRY_MEASUREMENTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.1";
/// SOP Class: Autorefraction Measurements Storage
#[rustfmt::skip]
pub const AUTOREFRACTION_MEASUREMENTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.2";
/// SOP Class: Keratometry Measurements Storage
#[rustfmt::skip]
pub const KERATOMETRY_MEASUREMENTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.3";
/// SOP Class: Subjective Refraction Measurements Storage
#[rustfmt::skip]
pub const SUBJECTIVE_REFRACTION_MEASUREMENTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.4";
/// SOP Class: Visual Acuity Measurements Storage
#[rustfmt::skip]
pub const VISUAL_ACUITY_MEASUREMENTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.5";
/// SOP Class: Spectacle Prescription Report Storage
#[rustfmt::skip]
pub const SPECTACLE_PRESCRIPTION_REPORT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.6";
/// SOP Class: Ophthalmic Axial Measurements Storage
#[rustfmt::skip]
pub const OPHTHALMIC_AXIAL_MEASUREMENTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.7";
/// SOP Class: Intraocular Lens Calculations Storage
#[rustfmt::skip]
pub const INTRAOCULAR_LENS_CALCULATIONS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.78.8";
/// SOP Class: Macular Grid Thickness and Volume Report Storage
#[rustfmt::skip]
pub const MACULAR_GRID_THICKNESS_AND_VOLUME_REPORT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.79.1";
/// SOP Class: Standalone Overlay Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STANDALONE_OVERLAY_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.8";
/// SOP Class: Ophthalmic Visual Field Static Perimetry Measurements Storage
#[rustfmt::skip]
pub const OPHTHALMIC_VISUAL_FIELD_STATIC_PERIMETRY_MEASUREMENTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.80.1";
/// SOP Class: Ophthalmic Thickness Map Storage
#[rustfmt::skip]
pub const OPHTHALMIC_THICKNESS_MAP_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.81.1";
/// SOP Class: Corneal Topography Map Storage
#[rustfmt::skip]
pub const CORNEAL_TOPOGRAPHY_MAP_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.82.1";
/// SOP Class: Text SR Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const TEXT_SR_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.1.1.88.1";
/// SOP Class: Basic Text SR Storage
#[rustfmt::skip]
pub const BASIC_TEXT_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.11";
/// SOP Class: Audio SR Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const AUDIO_SR_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.1.1.88.2";
/// SOP Class: Enhanced SR Storage
#[rustfmt::skip]
pub const ENHANCED_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.22";
/// SOP Class: Detail SR Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const DETAIL_SR_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.1.1.88.3";
/// SOP Class: Comprehensive SR Storage
#[rustfmt::skip]
pub const COMPREHENSIVE_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.33";
/// SOP Class: Comprehensive 3D SR Storage
#[rustfmt::skip]
pub const COMPREHENSIVE3_DSR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.34";
/// SOP Class: Extensible SR Storage
#[rustfmt::skip]
pub const EXTENSIBLE_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.35";
/// SOP Class: Comprehensive SR Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const COMPREHENSIVE_SR_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.1.1.88.4";
/// SOP Class: Procedure Log Storage
#[rustfmt::skip]
pub const PROCEDURE_LOG_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.40";
/// SOP Class: Mammography CAD SR Storage
#[rustfmt::skip]
pub const MAMMOGRAPHY_CADSR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.50";
/// SOP Class: Key Object Selection Document Storage
#[rustfmt::skip]
pub const KEY_OBJECT_SELECTION_DOCUMENT_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.59";
/// SOP Class: Chest CAD SR Storage
#[rustfmt::skip]
pub const CHEST_CADSR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.65";
/// SOP Class: X-Ray Radiation Dose SR Storage
#[rustfmt::skip]
pub const X_RAY_RADIATION_DOSE_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.67";
/// SOP Class: Radiopharmaceutical Radiation Dose SR Storage
#[rustfmt::skip]
pub const RADIOPHARMACEUTICAL_RADIATION_DOSE_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.68";
/// SOP Class: Colon CAD SR Storage
#[rustfmt::skip]
pub const COLON_CADSR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.69";
/// SOP Class: Implantation Plan SR Storage
#[rustfmt::skip]
pub const IMPLANTATION_PLAN_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.70";
/// SOP Class: Acquisition Context SR Storage
#[rustfmt::skip]
pub const ACQUISITION_CONTEXT_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.71";
/// SOP Class: Simplified Adult Echo SR Storage
#[rustfmt::skip]
pub const SIMPLIFIED_ADULT_ECHO_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.72";
/// SOP Class: Patient Radiation Dose SR Storage
#[rustfmt::skip]
pub const PATIENT_RADIATION_DOSE_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.73";
/// SOP Class: Planned Imaging Agent Administration SR Storage
#[rustfmt::skip]
pub const PLANNED_IMAGING_AGENT_ADMINISTRATION_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.74";
/// SOP Class: Performed Imaging Agent Administration SR Storage
#[rustfmt::skip]
pub const PERFORMED_IMAGING_AGENT_ADMINISTRATION_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.75";
/// SOP Class: Enhanced X-Ray Radiation Dose SR Storage
#[rustfmt::skip]
pub const ENHANCED_X_RAY_RADIATION_DOSE_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.76";
/// SOP Class: Standalone Curve Storage (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const STANDALONE_CURVE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9";
/// SOP Class: Waveform Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const WAVEFORM_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.1.1.9.1";
/// SOP Class: 12-lead ECG Waveform Storage
#[rustfmt::skip]
pub const TWELVE_LEAD_ECG_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.1.1";
/// SOP Class: General ECG Waveform Storage
#[rustfmt::skip]
pub const GENERAL_ECG_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.1.2";
/// SOP Class: Ambulatory ECG Waveform Storage
#[rustfmt::skip]
pub const AMBULATORY_ECG_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.1.3";
/// SOP Class: Hemodynamic Waveform Storage
#[rustfmt::skip]
pub const HEMODYNAMIC_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.2.1";
/// SOP Class: Cardiac Electrophysiology Waveform Storage
#[rustfmt::skip]
pub const CARDIAC_ELECTROPHYSIOLOGY_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.3.1";
/// SOP Class: Basic Voice Audio Waveform Storage
#[rustfmt::skip]
pub const BASIC_VOICE_AUDIO_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.4.1";
/// SOP Class: General Audio Waveform Storage
#[rustfmt::skip]
pub const GENERAL_AUDIO_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.4.2";
/// SOP Class: Arterial Pulse Waveform Storage
#[rustfmt::skip]
pub const ARTERIAL_PULSE_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.5.1";
/// SOP Class: Respiratory Waveform Storage
#[rustfmt::skip]
pub const RESPIRATORY_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.6.1";
/// SOP Class: Multi-channel Respiratory Waveform Storage
#[rustfmt::skip]
pub const MULTICHANNEL_RESPIRATORY_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.6.2";
/// SOP Class: Routine Scalp Electroencephalogram Waveform Storage
#[rustfmt::skip]
pub const ROUTINE_SCALP_ELECTROENCEPHALOGRAM_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.7.1";
/// SOP Class: Electromyogram Waveform Storage
#[rustfmt::skip]
pub const ELECTROMYOGRAM_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.7.2";
/// SOP Class: Electrooculogram Waveform Storage
#[rustfmt::skip]
pub const ELECTROOCULOGRAM_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.7.3";
/// SOP Class: Sleep Electroencephalogram Waveform Storage
#[rustfmt::skip]
pub const SLEEP_ELECTROENCEPHALOGRAM_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.7.4";
/// SOP Class: Body Position Waveform Storage
#[rustfmt::skip]
pub const BODY_POSITION_WAVEFORM_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.9.8.1";
/// SOP Class: Content Assessment Results Storage
#[rustfmt::skip]
pub const CONTENT_ASSESSMENT_RESULTS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.90.1";
/// SOP Class: Microscopy Bulk Simple Annotations Storage
#[rustfmt::skip]
pub const MICROSCOPY_BULK_SIMPLE_ANNOTATIONS_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.91.1";
/// SOP Class: Patient Root Query/Retrieve Information Model - FIND
#[rustfmt::skip]
pub const PATIENT_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.1.2.1.1";
/// SOP Class: Patient Root Query/Retrieve Information Model - MOVE
#[rustfmt::skip]
pub const PATIENT_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.1.2.1.2";
/// SOP Class: Patient Root Query/Retrieve Information Model - GET
#[rustfmt::skip]
pub const PATIENT_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.1.2.1.3";
/// SOP Class: Study Root Query/Retrieve Information Model - FIND
#[rustfmt::skip]
pub const STUDY_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.1.2.2.1";
/// SOP Class: Study Root Query/Retrieve Information Model - MOVE
#[rustfmt::skip]
pub const STUDY_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.1.2.2.2";
/// SOP Class: Study Root Query/Retrieve Information Model - GET
#[rustfmt::skip]
pub const STUDY_ROOT_QUERY_RETRIEVE_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.1.2.2.3";
/// SOP Class: Patient/Study Only Query/Retrieve Information Model - FIND (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PATIENT_STUDY_ONLY_QUERY_RETRIEVE_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.1.2.3.1";
/// SOP Class: Patient/Study Only Query/Retrieve Information Model - MOVE (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PATIENT_STUDY_ONLY_QUERY_RETRIEVE_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.1.2.3.2";
/// SOP Class: Patient/Study Only Query/Retrieve Information Model - GET (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const PATIENT_STUDY_ONLY_QUERY_RETRIEVE_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.1.2.3.3";
/// SOP Class: Composite Instance Root Retrieve - MOVE
#[rustfmt::skip]
pub const COMPOSITE_INSTANCE_ROOT_RETRIEVE_MOVE: &str = "1.2.840.10008.5.1.4.1.2.4.2";
/// SOP Class: Composite Instance Root Retrieve - GET
#[rustfmt::skip]
pub const COMPOSITE_INSTANCE_ROOT_RETRIEVE_GET: &str = "1.2.840.10008.5.1.4.1.2.4.3";
/// SOP Class: Composite Instance Retrieve Without Bulk Data - GET
#[rustfmt::skip]
pub const COMPOSITE_INSTANCE_RETRIEVE_WITHOUT_BULK_DATA_GET: &str = "1.2.840.10008.5.1.4.1.2.5.3";
/// SOP Class: Defined Procedure Protocol Information Model - FIND
#[rustfmt::skip]
pub const DEFINED_PROCEDURE_PROTOCOL_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.20.1";
/// SOP Class: Defined Procedure Protocol Information Model - MOVE
#[rustfmt::skip]
pub const DEFINED_PROCEDURE_PROTOCOL_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.20.2";
/// SOP Class: Defined Procedure Protocol Information Model - GET
#[rustfmt::skip]
pub const DEFINED_PROCEDURE_PROTOCOL_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.20.3";
/// SOP Class: Modality Worklist Information Model - FIND
#[rustfmt::skip]
pub const MODALITY_WORKLIST_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.31";
/// Meta SOP Class: General Purpose Worklist Management Meta SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const GENERAL_PURPOSE_WORKLIST_MANAGEMENT_META: &str = "1.2.840.10008.5.1.4.32";
/// SOP Class: General Purpose Worklist Information Model - FIND (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const GENERAL_PURPOSE_WORKLIST_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.32.1";
/// SOP Class: General Purpose Scheduled Procedure Step SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const GENERAL_PURPOSE_SCHEDULED_PROCEDURE_STEP: &str = "1.2.840.10008.5.1.4.32.2";
/// SOP Class: General Purpose Performed Procedure Step SOP Class (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const GENERAL_PURPOSE_PERFORMED_PROCEDURE_STEP: &str = "1.2.840.10008.5.1.4.32.3";
/// SOP Class: Instance Availability Notification SOP Class
#[rustfmt::skip]
pub const INSTANCE_AVAILABILITY_NOTIFICATION: &str = "1.2.840.10008.5.1.4.33";
/// SOP Class: RT Beams Delivery Instruction Storage - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const RT_BEAMS_DELIVERY_INSTRUCTION_STORAGE_TRIAL: &str = "1.2.840.10008.5.1.4.34.1";
/// SOP Class: RT Brachy Application Setup Delivery Instruction Storage
#[rustfmt::skip]
pub const RT_BRACHY_APPLICATION_SETUP_DELIVERY_INSTRUCTION_STORAGE: &str = "1.2.840.10008.5.1.4.34.10";
/// SOP Class: RT Conventional Machine Verification - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const RT_CONVENTIONAL_MACHINE_VERIFICATION_TRIAL: &str = "1.2.840.10008.5.1.4.34.2";
/// SOP Class: RT Ion Machine Verification - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const RT_ION_MACHINE_VERIFICATION_TRIAL: &str = "1.2.840.10008.5.1.4.34.3";
/// Service Class: Unified Worklist and Procedure Step Service Class - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const UNIFIED_WORKLIST_AND_PROCEDURE_STEP_TRIAL: &str = "1.2.840.10008.5.1.4.34.4";
/// SOP Class: Unified Procedure Step - Push SOP Class - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_PUSH_TRIAL: &str = "1.2.840.10008.5.1.4.34.4.1";
/// SOP Class: Unified Procedure Step - Watch SOP Class - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_WATCH_TRIAL: &str = "1.2.840.10008.5.1.4.34.4.2";
/// SOP Class: Unified Procedure Step - Pull SOP Class - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_PULL_TRIAL: &str = "1.2.840.10008.5.1.4.34.4.3";
/// SOP Class: Unified Procedure Step - Event SOP Class - Trial (Retired)
#[deprecated(note = "Retired DICOM UID")]
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_EVENT_TRIAL: &str = "1.2.840.10008.5.1.4.34.4.4";
/// Well-known SOP Instance: UPS Global Subscription SOP Instance
#[rustfmt::skip]
pub const UPS_GLOBAL_SUBSCRIPTION_INSTANCE: &str = "1.2.840.10008.5.1.4.34.5";
/// Well-known SOP Instance: UPS Filtered Global Subscription SOP Instance
#[rustfmt::skip]
pub const UPS_FILTERED_GLOBAL_SUBSCRIPTION_INSTANCE: &str = "1.2.840.10008.5.1.4.34.5.1";
/// Service Class: Unified Worklist and Procedure Step Service Class
#[rustfmt::skip]
pub const UNIFIED_WORKLIST_AND_PROCEDURE_STEP: &str = "1.2.840.10008.5.1.4.34.6";
/// SOP Class: Unified Procedure Step - Push SOP Class
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_PUSH: &str = "1.2.840.10008.5.1.4.34.6.1";
/// SOP Class: Unified Procedure Step - Watch SOP Class
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_WATCH: &str = "1.2.840.10008.5.1.4.34.6.2";
/// SOP Class: Unified Procedure Step - Pull SOP Class
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_PULL: &str = "1.2.840.10008.5.1.4.34.6.3";
/// SOP Class: Unified Procedure Step - Event SOP Class
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_EVENT: &str = "1.2.840.10008.5.1.4.34.6.4";
/// SOP Class: Unified Procedure Step - Query SOP Class
#[rustfmt::skip]
pub const UNIFIED_PROCEDURE_STEP_QUERY: &str = "1.2.840.10008.5.1.4.34.6.5";
/// SOP Class: RT Beams Delivery Instruction Storage
#[rustfmt::skip]
pub const RT_BEAMS_DELIVERY_INSTRUCTION_STORAGE: &str = "1.2.840.10008.5.1.4.34.7";
/// SOP Class: RT Conventional Machine Verification
#[rustfmt::skip]
pub const RT_CONVENTIONAL_MACHINE_VERIFICATION: &str = "1.2.840.10008.5.1.4.34.8";
/// SOP Class: RT Ion Machine Verification
#[rustfmt::skip]
pub const RT_ION_MACHINE_VERIFICATION: &str = "1.2.840.10008.5.1.4.34.9";
/// SOP Class: General Relevant Patient Information Query
#[rustfmt::skip]
pub const GENERAL_RELEVANT_PATIENT_INFORMATION_QUERY: &str = "1.2.840.10008.5.1.4.37.1";
/// SOP Class: Breast Imaging Relevant Patient Information Query
#[rustfmt::skip]
pub const BREAST_IMAGING_RELEVANT_PATIENT_INFORMATION_QUERY: &str = "1.2.840.10008.5.1.4.37.2";
/// SOP Class: Cardiac Relevant Patient Information Query
#[rustfmt::skip]
pub const CARDIAC_RELEVANT_PATIENT_INFORMATION_QUERY: &str = "1.2.840.10008.5.1.4.37.3";
/// SOP Class: Hanging Protocol Storage
#[rustfmt::skip]
pub const HANGING_PROTOCOL_STORAGE: &str = "1.2.840.10008.5.1.4.38.1";
/// SOP Class: Hanging Protocol Information Model - FIND
#[rustfmt::skip]
pub const HANGING_PROTOCOL_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.38.2";
/// SOP Class: Hanging Protocol Information Model - MOVE
#[rustfmt::skip]
pub const HANGING_PROTOCOL_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.38.3";
/// SOP Class: Hanging Protocol Information Model - GET
#[rustfmt::skip]
pub const HANGING_PROTOCOL_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.38.4";
/// SOP Class: Color Palette Storage
#[rustfmt::skip]
pub const COLOR_PALETTE_STORAGE: &str = "1.2.840.10008.5.1.4.39.1";
/// SOP Class: Color Palette Query/Retrieve Information Model - FIND
#[rustfmt::skip]
pub const COLOR_PALETTE_QUERY_RETRIEVE_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.39.2";
/// SOP Class: Color Palette Query/Retrieve Information Model - MOVE
#[rustfmt::skip]
pub const COLOR_PALETTE_QUERY_RETRIEVE_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.39.3";
/// SOP Class: Color Palette Query/Retrieve Information Model - GET
#[rustfmt::skip]
pub const COLOR_PALETTE_QUERY_RETRIEVE_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.39.4";
/// SOP Class: Product Characteristics Query SOP Class
#[rustfmt::skip]
pub const PRODUCT_CHARACTERISTICS_QUERY: &str = "1.2.840.10008.5.1.4.41";
/// SOP Class: Substance Approval Query SOP Class
#[rustfmt::skip]
pub const SUBSTANCE_APPROVAL_QUERY: &str = "1.2.840.10008.5.1.4.42";
/// SOP Class: Generic Implant Template Storage
#[rustfmt::skip]
pub const GENERIC_IMPLANT_TEMPLATE_STORAGE: &str = "1.2.840.10008.5.1.4.43.1";
/// SOP Class: Generic Implant Template Information Model - FIND
#[rustfmt::skip]
pub const GENERIC_IMPLANT_TEMPLATE_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.43.2";
/// SOP Class: Generic Implant Template Information Model - MOVE
#[rustfmt::skip]
pub const GENERIC_IMPLANT_TEMPLATE_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.43.3";
/// SOP Class: Generic Implant Template Information Model - GET
#[rustfmt::skip]
pub const GENERIC_IMPLANT_TEMPLATE_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.43.4";
/// SOP Class: Implant Assembly Template Storage
#[rustfmt::skip]
pub const IMPLANT_ASSEMBLY_TEMPLATE_STORAGE: &str = "1.2.840.10008.5.1.4.44.1";
/// SOP Class: Implant Assembly Template Information Model - FIND
#[rustfmt::skip]
pub const IMPLANT_ASSEMBLY_TEMPLATE_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.44.2";
/// SOP Class: Implant Assembly Template Information Model - MOVE
#[rustfmt::skip]
pub const IMPLANT_ASSEMBLY_TEMPLATE_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.44.3";
/// SOP Class: Implant Assembly Template Information Model - GET
#[rustfmt::skip]
pub const IMPLANT_ASSEMBLY_TEMPLATE_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.44.4";
/// SOP Class: Implant Template Group Storage
#[rustfmt::skip]
pub const IMPLANT_TEMPLATE_GROUP_STORAGE: &str = "1.2.840.10008.5.1.4.45.1";
/// SOP Class: Implant Template Group Information Model - FIND
#[rustfmt::skip]
pub const IMPLANT_TEMPLATE_GROUP_INFORMATION_MODEL_FIND: &str = "1.2.840.10008.5.1.4.45.2";
/// SOP Class: Implant Template Group Information Model - MOVE
#[rustfmt::skip]
pub const IMPLANT_TEMPLATE_GROUP_INFORMATION_MODEL_MOVE: &str = "1.2.840.10008.5.1.4.45.3";
/// SOP Class: Implant Template Group Information Model - GET
#[rustfmt::skip]
pub const IMPLANT_TEMPLATE_GROUP_INFORMATION_MODEL_GET: &str = "1.2.840.10008.5.1.4.45.4";
/// Application Hosting Modle: Native DICOM Model
#[rustfmt::skip]
pub const NATIVE_DICOM_MODEL: &str = "1.2.840.10008.7.1.1";
/// Application Hosting Modle: Abstract Multi-Dimensional Image Model
#[rustfmt::skip]
pub const ABSTRACT_MULTI_DIMENSIONAL_IMAGE_MODEL: &str = "1.2.840.10008.7.1.2";
/// Mapping Resource: DICOM Content Mapping Resource
#[rustfmt::skip]
pub const DICOM_CONTENT_MAPPING_RESOURCE: &str = "1.2.840.10008.8.1.1";

#[allow(unused_imports)]
use dicom_core::dictionary::UidType::*;
#[allow(dead_code)]
type E = UidDictionaryEntryRef<'static>;

#[rustfmt::skip]
#[cfg(feature = "sop-class")]
pub(crate) const SOP_CLASSES: &[E] = &[
    E::new("1.2.840.10008.1.1", "Verification SOP Class", "Verification", SopClass, false),
    E::new("1.2.840.10008.1.20.1", "Storage Commitment Push Model SOP Class", "StorageCommitmentPushModel", SopClass, false),
    E::new("1.2.840.10008.1.20.2", "Storage Commitment Pull Model SOP Class (Retired)", "StorageCommitmentPullModel", SopClass, true),
    E::new("1.2.840.10008.1.3.10", "Media Storage Directory Storage", "MediaStorageDirectoryStorage", SopClass, false),
    E::new("1.2.840.10008.1.40", "Procedural Event Logging SOP Class", "ProceduralEventLogging", SopClass, false),
    E::new("1.2.840.10008.1.42", "Substance Administration Logging SOP Class", "SubstanceAdministrationLogging", SopClass, false),
    E::new("1.2.840.10008.1.9", "Basic Study Content Notification SOP Class (Retired)", "BasicStudyContentNotification", SopClass, true),
    E::new("1.2.840.10008.10.1", "Video Endoscopic Image Real-Time Communication", "VideoEndoscopicImageRealTimeCommunication", SopClass, false),
    E::new("1.2.840.10008.10.2", "Video Photographic Image Real-Time Communication", "VideoPhotographicImageRealTimeCommunication", SopClass, false),
    E::new("1.2.840.10008.10.3", "Audio Waveform Real-Time Communication", "AudioWaveformRealTimeCommunication", SopClass, false),
    E::new("1.2.840.10008.10.4", "Rendition Selection Document Real-Time Communication", "RenditionSelectionDocumentRealTimeCommunication", SopClass, false),
    E::new("1.2.840.10008.3.1.2.1.1", "Detached Patient Management SOP Class (Retired)", "DetachedPatientManagement", SopClass, true),
    E::new("1.2.840.10008.3.1.2.2.1", "Detached Visit Management SOP Class (Retired)", "DetachedVisitManagement", SopClass, true),
    E::new("1.2.840.10008.3.1.2.3.1", "Detached Study Management SOP Class (Retired)", "DetachedStudyManagement", SopClass, true),
    E::new("1.2.840.10008.3.1.2.3.2", "Study Component Management SOP Class (Retired)", "StudyComponentManagement", SopClass, true),
    E::new("1.2.840.10008.3.1.2.3.3", "Modality Performed Procedure Step SOP Class", "ModalityPerformedProcedureStep", SopClass, false),
    E::new("1.2.840.10008.3.1.2.3.4", "Modality Performed Procedure Step Retrieve SOP Class", "ModalityPerformedProcedureStepRetrieve", SopClass, false),
    E::new("1.2.840.10008.3.1.2.3.5", "Modality Performed Procedure Step Notification SOP Class", "ModalityPerformedProcedureStepNotification", SopClass, false),
    E::new("1.2.840.10008.3.1.2.5.1", "Detached Results Management SOP Class (Retired)", "DetachedResultsManagement", SopClass, true),
    E::new("1.2.840.10008.3.1.2.6.1", "Detached Interpretation Management SOP Class (Retired)", "DetachedInterpretationManagement", SopClass, true),
    E::new("1.2.840.10008.5.1.1.1", "Basic Film Session SOP Class", "BasicFilmSession", SopClass, false),
    E::new("1.2.840.10008.5.1.1.14", "Print Job SOP Class", "PrintJob", SopClass, false),
    E::new("1.2.840.10008.5.1.1.15", "Basic Annotation Box SOP Class", "BasicAnnotationBox", SopClass, false),
    E::new("1.2.840.10008.5.1.1.16", "Printer SOP Class", "Printer", SopClass, false),
    E::new("1.2.840.10008.5.1.1.16.376", "Printer Configuration Retrieval SOP Class", "PrinterConfigurationRetrieval", SopClass, false),
    E::new("1.2.840.10008.5.1.1.2", "Basic Film Box SOP Class", "BasicFilmBox", SopClass, false),
    E::new("1.2.840.10008.5.1.1.22", "VOI LUT Box SOP Class", "VOILUTBox", SopClass, false),
    E::new("1.2.840.10008.5.1.1.23", "Presentation LUT SOP Class", "PresentationLUT", SopClass, false),
    E::new("1.2.840.10008.5.1.1.24", "Image Overlay Box SOP Class (Retired)", "ImageOverlayBox", SopClass, true),
    E::new("1.2.840.10008.5.1.1.24.1", "Basic Print Image Overlay Box SOP Class (Retired)", "BasicPrintImageOverlayBox", SopClass, true),
    E::new("1.2.840.10008.5.1.1.26", "Print Queue Management SOP Class (Retired)", "PrintQueueManagement", SopClass, true),
    E::new("1.2.840.10008.5.1.1.27", "Stored Print Storage SOP Class (Retired)", "StoredPrintStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.1.29", "Hardcopy Grayscale Image Storage SOP Class (Retired)", "HardcopyGrayscaleImageStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.1.30", "Hardcopy Color Image Storage SOP Class (Retired)", "HardcopyColorImageStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.1.31", "Pull Print Request SOP Class (Retired)", "PullPrintRequest", SopClass, true),
    E::new("1.2.840.10008.5.1.1.33", "Media Creation Management SOP Class UID", "MediaCreationManagement", SopClass, false),
    E::new("1.2.840.10008.5.1.1.4", "Basic Grayscale Image Box SOP Class", "BasicGrayscaleImageBox", SopClass, false),
    E::new("1.2.840.10008.5.1.1.4.1", "Basic Color Image Box SOP Class", "BasicColorImageBox", SopClass, false),
    E::new("1.2.840.10008.5.1.1.4.2", "Referenced Image Box SOP Class (Retired)", "ReferencedImageBox", SopClass, true),
    E::new("1.2.840.10008.5.1.1.40", "Display System SOP Class", "DisplaySystem", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.1", "Computed Radiography Image Storage", "ComputedRadiographyImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.1.1", "Digital X-Ray Image Storage - For Presentation", "DigitalXRayImageStorageForPresentation", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.1.1.1", "Digital X-Ray Image Storage - For Processing", "DigitalXRayImageStorageForProcessing", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.1.2", "Digital Mammography X-Ray Image Storage - For Presentation", "DigitalMammographyXRayImageStorageForPresentation", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.1.2.1", "Digital Mammography X-Ray Image Storage - For Processing", "DigitalMammographyXRayImageStorageForProcessing", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.1.3", "Digital Intra-Oral X-Ray Image Storage - For Presentation", "DigitalIntraOralXRayImageStorageForPresentation", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.1.3.1", "Digital Intra-Oral X-Ray Image Storage - For Processing", "DigitalIntraOralXRayImageStorageForProcessing", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.10", "Standalone Modality LUT Storage (Retired)", "StandaloneModalityLUTStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.104.1", "Encapsulated PDF Storage", "EncapsulatedPDFStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.104.2", "Encapsulated CDA Storage", "EncapsulatedCDAStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.104.3", "Encapsulated STL Storage", "EncapsulatedSTLStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.104.4", "Encapsulated OBJ Storage", "EncapsulatedOBJStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.104.5", "Encapsulated MTL Storage", "EncapsulatedMTLStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11", "Standalone VOI LUT Storage (Retired)", "StandaloneVOILUTStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.11.1", "Grayscale Softcopy Presentation State Storage", "GrayscaleSoftcopyPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.10", "Segmented Volume Rendering Volumetric Presentation State Storage", "SegmentedVolumeRenderingVolumetricPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.11", "Multiple Volume Rendering Volumetric Presentation State Storage", "MultipleVolumeRenderingVolumetricPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.12", "Variable Modality LUT Softcopy Presentation State Storage", "VariableModalityLUTPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.2", "Color Softcopy Presentation State Storage", "ColorSoftcopyPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.3", "Pseudo-Color Softcopy Presentation State Storage", "PseudoColorSoftcopyPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.4", "Blending Softcopy Presentation State Storage", "BlendingSoftcopyPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.5", "XA/XRF Grayscale Softcopy Presentation State Storage", "XAXRFGrayscaleSoftcopyPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.6", "Grayscale Planar MPR Volumetric Presentation State Storage", "GrayscalePlanarMPRVolumetricPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.7", "Compositing Planar MPR Volumetric Presentation State Storage", "CompositingPlanarMPRVolumetricPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.8", "Advanced Blending Presentation State Storage", "AdvancedBlendingPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.11.9", "Volume Rendering Volumetric Presentation State Storage", "VolumeRenderingVolumetricPresentationStateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.12.1", "X-Ray Angiographic Image Storage", "XRayAngiographicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.12.1.1", "Enhanced XA Image Storage", "EnhancedXAImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.12.2", "X-Ray Radiofluoroscopic Image Storage", "XRayRadiofluoroscopicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.12.2.1", "Enhanced XRF Image Storage", "EnhancedXRFImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.12.3", "X-Ray Angiographic Bi-Plane Image Storage (Retired)", "XRayAngiographicBiPlaneImageStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.128", "Positron Emission Tomography Image Storage", "PositronEmissionTomographyImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.128.1", "Legacy Converted Enhanced PET Image Storage", "LegacyConvertedEnhancedPETImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.129", "Standalone PET Curve Storage (Retired)", "StandalonePETCurveStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.13.1.1", "X-Ray 3D Angiographic Image Storage", "XRay3DAngiographicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.13.1.2", "X-Ray 3D Craniofacial Image Storage", "XRay3DCraniofacialImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.13.1.3", "Breast Tomosynthesis Image Storage", "BreastTomosynthesisImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.13.1.4", "Breast Projection X-Ray Image Storage - For Presentation", "BreastProjectionXRayImageStorageForPresentation", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.13.1.5", "Breast Projection X-Ray Image Storage - For Processing", "BreastProjectionXRayImageStorageForProcessing", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.130", "Enhanced PET Image Storage", "EnhancedPETImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.131", "Basic Structured Display Storage", "BasicStructuredDisplayStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.14.1", "Intravascular Optical Coherence Tomography Image Storage - For Presentation", "IntravascularOpticalCoherenceTomographyImageStorageForPresentation", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.14.2", "Intravascular Optical Coherence Tomography Image Storage - For Processing", "IntravascularOpticalCoherenceTomographyImageStorageForProcessing", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.2", "CT Image Storage", "CTImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.2.1", "Enhanced CT Image Storage", "EnhancedCTImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.2.2", "Legacy Converted Enhanced CT Image Storage", "LegacyConvertedEnhancedCTImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.20", "Nuclear Medicine Image Storage", "NuclearMedicineImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.1", "CT Defined Procedure Protocol Storage", "CTDefinedProcedureProtocolStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.2", "CT Performed Procedure Protocol Storage", "CTPerformedProcedureProtocolStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.3", "Protocol Approval Storage", "ProtocolApprovalStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.4", "Protocol Approval Information Model - FIND", "ProtocolApprovalInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.5", "Protocol Approval Information Model - MOVE", "ProtocolApprovalInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.6", "Protocol Approval Information Model - GET", "ProtocolApprovalInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.7", "XA Defined Procedure Protocol Storage", "XADefinedProcedureProtocolStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.200.8", "XA Performed Procedure Protocol Storage", "XAPerformedProcedureProtocolStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.201.1", "Inventory Storage", "InventoryStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.201.2", "Inventory - FIND", "InventoryFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.201.3", "Inventory - MOVE", "InventoryMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.201.4", "Inventory - GET", "InventoryGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.201.5", "Inventory Creation", "InventoryCreation", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.201.6", "Repository Query", "RepositoryQuery", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.3", "Ultrasound Multi-frame Image Storage (Retired)", "UltrasoundMultiFrameImageStorageRetired", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.3.1", "Ultrasound Multi-frame Image Storage", "UltrasoundMultiFrameImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.30", "Parametric Map Storage", "ParametricMapStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.4", "MR Image Storage", "MRImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.4.1", "Enhanced MR Image Storage", "EnhancedMRImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.4.2", "MR Spectroscopy Storage", "MRSpectroscopyStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.4.3", "Enhanced MR Color Image Storage", "EnhancedMRColorImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.4.4", "Legacy Converted Enhanced MR Image Storage", "LegacyConvertedEnhancedMRImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.1", "RT Image Storage", "RTImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.10", "RT Physician Intent Storage", "RTPhysicianIntentStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.11", "RT Segment Annotation Storage", "RTSegmentAnnotationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.12", "RT Radiation Set Storage", "RTRadiationSetStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.13", "C-Arm Photon-Electron Radiation Storage", "CArmPhotonElectronRadiationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.14", "Tomotherapeutic Radiation Storage", "TomotherapeuticRadiationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.15", "Robotic-Arm Radiation Storage", "RoboticArmRadiationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.16", "RT Radiation Record Set Storage", "RTRadiationRecordSetStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.17", "RT Radiation Salvage Record Storage", "RTRadiationSalvageRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.18", "Tomotherapeutic Radiation Record Storage", "TomotherapeuticRadiationRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.19", "C-Arm Photon-Electron Radiation Record Storage", "CArmPhotonElectronRadiationRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.2", "RT Dose Storage", "RTDoseStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.20", "Robotic Radiation Record Storage", "RoboticRadiationRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.21", "RT Radiation Set Delivery Instruction Storage", "RTRadiationSetDeliveryInstructionStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.22", "RT Treatment Preparation Storage", "RTTreatmentPreparationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.23", "Enhanced RT Image Storage", "EnhancedRTImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.24", "Enhanced Continuous RT Image Storage", "EnhancedContinuousRTImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.25", "RT Patient Position Acquisition Instruction Storage", "RTPatientPositionAcquisitionInstructionStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.3", "RT Structure Set Storage", "RTStructureSetStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.4", "RT Beams Treatment Record Storage", "RTBeamsTreatmentRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.5", "RT Plan Storage", "RTPlanStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.6", "RT Brachy Treatment Record Storage", "RTBrachyTreatmentRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.7", "RT Treatment Summary Record Storage", "RTTreatmentSummaryRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.8", "RT Ion Plan Storage", "RTIonPlanStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.481.9", "RT Ion Beams Treatment Record Storage", "RTIonBeamsTreatmentRecordStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.5", "Nuclear Medicine Image Storage (Retired)", "NuclearMedicineImageStorageRetired", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.501.1", "DICOS CT Image Storage", "DICOSCTImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.501.2.1", "DICOS Digital X-Ray Image Storage - For Presentation", "DICOSDigitalXRayImageStorageForPresentation", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.501.2.2", "DICOS Digital X-Ray Image Storage - For Processing", "DICOSDigitalXRayImageStorageForProcessing", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.501.3", "DICOS Threat Detection Report Storage", "DICOSThreatDetectionReportStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.501.4", "DICOS 2D AIT Storage", "DICOS2DAITStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.501.5", "DICOS 3D AIT Storage", "DICOS3DAITStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.501.6", "DICOS Quadrupole Resonance (QR) Storage", "DICOSQuadrupoleResonanceStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.6", "Ultrasound Image Storage (Retired)", "UltrasoundImageStorageRetired", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.6.1", "Ultrasound Image Storage", "UltrasoundImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.6.2", "Enhanced US Volume Storage", "EnhancedUSVolumeStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.601.1", "Eddy Current Image Storage", "EddyCurrentImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.601.2", "Eddy Current Multi-frame Image Storage", "EddyCurrentMultiFrameImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.66", "Raw Data Storage", "RawDataStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.66.1", "Spatial Registration Storage", "SpatialRegistrationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.66.2", "Spatial Fiducials Storage", "SpatialFiducialsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.66.3", "Deformable Spatial Registration Storage", "DeformableSpatialRegistrationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.66.4", "Segmentation Storage", "SegmentationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.66.5", "Surface Segmentation Storage", "SurfaceSegmentationStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.66.6", "Tractography Results Storage", "TractographyResultsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.67", "Real World Value Mapping Storage", "RealWorldValueMappingStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.68.1", "Surface Scan Mesh Storage", "SurfaceScanMeshStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.68.2", "Surface Scan Point Cloud Storage", "SurfaceScanPointCloudStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.7", "Secondary Capture Image Storage", "SecondaryCaptureImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.7.1", "Multi-frame Single Bit Secondary Capture Image Storage", "MultiFrameSingleBitSecondaryCaptureImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.7.2", "Multi-frame Grayscale Byte Secondary Capture Image Storage", "MultiFrameGrayscaleByteSecondaryCaptureImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.7.3", "Multi-frame Grayscale Word Secondary Capture Image Storage", "MultiFrameGrayscaleWordSecondaryCaptureImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.7.4", "Multi-frame True Color Secondary Capture Image Storage", "MultiFrameTrueColorSecondaryCaptureImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1", "VL Image Storage - Trial (Retired)", "VLImageStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.1", "VL Endoscopic Image Storage", "VLEndoscopicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.1.1", "Video Endoscopic Image Storage", "VideoEndoscopicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.2", "VL Microscopic Image Storage", "VLMicroscopicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.2.1", "Video Microscopic Image Storage", "VideoMicroscopicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.3", "VL Slide-Coordinates Microscopic Image Storage", "VLSlideCoordinatesMicroscopicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.4", "VL Photographic Image Storage", "VLPhotographicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.4.1", "Video Photographic Image Storage", "VideoPhotographicImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.1", "Ophthalmic Photography 8 Bit Image Storage", "OphthalmicPhotography8BitImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.2", "Ophthalmic Photography 16 Bit Image Storage", "OphthalmicPhotography16BitImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.3", "Stereometric Relationship Storage", "StereometricRelationshipStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.4", "Ophthalmic Tomography Image Storage", "OphthalmicTomographyImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.5", "Wide Field Ophthalmic Photography Stereographic Projection Image Storage", "WideFieldOphthalmicPhotographyStereographicProjectionImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.6", "Wide Field Ophthalmic Photography 3D Coordinates Image Storage", "WideFieldOphthalmicPhotography3DCoordinatesImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.7", "Ophthalmic Optical Coherence Tomography En Face Image Storage", "OphthalmicOpticalCoherenceTomographyEnFaceImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.5.8", "Ophthalmic Optical Coherence Tomography B-scan Volume Analysis Storage", "OphthalmicOpticalCoherenceTomographyBscanVolumeAnalysisStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.6", "VL Whole Slide Microscopy Image Storage", "VLWholeSlideMicroscopyImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.1.7", "Dermoscopic Photography Image Storage", "DermoscopicPhotographyImageStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.77.2", "VL Multi-frame Image Storage - Trial (Retired)", "VLMultiFrameImageStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.78.1", "Lensometry Measurements Storage", "LensometryMeasurementsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.78.2", "Autorefraction Measurements Storage", "AutorefractionMeasurementsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.78.3", "Keratometry Measurements Storage", "KeratometryMeasurementsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.78.4", "Subjective Refraction Measurements Storage", "SubjectiveRefractionMeasurementsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.78.5", "Visual Acuity Measurements Storage", "VisualAcuityMeasurementsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.78.6", "Spectacle Prescription Report Storage", "SpectaclePrescriptionReportStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.78.7", "Ophthalmic Axial Measurements Storage", "OphthalmicAxialMeasurementsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.78.8", "Intraocular Lens Calculations Storage", "IntraocularLensCalculationsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.79.1", "Macular Grid Thickness and Volume Report Storage", "MacularGridThicknessAndVolumeReportStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.8", "Standalone Overlay Storage (Retired)", "StandaloneOverlayStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.80.1", "Ophthalmic Visual Field Static Perimetry Measurements Storage", "OphthalmicVisualFieldStaticPerimetryMeasurementsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.81.1", "Ophthalmic Thickness Map Storage", "OphthalmicThicknessMapStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.82.1", "Corneal Topography Map Storage", "CornealTopographyMapStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.1", "Text SR Storage - Trial (Retired)", "TextSRStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.88.11", "Basic Text SR Storage", "BasicTextSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.2", "Audio SR Storage - Trial (Retired)", "AudioSRStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.88.22", "Enhanced SR Storage", "EnhancedSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.3", "Detail SR Storage - Trial (Retired)", "DetailSRStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.88.33", "Comprehensive SR Storage", "ComprehensiveSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.34", "Comprehensive 3D SR Storage", "Comprehensive3DSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.35", "Extensible SR Storage", "ExtensibleSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.4", "Comprehensive SR Storage - Trial (Retired)", "ComprehensiveSRStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.88.40", "Procedure Log Storage", "ProcedureLogStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.50", "Mammography CAD SR Storage", "MammographyCADSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.59", "Key Object Selection Document Storage", "KeyObjectSelectionDocumentStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.65", "Chest CAD SR Storage", "ChestCADSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.67", "X-Ray Radiation Dose SR Storage", "XRayRadiationDoseSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.68", "Radiopharmaceutical Radiation Dose SR Storage", "RadiopharmaceuticalRadiationDoseSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.69", "Colon CAD SR Storage", "ColonCADSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.70", "Implantation Plan SR Storage", "ImplantationPlanSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.71", "Acquisition Context SR Storage", "AcquisitionContextSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.72", "Simplified Adult Echo SR Storage", "SimplifiedAdultEchoSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.73", "Patient Radiation Dose SR Storage", "PatientRadiationDoseSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.74", "Planned Imaging Agent Administration SR Storage", "PlannedImagingAgentAdministrationSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.75", "Performed Imaging Agent Administration SR Storage", "PerformedImagingAgentAdministrationSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.88.76", "Enhanced X-Ray Radiation Dose SR Storage", "EnhancedXRayRadiationDoseSRStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9", "Standalone Curve Storage (Retired)", "StandaloneCurveStorage", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.9.1", "Waveform Storage - Trial (Retired)", "WaveformStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.1.9.1.1", "12-lead ECG Waveform Storage", "TwelveLeadECGWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.1.2", "General ECG Waveform Storage", "GeneralECGWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.1.3", "Ambulatory ECG Waveform Storage", "AmbulatoryECGWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.2.1", "Hemodynamic Waveform Storage", "HemodynamicWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.3.1", "Cardiac Electrophysiology Waveform Storage", "CardiacElectrophysiologyWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.4.1", "Basic Voice Audio Waveform Storage", "BasicVoiceAudioWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.4.2", "General Audio Waveform Storage", "GeneralAudioWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.5.1", "Arterial Pulse Waveform Storage", "ArterialPulseWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.6.1", "Respiratory Waveform Storage", "RespiratoryWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.6.2", "Multi-channel Respiratory Waveform Storage", "MultichannelRespiratoryWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.7.1", "Routine Scalp Electroencephalogram Waveform Storage", "RoutineScalpElectroencephalogramWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.7.2", "Electromyogram Waveform Storage", "ElectromyogramWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.7.3", "Electrooculogram Waveform Storage", "ElectrooculogramWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.7.4", "Sleep Electroencephalogram Waveform Storage", "SleepElectroencephalogramWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.9.8.1", "Body Position Waveform Storage", "BodyPositionWaveformStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.90.1", "Content Assessment Results Storage", "ContentAssessmentResultsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.1.91.1", "Microscopy Bulk Simple Annotations Storage", "MicroscopyBulkSimpleAnnotationsStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.1.1", "Patient Root Query/Retrieve Information Model - FIND", "PatientRootQueryRetrieveInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.1.2", "Patient Root Query/Retrieve Information Model - MOVE", "PatientRootQueryRetrieveInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.1.3", "Patient Root Query/Retrieve Information Model - GET", "PatientRootQueryRetrieveInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.2.1", "Study Root Query/Retrieve Information Model - FIND", "StudyRootQueryRetrieveInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.2.2", "Study Root Query/Retrieve Information Model - MOVE", "StudyRootQueryRetrieveInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.2.3", "Study Root Query/Retrieve Information Model - GET", "StudyRootQueryRetrieveInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.3.1", "Patient/Study Only Query/Retrieve Information Model - FIND (Retired)", "PatientStudyOnlyQueryRetrieveInformationModelFind", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.2.3.2", "Patient/Study Only Query/Retrieve Information Model - MOVE (Retired)", "PatientStudyOnlyQueryRetrieveInformationModelMove", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.2.3.3", "Patient/Study Only Query/Retrieve Information Model - GET (Retired)", "PatientStudyOnlyQueryRetrieveInformationModelGet", SopClass, true),
    E::new("1.2.840.10008.5.1.4.1.2.4.2", "Composite Instance Root Retrieve - MOVE", "CompositeInstanceRootRetrieveMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.4.3", "Composite Instance Root Retrieve - GET", "CompositeInstanceRootRetrieveGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.1.2.5.3", "Composite Instance Retrieve Without Bulk Data - GET", "CompositeInstanceRetrieveWithoutBulkDataGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.20.1", "Defined Procedure Protocol Information Model - FIND", "DefinedProcedureProtocolInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.20.2", "Defined Procedure Protocol Information Model - MOVE", "DefinedProcedureProtocolInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.20.3", "Defined Procedure Protocol Information Model - GET", "DefinedProcedureProtocolInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.31", "Modality Worklist Information Model - FIND", "ModalityWorklistInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.32.1", "General Purpose Worklist Information Model - FIND (Retired)", "GeneralPurposeWorklistInformationModelFind", SopClass, true),
    E::new("1.2.840.10008.5.1.4.32.2", "General Purpose Scheduled Procedure Step SOP Class (Retired)", "GeneralPurposeScheduledProcedureStep", SopClass, true),
    E::new("1.2.840.10008.5.1.4.32.3", "General Purpose Performed Procedure Step SOP Class (Retired)", "GeneralPurposePerformedProcedureStep", SopClass, true),
    E::new("1.2.840.10008.5.1.4.33", "Instance Availability Notification SOP Class", "InstanceAvailabilityNotification", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.1", "RT Beams Delivery Instruction Storage - Trial (Retired)", "RTBeamsDeliveryInstructionStorageTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.34.10", "RT Brachy Application Setup Delivery Instruction Storage", "RTBrachyApplicationSetupDeliveryInstructionStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.2", "RT Conventional Machine Verification - Trial (Retired)", "RTConventionalMachineVerificationTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.34.3", "RT Ion Machine Verification - Trial (Retired)", "RTIonMachineVerificationTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.34.4.1", "Unified Procedure Step - Push SOP Class - Trial (Retired)", "UnifiedProcedureStepPushTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.34.4.2", "Unified Procedure Step - Watch SOP Class - Trial (Retired)", "UnifiedProcedureStepWatchTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.34.4.3", "Unified Procedure Step - Pull SOP Class - Trial (Retired)", "UnifiedProcedureStepPullTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.34.4.4", "Unified Procedure Step - Event SOP Class - Trial (Retired)", "UnifiedProcedureStepEventTrial", SopClass, true),
    E::new("1.2.840.10008.5.1.4.34.6.1", "Unified Procedure Step - Push SOP Class", "UnifiedProcedureStepPush", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.6.2", "Unified Procedure Step - Watch SOP Class", "UnifiedProcedureStepWatch", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.6.3", "Unified Procedure Step - Pull SOP Class", "UnifiedProcedureStepPull", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.6.4", "Unified Procedure Step - Event SOP Class", "UnifiedProcedureStepEvent", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.6.5", "Unified Procedure Step - Query SOP Class", "UnifiedProcedureStepQuery", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.7", "RT Beams Delivery Instruction Storage", "RTBeamsDeliveryInstructionStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.8", "RT Conventional Machine Verification", "RTConventionalMachineVerification", SopClass, false),
    E::new("1.2.840.10008.5.1.4.34.9", "RT Ion Machine Verification", "RTIonMachineVerification", SopClass, false),
    E::new("1.2.840.10008.5.1.4.37.1", "General Relevant Patient Information Query", "GeneralRelevantPatientInformationQuery", SopClass, false),
    E::new("1.2.840.10008.5.1.4.37.2", "Breast Imaging Relevant Patient Information Query", "BreastImagingRelevantPatientInformationQuery", SopClass, false),
    E::new("1.2.840.10008.5.1.4.37.3", "Cardiac Relevant Patient Information Query", "CardiacRelevantPatientInformationQuery", SopClass, false),
    E::new("1.2.840.10008.5.1.4.38.1", "Hanging Protocol Storage", "HangingProtocolStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.38.2", "Hanging Protocol Information Model - FIND", "HangingProtocolInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.38.3", "Hanging Protocol Information Model - MOVE", "HangingProtocolInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.38.4", "Hanging Protocol Information Model - GET", "HangingProtocolInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.39.1", "Color Palette Storage", "ColorPaletteStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.39.2", "Color Palette Query/Retrieve Information Model - FIND", "ColorPaletteQueryRetrieveInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.39.3", "Color Palette Query/Retrieve Information Model - MOVE", "ColorPaletteQueryRetrieveInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.39.4", "Color Palette Query/Retrieve Information Model - GET", "ColorPaletteQueryRetrieveInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.41", "Product Characteristics Query SOP Class", "ProductCharacteristicsQuery", SopClass, false),
    E::new("1.2.840.10008.5.1.4.42", "Substance Approval Query SOP Class", "SubstanceApprovalQuery", SopClass, false),
    E::new("1.2.840.10008.5.1.4.43.1", "Generic Implant Template Storage", "GenericImplantTemplateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.43.2", "Generic Implant Template Information Model - FIND", "GenericImplantTemplateInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.43.3", "Generic Implant Template Information Model - MOVE", "GenericImplantTemplateInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.43.4", "Generic Implant Template Information Model - GET", "GenericImplantTemplateInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.44.1", "Implant Assembly Template Storage", "ImplantAssemblyTemplateStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.44.2", "Implant Assembly Template Information Model - FIND", "ImplantAssemblyTemplateInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.44.3", "Implant Assembly Template Information Model - MOVE", "ImplantAssemblyTemplateInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.44.4", "Implant Assembly Template Information Model - GET", "ImplantAssemblyTemplateInformationModelGet", SopClass, false),
    E::new("1.2.840.10008.5.1.4.45.1", "Implant Template Group Storage", "ImplantTemplateGroupStorage", SopClass, false),
    E::new("1.2.840.10008.5.1.4.45.2", "Implant Template Group Information Model - FIND", "ImplantTemplateGroupInformationModelFind", SopClass, false),
    E::new("1.2.840.10008.5.1.4.45.3", "Implant Template Group Information Model - MOVE", "ImplantTemplateGroupInformationModelMove", SopClass, false),
    E::new("1.2.840.10008.5.1.4.45.4", "Implant Template Group Information Model - GET", "ImplantTemplateGroupInformationModelGet", SopClass, false),
];

#[rustfmt::skip]
#[cfg(feature = "transfer-syntax")]
pub(crate) const TRANSFER_SYNTAXES: &[E] = &[
    E::new("1.2.840.10008.1.2", "Implicit VR Little Endian: Default Transfer Syntax for DICOM", "ImplicitVRLittleEndian", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.1", "Explicit VR Little Endian", "ExplicitVRLittleEndian", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.1.98", "Encapsulated Uncompressed Explicit VR Little Endian", "EncapsulatedUncompressedExplicitVRLittleEndian", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.1.99", "Deflated Explicit VR Little Endian", "DeflatedExplicitVRLittleEndian", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.2", "Explicit VR Big Endian (Retired)", "ExplicitVRBigEndian", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.100", "MPEG2 Main Profile / Main Level", "MPEG2MPML", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.100.1", "Fragmentable MPEG2 Main Profile / Main Level", "MPEG2MPMLF", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.101", "MPEG2 Main Profile / High Level", "MPEG2MPHL", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.101.1", "Fragmentable MPEG2 Main Profile / High Level", "MPEG2MPHLF", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.102", "MPEG-4 AVC/H.264 High Profile / Level 4.1", "MPEG4HP41", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.102.1", "Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.1", "MPEG4HP41F", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.103", "MPEG-4 AVC/H.264 BD-compatible High Profile / Level 4.1", "MPEG4HP41BD", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.103.1", "Fragmentable MPEG-4 AVC/H.264 BD-compatible High Profile / Level 4.1", "MPEG4HP41BDF", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.104", "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video", "MPEG4HP422D", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.104.1", "Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 2D Video", "MPEG4HP422DF", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.105", "MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video", "MPEG4HP423D", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.105.1", "Fragmentable MPEG-4 AVC/H.264 High Profile / Level 4.2 For 3D Video", "MPEG4HP423DF", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.106", "MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2", "MPEG4HP42STEREO", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.106.1", "Fragmentable MPEG-4 AVC/H.264 Stereo High Profile / Level 4.2", "MPEG4HP42STEREOF", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.107", "HEVC/H.265 Main Profile / Level 5.1", "HEVCMP51", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.108", "HEVC/H.265 Main 10 Profile / Level 5.1", "HEVCM10P51", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.50", "JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression", "JPEGBaseline8Bit", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.51", "JPEG Extended (Process 2", "JPEGExtended12Bit", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.52", "JPEG Extended (Process 3", "JPEGExtended35", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.53", "JPEG Spectral Selection, Non-Hierarchical (Process 6", "JPEGSpectralSelectionNonHierarchical68", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.54", "JPEG Spectral Selection, Non-Hierarchical (Process 7", "JPEGSpectralSelectionNonHierarchical79", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.55", "JPEG Full Progression, Non-Hierarchical (Process 10", "JPEGFullProgressionNonHierarchical1012", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.56", "JPEG Full Progression, Non-Hierarchical (Process 11", "JPEGFullProgressionNonHierarchical1113", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.57", "JPEG Lossless, Non-Hierarchical (Process 14)", "JPEGLossless", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.58", "JPEG Lossless, Non-Hierarchical (Process 15) (Retired)", "JPEGLosslessNonHierarchical15", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.59", "JPEG Extended, Hierarchical (Process 16", "JPEGExtendedHierarchical1618", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.60", "JPEG Extended, Hierarchical (Process 17", "JPEGExtendedHierarchical1719", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.61", "JPEG Spectral Selection, Hierarchical (Process 20", "JPEGSpectralSelectionHierarchical2022", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.62", "JPEG Spectral Selection, Hierarchical (Process 21", "JPEGSpectralSelectionHierarchical2123", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.63", "JPEG Full Progression, Hierarchical (Process 24", "JPEGFullProgressionHierarchical2426", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.64", "JPEG Full Progression, Hierarchical (Process 25", "JPEGFullProgressionHierarchical2527", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.65", "JPEG Lossless, Hierarchical (Process 28) (Retired)", "JPEGLosslessHierarchical28", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.66", "JPEG Lossless, Hierarchical (Process 29) (Retired)", "JPEGLosslessHierarchical29", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.4.70", "JPEG Lossless, Non-Hierarchical, First-Order Prediction (Process 14 [Selection Value 1]): Default Transfer Syntax for Lossless JPEG Image Compression", "JPEGLosslessSV1", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.80", "JPEG-LS Lossless Image Compression", "JPEGLSLossless", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.81", "JPEG-LS Lossy (Near-Lossless) Image Compression", "JPEGLSNearLossless", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.90", "JPEG 2000 Image Compression (Lossless Only)", "JPEG2000Lossless", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.91", "JPEG 2000 Image Compression", "JPEG2000", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.92", "JPEG 2000 Part 2 Multi-component Image Compression (Lossless Only)", "JPEG2000MCLossless", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.93", "JPEG 2000 Part 2 Multi-component Image Compression", "JPEG2000MC", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.94", "JPIP Referenced", "JPIPReferenced", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.4.95", "JPIP Referenced Deflate", "JPIPReferencedDeflate", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.5", "RLE Lossless", "RLELossless", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.6.1", "RFC 2557 MIME encapsulation (Retired)", "RFC2557MIMEEncapsulation", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.6.2", "XML Encoding (Retired)", "XMLEncoding", TransferSyntax, true),
    E::new("1.2.840.10008.1.2.7.1", "SMPTE ST 2110-20 Uncompressed Progressive Active Video", "SMPTEST211020UncompressedProgressiveActiveVideo", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.7.2", "SMPTE ST 2110-20 Uncompressed Interlaced Active Video", "SMPTEST211020UncompressedInterlacedActiveVideo", TransferSyntax, false),
    E::new("1.2.840.10008.1.2.7.3", "SMPTE ST 2110-30 PCM Digital Audio", "SMPTEST211030PCMDigitalAudio", TransferSyntax, false),
    E::new("1.2.840.10008.1.20", "Papyrus 3 Implicit VR Little Endian (Retired)", "Papyrus3ImplicitVRLittleEndian", TransferSyntax, true),
];

#[rustfmt::skip]
#[cfg(feature = "meta-sop-class")]
pub(crate) const META_SOP_CLASSES: &[E] = &[
    E::new("1.2.840.10008.3.1.2.1.4", "Detached Patient Management Meta SOP Class (Retired)", "DetachedPatientManagementMeta", MetaSopClass, true),
    E::new("1.2.840.10008.3.1.2.5.4", "Detached Results Management Meta SOP Class (Retired)", "DetachedResultsManagementMeta", MetaSopClass, true),
    E::new("1.2.840.10008.3.1.2.5.5", "Detached Study Management Meta SOP Class (Retired)", "DetachedStudyManagementMeta", MetaSopClass, true),
    E::new("1.2.840.10008.5.1.1.18", "Basic Color Print Management Meta SOP Class", "BasicColorPrintManagementMeta", MetaSopClass, false),
    E::new("1.2.840.10008.5.1.1.18.1", "Referenced Color Print Management Meta SOP Class (Retired)", "ReferencedColorPrintManagementMeta", MetaSopClass, true),
    E::new("1.2.840.10008.5.1.1.32", "Pull Stored Print Management Meta SOP Class (Retired)", "PullStoredPrintManagementMeta", MetaSopClass, true),
    E::new("1.2.840.10008.5.1.1.9", "Basic Grayscale Print Management Meta SOP Class", "BasicGrayscalePrintManagementMeta", MetaSopClass, false),
    E::new("1.2.840.10008.5.1.1.9.1", "Referenced Grayscale Print Management Meta SOP Class (Retired)", "ReferencedGrayscalePrintManagementMeta", MetaSopClass, true),
    E::new("1.2.840.10008.5.1.4.32", "General Purpose Worklist Management Meta SOP Class (Retired)", "GeneralPurposeWorklistManagementMeta", MetaSopClass, true),
];

#[rustfmt::skip]
#[cfg(feature = "well-known-sop-instance")]
pub(crate) const WELL_KNOWN_SOP_INSTANCES: &[E] = &[
    E::new("1.2.840.10008.1.20.1.1", "Storage Commitment Push Model SOP Instance", "StorageCommitmentPushModelInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.20.2.1", "Storage Commitment Pull Model SOP Instance (Retired)", "StorageCommitmentPullModelInstance", WellKnownSopInstance, true),
    E::new("1.2.840.10008.1.40.1", "Procedural Event Logging SOP Instance", "ProceduralEventLoggingInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.42.1", "Substance Administration Logging SOP Instance", "SubstanceAdministrationLoggingInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.1", "Hot Iron Color Palette SOP Instance", "HotIronPalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.2", "PET Color Palette SOP Instance", "PETPalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.3", "Hot Metal Blue Color Palette SOP Instance", "HotMetalBluePalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.4", "PET 20 Step Color Palette SOP Instance", "PET20StepPalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.5", "Spring Color Palette SOP Instance", "SpringPalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.6", "Summer Color Palette SOP Instance", "SummerPalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.7", "Fall Color Palette SOP Instance", "FallPalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.1.5.8", "Winter Color Palette SOP Instance", "WinterPalette", WellKnownSopInstance, false),
    E::new("1.2.840.10008.5.1.1.17", "Printer SOP Instance", "PrinterInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.5.1.1.17.376", "Printer Configuration Retrieval SOP Instance", "PrinterConfigurationRetrievalInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.5.1.1.25", "Print Queue SOP Instance (Retired)", "PrintQueueInstance", WellKnownSopInstance, true),
    E::new("1.2.840.10008.5.1.1.40.1", "Display System SOP Instance", "DisplaySystemInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.5.1.4.1.1.201.1.1", "Storage Management SOP Instance", "StorageManagementInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.5.1.4.34.5", "UPS Global Subscription SOP Instance", "UPSGlobalSubscriptionInstance", WellKnownSopInstance, false),
    E::new("1.2.840.10008.5.1.4.34.5.1", "UPS Filtered Global Subscription SOP Instance", "UPSFilteredGlobalSubscriptionInstance", WellKnownSopInstance, false),
];

#[rustfmt::skip]
#[cfg(feature = "dicom-uid-as-coding-scheme")]
pub(crate) const DICOM_UIDS_AS_CODING_SCHEMES: &[E] = &[
    E::new("1.2.840.10008.2.6.1", "DICOM UID Registry", "DCMUID", DicomUidsAsCodingScheme, false),
];

#[rustfmt::skip]
#[cfg(feature = "coding-scheme")]
pub(crate) const CODING_SCHEMES: &[E] = &[
    E::new("1.2.840.10008.2.16.10", "Dublin Core", "DC", CodingScheme, false),
    E::new("1.2.840.10008.2.16.11", "New York University Melanoma Clinical Cooperative Group", "NYUMCCG", CodingScheme, false),
    E::new("1.2.840.10008.2.16.12", "Mayo Clinic Non-radiological Images Specific Body Structure Anatomical Surface Region Guide", "MAYONRISBSASRG", CodingScheme, false),
    E::new("1.2.840.10008.2.16.13", "Image Biomarker Standardisation Initiative", "IBSI", CodingScheme, false),
    E::new("1.2.840.10008.2.16.14", "Radiomics Ontology", "RO", CodingScheme, false),
    E::new("1.2.840.10008.2.16.15", "RadElement", "RADELEMENT", CodingScheme, false),
    E::new("1.2.840.10008.2.16.16", "ICD-11", "I11", CodingScheme, false),
    E::new("1.2.840.10008.2.16.17", "Unified numbering system (UNS) for metals and alloys", "UNS", CodingScheme, false),
    E::new("1.2.840.10008.2.16.18", "Research Resource Identification", "RRID", CodingScheme, false),
    E::new("1.2.840.10008.2.16.4", "DICOM Controlled Terminology", "DCM", CodingScheme, false),
    E::new("1.2.840.10008.2.16.5", "Adult Mouse Anatomy Ontology", "MA", CodingScheme, false),
    E::new("1.2.840.10008.2.16.6", "Uberon Ontology", "UBERON", CodingScheme, false),
    E::new("1.2.840.10008.2.16.7", "Integrated Taxonomic Information System (ITIS) Taxonomic Serial Number (TSN)", "ITIS_TSN", CodingScheme, false),
    E::new("1.2.840.10008.2.16.8", "Mouse Genome Initiative (MGI)", "MGI", CodingScheme, false),
    E::new("1.2.840.10008.2.16.9", "PubChem Compound CID", "PUBCHEM_CID", CodingScheme, false),
];

#[rustfmt::skip]
#[cfg(feature = "application-context-name")]
pub(crate) const APPLICATION_CONTEXT_NAMES: &[E] = &[
    E::new("1.2.840.10008.3.1.1.1", "DICOM Application Context Name", "DICOMApplicationContext", ApplicationContextName, false),
];

#[rustfmt::skip]
#[cfg(feature = "service-class")]
pub(crate) const SERVICE_CLASSES: &[E] = &[
    E::new("1.2.840.10008.4.2", "Storage Service Class", "Storage", ServiceClass, false),
    E::new("1.2.840.10008.5.1.4.34.4", "Unified Worklist and Procedure Step Service Class - Trial (Retired)", "UnifiedWorklistAndProcedureStepTrial", ServiceClass, true),
    E::new("1.2.840.10008.5.1.4.34.6", "Unified Worklist and Procedure Step Service Class", "UnifiedWorklistAndProcedureStep", ServiceClass, false),
];

#[rustfmt::skip]
#[cfg(feature = "application-hosting-model")]
pub(crate) const APPLICATION_HOSTING_MODELS: &[E] = &[
    E::new("1.2.840.10008.7.1.1", "Native DICOM Model", "NativeDICOMModel", ApplicationHostingModel, false),
    E::new("1.2.840.10008.7.1.2", "Abstract Multi-Dimensional Image Model", "AbstractMultiDimensionalImageModel", ApplicationHostingModel, false),
];

#[rustfmt::skip]
#[cfg(feature = "mapping-resource")]
pub(crate) const MAPPING_RESOURCES: &[E] = &[
    E::new("1.2.840.10008.8.1.1", "DICOM Content Mapping Resource", "DICOMContentMappingResource", MappingResource, false),
];

#[rustfmt::skip]
#[cfg(feature = "ldap-oid")]
pub(crate) const LDAP_OIDS: &[E] = &[
    E::new("1.2.840.10008.15.0.3.1", "dicomDeviceName", "dicomDeviceName", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.10", "dicomAssociationInitiator", "dicomAssociationInitiator", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.11", "dicomAssociationAcceptor", "dicomAssociationAcceptor", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.12", "dicomHostname", "dicomHostname", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.13", "dicomPort", "dicomPort", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.14", "dicomSOPClass", "dicomSOPClass", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.15", "dicomTransferRole", "dicomTransferRole", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.16", "dicomTransferSyntax", "dicomTransferSyntax", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.17", "dicomPrimaryDeviceType", "dicomPrimaryDeviceType", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.18", "dicomRelatedDeviceReference", "dicomRelatedDeviceReference", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.19", "dicomPreferredCalledAETitle", "dicomPreferredCalledAETitle", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.2", "dicomDescription", "dicomDescription", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.20", "dicomTLSCyphersuite", "dicomTLSCyphersuite", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.21", "dicomAuthorizedNodeCertificateReference", "dicomAuthorizedNodeCertificateReference", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.22", "dicomThisNodeCertificateReference", "dicomThisNodeCertificateReference", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.23", "dicomInstalled", "dicomInstalled", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.24", "dicomStationName", "dicomStationName", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.25", "dicomDeviceSerialNumber", "dicomDeviceSerialNumber", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.26", "dicomInstitutionName", "dicomInstitutionName", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.27", "dicomInstitutionAddress", "dicomInstitutionAddress", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.28", "dicomInstitutionDepartmentName", "dicomInstitutionDepartmentName", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.29", "dicomIssuerOfPatientID", "dicomIssuerOfPatientID", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.3", "dicomManufacturer", "dicomManufacturer", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.30", "dicomPreferredCallingAETitle", "dicomPreferredCallingAETitle", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.31", "dicomSupportedCharacterSet", "dicomSupportedCharacterSet", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.4", "dicomManufacturerModelName", "dicomManufacturerModelName", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.5", "dicomSoftwareVersion", "dicomSoftwareVersion", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.6", "dicomVendorData", "dicomVendorData", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.7", "dicomAETitle", "dicomAETitle", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.8", "dicomNetworkConnectionReference", "dicomNetworkConnectionReference", LdapOid, false),
    E::new("1.2.840.10008.15.0.3.9", "dicomApplicationCluster", "dicomApplicationCluster", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.1", "dicomConfigurationRoot", "dicomConfigurationRoot", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.2", "dicomDevicesRoot", "dicomDevicesRoot", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.3", "dicomUniqueAETitlesRegistryRoot", "dicomUniqueAETitlesRegistryRoot", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.4", "dicomDevice", "dicomDevice", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.5", "dicomNetworkAE", "dicomNetworkAE", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.6", "dicomNetworkConnection", "dicomNetworkConnection", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.7", "dicomUniqueAETitle", "dicomUniqueAETitle", LdapOid, false),
    E::new("1.2.840.10008.15.0.4.8", "dicomTransferCapability", "dicomTransferCapability", LdapOid, false),
];

#[rustfmt::skip]
#[cfg(feature = "synchronization-frame-of-reference")]
pub(crate) const SYNCHRONIZATION_FRAME_OF_REFERENCES: &[E] = &[
    E::new("1.2.840.10008.15.1.1", "Universal Coordinated Time", "UTC", SynchronizationFrameOfReference, false),
];
