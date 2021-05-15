# dicom-pixeldata
Pixel data handler for the dicom-rs crate.

## Example
```rust
use std::error::Error;
use dicom_object::open_file;
use dicom_pixeldata::PixelDecoder;

fn main() -> Result<(), Box<dyn Error>> {
    let obj = open_file("dicom.dcm")?;
    let image = obj.decode_pixel_data()?;
    let dynamic_image = image.to_dynamic_image()?;
    dynamic_image.save("out.png")?;
    Ok(())
}
```