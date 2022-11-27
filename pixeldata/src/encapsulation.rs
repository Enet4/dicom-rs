use dicom_core::value::{Value, C};
use dicom_core::DataDictionary;
use dicom_object::mem::InMemFragment;
use dicom_object::InMemDicomObject;
use snafu::prelude::*;
/// Encapsulate the pixel data of the frames. If frames > 1 then fragments is ignored and set to 1.
/// If the calculated fragment size is less than 2 bytes, then it is set to 2 bytes
pub fn encapsulate<'a, D>(
    frames: Vec<Vec<u8>>,
    number_of_fragments: u32,
) -> Value<InMemDicomObject<D>, InMemFragment>
where
    D: DataDictionary,
    D: Clone,
{
    let mut frames = frames;
    let number_of_fragments = if frames.len() > 1 { 1 } else { number_of_fragments };
    let mut bot = C::new();
    let mut fragments = C::new();

    frames
        .iter_mut()
        .flat_map(|data| {
            // Calculate the fragment size. If it is less than 2 bytes, make it 2 bytes.
            // Otherwise make it even.
            let fragment_size = (data.len() as f32 / number_of_fragments as f32).ceil() as u32;
            let fragment_size = if fragment_size > 2 {
                if fragment_size % 2 == 0 {
                    fragment_size
                } else {
                    fragment_size + 1
                }
            } else {
                2u32
            };

            // Calculate the encapsulated size. If necessary pad the vector with zeroes so all the
            // chunks have the same fragment_size
            let encapsulated_size = (fragment_size * number_of_fragments) as usize;
            if encapsulated_size > data.len() {
                data.resize(encapsulated_size, 0);
            }

            data.chunks_exact(fragment_size as usize)
        })
        .map(|fragment| fragment.to_owned().to_vec())
        .fold(0, |acc, fragment| {
            let length = fragment.len() as u32;
            bot.push(acc as u32);
            fragments.push((*fragment).to_vec());

            acc + length + 1
        });

    Value::PixelSequence {
        offset_table: bot,
        fragments: fragments,
    }
}
