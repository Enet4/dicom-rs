#[derive(debug)]
pub struct EncapsulatedAdapter;

pub const ENCAPSULATED_UNCOMPRESSED_EXPLICIT_VR_LITTLE_ENDIAN: TransferSyntax<EncapsulatedAdapter> = TransferSyntax::new(
    "1.2.840.10008.1.2.1.98",
    "Encapsulated Uncompressed Explicit VR Little Endian",
    Endianness::Little,
    true,
    Codec::Encapsulated(Some(EncapsulatedAdapter), Some(EncapsulatedAdapter),
);


impl PixelDataReader for EncapsulatedAdapter {
    fn decode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        dst: &mut Vec<u8>,
    ) -> DecodeResult<()>{
        let cols = src
            .cols()
            .context(decode_error::MissingAttributeSnafu { name: "Columns" })?;
        let rows = src
            .rows()
            .context(decode_error::MissingAttributeSnafu { name: "Rows" })?;
        let samples_per_pixel =
            src.samples_per_pixel()
                .context(decode_error::MissingAttributeSnafu {
                    name: "SamplesPerPixel",
                })?;
        let bits_allocated = src
            .bits_allocated()
            .context(decode_error::MissingAttributeSnafu {
                name: "BitsAllocated",
            })?;

        if bits_allocated != 8 && bits_allocated != 16 {
            whatever!("BitsAllocated other than 8 or 16 is not supported");
        }
        // Encapsulated Uncompressed has each frame encoded in one fragment
        // So we can assume 1 frag = 1 frame
        // ref. PS3.5 A.4.11 
        let nr_frames =
            src.number_of_fragments()
                .whatever_context("Invalid pixel data, no fragments found")? as usize;
        ensure!(
            nr_frames > frame as usize,
            decode_error::FrameRangeOutOfBoundsSnafu
        );

        let bytes_per_sample = (bits_allocated / 8) as usize;
        let samples_per_pixel = samples_per_pixel as usize;
        let decoded_pixel_data = match &src.fragment(frame as usize).value();
        dst
    }
}
