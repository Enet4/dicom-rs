//! Automatically generated. DO NOT EDIT!

use dictionary::DictionaryEntry;
use data::{Tag, VR};

type E<'a> = DictionaryEntry<'a>;

pub const ENTRIES: &'static [E<'static>] = &[
    E { tag: Tag(0x0008, 0x0005), alias: "SpecificCharacterSet", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0006), alias: "LanguageCodeSequence", vr: VR::SQ },
    E { tag: Tag(0x0008, 0x0008), alias: "ImageType", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0012), alias: "InstanceCreationDate", vr: VR::DA },
    E { tag: Tag(0x0008, 0x0013), alias: "InstanceCreationTime", vr: VR::TM },
    E { tag: Tag(0x0008, 0x0014), alias: "InstanceCreatorUID", vr: VR::UI },
    E { tag: Tag(0x0008, 0x0015), alias: "InstanceCoercionDateTime", vr: VR::DT },
    E { tag: Tag(0x0008, 0x0016), alias: "SOPClassUID", vr: VR::UI },
    E { tag: Tag(0x0008, 0x0018), alias: "SOPInstanceUID", vr: VR::UI },
    E { tag: Tag(0x0008, 0x001A), alias: "RelatedGeneralSOPClassUID", vr: VR::UI },
    E { tag: Tag(0x0008, 0x001B), alias: "OriginalSpecializedSOPClassUID", vr: VR::UI },
    E { tag: Tag(0x0008, 0x0020), alias: "StudyDate", vr: VR::DA },
    E { tag: Tag(0x0008, 0x0021), alias: "SeriesDate", vr: VR::DA },
    E { tag: Tag(0x0008, 0x0022), alias: "AcquisitionDate", vr: VR::DA },
    E { tag: Tag(0x0008, 0x0023), alias: "ContentDate", vr: VR::DA },
    E { tag: Tag(0x0008, 0x002A), alias: "AcquisitionDateTime", vr: VR::DT },
    E { tag: Tag(0x0008, 0x0030), alias: "StudyTime", vr: VR::TM },
    E { tag: Tag(0x0008, 0x0031), alias: "SeriesTime", vr: VR::TM },
    E { tag: Tag(0x0008, 0x0032), alias: "AcquisitionTime", vr: VR::TM },
    E { tag: Tag(0x0008, 0x0033), alias: "ContentTime", vr: VR::TM },
    E { tag: Tag(0x0008, 0x0050), alias: "AccessionNumber", vr: VR::SH },
    E { tag: Tag(0x0008, 0x0051), alias: "IssuerOfAccessionNumberSequence", vr: VR::SQ },
    E { tag: Tag(0x0008, 0x0052), alias: "QueryRetrieveLevel", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0053), alias: "QueryRetrieveView", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0054), alias: "RetrieveAETitle", vr: VR::AE },
    E { tag: Tag(0x0008, 0x0055), alias: "StationAETitle", vr: VR::AE },
    E { tag: Tag(0x0008, 0x0056), alias: "InstanceAvailability", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0058), alias: "FailedSOPInstanceUIDList", vr: VR::UI },
    E { tag: Tag(0x0008, 0x0060), alias: "Modality", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0061), alias: "ModalitiesInStudy", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0062), alias: "SOPClassesInStudy", vr: VR::UI },
    E { tag: Tag(0x0008, 0x0064), alias: "ConversionType", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0068), alias: "PresentationIntentType", vr: VR::CS },
    E { tag: Tag(0x0008, 0x0070), alias: "Manufacturer", vr: VR::LO },
    E { tag: Tag(0x0008, 0x0080), alias: "InstitutionName", vr: VR::LO },
    E { tag: Tag(0x0008, 0x0081), alias: "InstitutionAddress", vr: VR::ST },
    E { tag: Tag(0x0008, 0x0082), alias: "InstitutionCodeSequence", vr: VR::SQ },
    E { tag: Tag(0x4010, 0x1079), alias: "AnomalyLocatorIndicatorSequence", vr: VR::SQ }, // DICOS
    E { tag: Tag(0x4010, 0x107A), alias: "AnomalyLocatorIndicator", vr: VR::FL }, // DICOS
    E { tag: Tag(0x4010, 0x107B), alias: "PTORegionSequence", vr: VR::SQ }, // DICOS
    E { tag: Tag(0x4010, 0x107C), alias: "InspectionSelectionCriteria", vr: VR::CS }, // DICOS
    E { tag: Tag(0x4010, 0x107D), alias: "SecondaryInspectionMethodSequence", vr: VR::SQ }, // DICOS
    E { tag: Tag(0x4010, 0x107E), alias: "PRCSToRCSOrientation", vr: VR::DS }, // DICOS
    E { tag: Tag(0x4FFE, 0x0001), alias: "MACParametersSequence", vr: VR::SQ },
    E { tag: Tag(0x5200, 0x9229), alias: "SharedFunctionalGroupsSequence", vr: VR::SQ },
    E { tag: Tag(0x5200, 0x9230), alias: "PerFrameFunctionalGroupsSequence", vr: VR::SQ },
    E { tag: Tag(0x5400, 0x0100), alias: "WaveformSequence", vr: VR::SQ },
    E { tag: Tag(0x5400, 0x0110), alias: "ChannelMinimumValue", vr: VR::OB/* or  or OW */ },
    E { tag: Tag(0x5400, 0x0112), alias: "ChannelMaximumValue", vr: VR::OB/* or  or OW */ },
    E { tag: Tag(0x5400, 0x1004), alias: "WaveformBitsAllocated", vr: VR::US },
    E { tag: Tag(0x5400, 0x1006), alias: "WaveformSampleInterpretation", vr: VR::CS },
    E { tag: Tag(0x5400, 0x100A), alias: "WaveformPaddingValue", vr: VR::OB/* or  or OW */ },
    E { tag: Tag(0x5400, 0x1010), alias: "WaveformData", vr: VR::OB/* or  or OW */ },
    E { tag: Tag(0x5600, 0x0010), alias: "FirstOrderPhaseCorrectionAngle", vr: VR::OF },
    E { tag: Tag(0x5600, 0x0020), alias: "SpectroscopyData", vr: VR::OF },
    E { tag: Tag(0x7FE0, 0x0008), alias: "FloatPixelData", vr: VR::OF },
    E { tag: Tag(0x7FE0, 0x0009), alias: "DoubleFloatPixelData", vr: VR::OD },
    E { tag: Tag(0x7FE0, 0x0010), alias: "PixelData", vr: VR::OB/* or  or OW */ },
    E { tag: Tag(0xFFFA, 0xFFFA), alias: "DigitalSignaturesSequence", vr: VR::SQ },
    E { tag: Tag(0xFFFC, 0xFFFC), alias: "DataSetTrailingPadding", vr: VR::OB },
    E { tag: Tag(0xFFFE, 0xE000), alias: "Item", vr: VR::UN },
    E { tag: Tag(0xFFFE, 0xE00D), alias: "ItemDelimitationItem", vr: VR::UN },
    E { tag: Tag(0xFFFE, 0xE0DD), alias: "SequenceDelimitationItem", vr: VR::UN },
];
