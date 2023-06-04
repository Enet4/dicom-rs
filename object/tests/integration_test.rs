use std::{
    fs::File,
    io::{BufReader, Read},
};

use dicom_core::value::Value;
use dicom_dictionary_std::tags;
use dicom_encoding::text::SpecificCharacterSet;
use dicom_object::{
    file::{OpenFileOptions, ReadPreamble},
    mem::InMemDicomObject,
    open_file,
};
use dicom_test_files;

#[test]
fn test_ob_value_with_unknown_length() {
    let path =
        dicom_test_files::path("pydicom/JPEG2000.dcm").expect("test DICOM file should exist");
    let object = open_file(&path).unwrap();
    let element = object.element_by_name("PixelData").unwrap();

    match element.value() {
        Value::PixelSequence {
            fragments,
            offset_table,
        } => {
            // check offset table
            assert_eq!(offset_table.len(), 0);

            // check if the leading and trailing bytes look right
            assert_eq!(fragments.len(), 1);
            let fragment = &fragments[0];
            assert_eq!(fragment[0..2], [255, 79]);
            assert_eq!(fragment[fragment.len() - 2..fragment.len()], [255, 217]);
        }
        value => {
            panic!("expected a pixel sequence, but got {:?}", value);
        }
    }
}

#[test]
fn test_read_until_pixel_data() {
    let path =
        dicom_test_files::path("pydicom/JPEG2000.dcm").expect("test DICOM file should exist");
    let object = OpenFileOptions::new()
        .read_until(tags::PIXEL_DATA)
        .open_file(&path)
        .expect("File should open successfully");

    // contains other elements such as modality
    let element = object.element(tags::MODALITY).unwrap();
    assert_eq!(element.value().to_str().unwrap(), "NM");

    // but does not contain pixel data
    assert!(matches!(
        object.element(tags::PIXEL_DATA),
        Err(dicom_object::Error::NoSuchDataElementTag { .. })
    ));
}

#[test]
fn test_read_data_with_preamble() {
    let path = dicom_test_files::path("pydicom/liver.dcm").expect("test DICOM file should exist");
    let source = BufReader::new(File::open(path).unwrap());

    // should read preamble even though it's from a reader
    let object = OpenFileOptions::new()
        .read_preamble(ReadPreamble::Always)
        .from_reader(source)
        .expect("Should read from source successfully");

    // contains elements such as study date
    let element = object.element(tags::STUDY_DATE).unwrap();
    assert_eq!(element.value().to_str().unwrap(), "20030417");
}

#[test]
fn test_read_data_with_preamble_auto() {
    let path = dicom_test_files::path("pydicom/liver.dcm").expect("test DICOM file should exist");
    let source = BufReader::new(File::open(path).unwrap());

    // should read preamble even though it's from a reader
    let object = OpenFileOptions::new()
        .from_reader(source)
        .expect("Should read from source successfully");

    // contains elements such as study date
    let element = object.element(tags::STUDY_DATE).unwrap();
    assert_eq!(element.value().to_str().unwrap(), "20030417");
}

#[test]
fn test_read_data_without_preamble() {
    let path = dicom_test_files::path("pydicom/liver.dcm").expect("test DICOM file should exist");
    let mut source = BufReader::new(File::open(path).unwrap());

    // read preamble manually
    let mut preamble = [0; 128];

    source.read_exact(&mut preamble).unwrap();

    // explicitly do not read preamble
    let object = OpenFileOptions::new()
        .read_preamble(ReadPreamble::Never)
        .from_reader(source)
        .expect("Should read from source successfully");

    // contains elements such as study date
    let element = object.element(tags::STUDY_DATE).unwrap();
    assert_eq!(element.value().to_str().unwrap(), "20030417");
}

#[test]
fn test_read_data_without_preamble_auto() {
    let path = dicom_test_files::path("pydicom/liver.dcm").expect("test DICOM file should exist");
    let mut source = BufReader::new(File::open(path).unwrap());

    // skip preamble
    let mut preamble = [0; 128];

    source.read_exact(&mut preamble).unwrap();

    // detect lack of preamble automatically
    let object = OpenFileOptions::new()
        .from_reader(source)
        .expect("Should read from source successfully");

    // contains elements such as study date
    let element = object.element(tags::STUDY_DATE).unwrap();
    assert_eq!(element.value().to_str().unwrap(), "20030417");
}

#[test]
fn test_expl_vr_le_no_meta() {
    let path = dicom_test_files::path("pydicom/ExplVR_LitEndNoMeta.dcm")
        .expect("test DICOM file should exist");
    let source = BufReader::new(File::open(path).unwrap());
    let ts = dicom_transfer_syntax_registry::entries::EXPLICIT_VR_LITTLE_ENDIAN.erased();
    let object =
        InMemDicomObject::read_dataset_with_ts_cs(source, &ts, SpecificCharacterSet::Default)
            .unwrap();

    let sop_instance_uid = object.element_by_name("SOPInstanceUID").unwrap();
    assert_eq!(sop_instance_uid.to_str().unwrap(), "1.2.333.4444.5.6.7.8",);

    let series_instance_uid = object.element_by_name("SeriesInstanceUID").unwrap();
    assert_eq!(
        series_instance_uid.to_str().unwrap(),
        "1.2.333.4444.5.6.7.8.99",
    );

    let frame_of_reference_uid = object.element_by_name("FrameOfReferenceUID").unwrap();
    assert_eq!(
        frame_of_reference_uid.to_str().unwrap(),
        "1.2.333.4444.5.6.7.8.9",
    );
}
