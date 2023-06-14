//! Separate test suite for using `dicom_value!` in an isolated context,
//! without direct access to dependency `smallvec`

// empty module makes `smallvec` dependency unreachable,
// as would be typical in dependents of `dicom_core`
// unless they include it themselves
mod smallvec {}

#[test]
fn use_dicom_value() {
    use dicom_core::dicom_value;

    // multiple string literals with variant, no trailing comma
    let value = dicom_value!(Strs, ["BASE", "LIGHT", "DARK"]);
    assert_eq!(
        value.to_multi_str().as_ref(),
        &["BASE".to_owned(), "LIGHT".to_owned(), "DARK".to_owned(),],
    );

    // single string with variant
    let value = dicom_value!(Str, "PALETTE COLOR ");
    assert_eq!(value.to_string(), "PALETTE COLOR",);

    // numeric values
    let value = dicom_value!(U16, [1, 2, 5]);
    assert_eq!(value.to_multi_int::<u16>().unwrap(), &[1, 2, 5],);
}
