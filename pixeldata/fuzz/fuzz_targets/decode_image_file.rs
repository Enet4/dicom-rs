#![no_main]
use dicom_pixeldata::PixelDecoder;
use libfuzzer_sys::fuzz_target;
use std::error::Error;

fuzz_target!(|data: &[u8]| {
    let _ = fuzz(data);
});

fn fuzz(data: &[u8]) -> Result<(), Box<dyn Error>> {
    // deserialize random bytes
    let obj = dicom_object::from_reader(data)?;

    // decode them as an image
    let decoded = obj.decode_pixel_data()?;

    // turn into native pixel data vector
    let pixels: Vec<u16> = decoded.to_vec()?;

    // assert that the vector length matches the expected number of samples
    let size = decoded.rows() as u64
        * decoded.columns() as u64
        * decoded.samples_per_pixel() as u64
        * decoded.number_of_frames() as u64;

    assert_eq!(pixels.len() as u64, size);

    Ok(())
}
