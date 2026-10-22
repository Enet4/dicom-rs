#![no_main]
use dicom_core::value::{DicomDate, DicomTime, PrimitiveValue};
use dicom_core::{DataElementHeader, Length, Tag, VR};
use dicom_parser::dataset::{DataSetReader, DataSetWriter, DataToken};
use dicom_transfer_syntax_registry::entries;
use libfuzzer_sys::{
    arbitrary::{Arbitrary, Unstructured},
    fuzz_target,
};

fn arbitrary_value(u: &mut Unstructured) -> Option<PrimitiveValue> {
    Some(match u.arbitrary::<u8>().ok()? % 6 {
        0 => PrimitiveValue::Empty,
        1 => PrimitiveValue::from(Vec::<u8>::arbitrary(u).ok()?),
        2 => PrimitiveValue::from(u.arbitrary::<i32>().ok()?),
        3 => PrimitiveValue::from(u.arbitrary::<f64>().ok()?),
        4 => {
            let y = u.arbitrary::<u16>().ok()? % 10000;
            let m = u.arbitrary::<u8>().ok()? % 12 + 1;
            let d = u.arbitrary::<u8>().ok()? % 28 + 1;
            PrimitiveValue::from(DicomDate::from_ymd(y, m, d).ok()?)
        }
        5 => {
            let h = u.arbitrary::<u8>().ok()? % 24;
            let mi = u.arbitrary::<u8>().ok()? % 60;
            let s = u.arbitrary::<u8>().ok()? % 60;
            PrimitiveValue::from(DicomTime::from_hms(h, mi, s).ok()?)
        }
        _ => {
            let n = (u.arbitrary::<u8>().ok()? % 8) as usize;
            let tags = (0..n)
                .map(|_| Tag(u.arbitrary().unwrap_or(0), u.arbitrary().unwrap_or(0)))
                .collect();
            PrimitiveValue::Tags(tags)
        }
    })
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);

    let Ok(group) = u.arbitrary::<u16>() else {
        return;
    };
    let Ok(element) = u.arbitrary::<u16>() else {
        return;
    };
    let (Ok(a), Ok(b)) = (u.arbitrary::<u8>(), u.arbitrary::<u8>()) else {
        return;
    };
    let vr = VR::from_binary([a, b]).unwrap_or(VR::UN);
    let Some(value) = arbitrary_value(&mut u) else {
        return;
    };
    let Ok(explicit_vr) = u.arbitrary::<bool>() else {
        return;
    };

    let tag = Tag(group, element);
    let header = DataElementHeader::new(tag, vr, Length(0));

    let ts = if explicit_vr {
        entries::EXPLICIT_VR_LITTLE_ENDIAN.erased()
    } else {
        entries::IMPLICIT_VR_LITTLE_ENDIAN.erased()
    };

    let mut out = Vec::new();
    let Ok(mut writer) = DataSetWriter::with_ts(&mut out, &ts) else {
        return;
    };

    let wrote = writer.write_sequence([
        DataToken::ElementHeader(header),
        DataToken::PrimitiveValue(value),
    ]);
    drop(writer);
    if wrote.is_err() {
        return;
    }

    if let Ok(reader) = DataSetReader::new_with_ts(out.as_slice(), &ts) {
        for token in reader {
            let Ok(token) = token else { break };
            if let DataToken::ElementHeader(read_header) = token {
                assert_eq!(
                    read_header.tag, tag,
                    "element encoded for tag {} decoded back as {}",
                    tag, read_header.tag
                );
                break;
            }
        }
    }
});
