# dicom-pixeldata
Pixel data handler for the dicom-rs crate.

## Example
```rust
use dicom::open_file;
use dicom_object::open_file;
use dicom_pixeldata::PixelDecoder;

fn main() {
    let obj = open_file("dicom.dcm").unwrap();
    let image = obj.decode_pixel_data().unwrap().to_dynamic_image().unwrap();
    image
        .save("out.png")
        .unwrap();
```

## Supported features
- [x] JPEG2000, JPG Lossless, JPEG Lossy conversion to `DynamicImage`
- [ ] Multi-frame dicoms
- [ ] RGB and other color spaces
- [ ] LUT, Modality LUT and VOI Lut transformations