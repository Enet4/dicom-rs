use dicom_core::value::{Value, C};
use dicom_core::DataDictionary;
use dicom_object::mem::InMemFragment;
use dicom_object::InMemDicomObject;
use snafu::prelude::*;

#[derive(Debug)]
pub struct EncapsulatedPixels {
    bot: C<u32>,
    current_offset: u32,
    fragments: C<Vec<u8>>,
    number_of_fragments: u32
}

impl EncapsulatedPixels {
    /// Add a single frame to EncapsulatedPixels
    pub fn add_frame(&mut self, data: Vec<u8>) {
        for fragment in fragment_frame(data, self.number_of_fragments) {
            let size = fragment.len() as u32;
            self.bot.push(self.current_offset);
            self.fragments.push((*fragment).to_vec());
            self.current_offset += size;
        }
    }

    /// Creates an empty EncapsulatedPixels struct
    pub fn new(number_of_fragments: u32) -> Self {
        let number_of_fragments = if number_of_fragments > 0 {
            number_of_fragments
        } else {
            1u32
        };

        EncapsulatedPixels {
            bot: C::new(),
            current_offset: 0,
            fragments: C::new(),
            number_of_fragments: number_of_fragments
        }
    }
}

impl<D> Into<Value<InMemDicomObject<D>, InMemFragment>> for EncapsulatedPixels
where
    D: DataDictionary,
    D: Clone,
{
    fn into(self) -> Value<InMemDicomObject<D>, InMemFragment> {
        Value::PixelSequence {
            offset_table: self.bot,
            fragments: self.fragments,
        }
    }
}

impl From<Vec<Vec<u8>>> for EncapsulatedPixels {

    /// Create EncapsulatedPixels from a list of fragments and calculate the bot
    fn from(fragments: Vec<Vec<u8>>) -> Self {
        let mut bot = C::with_capacity(fragments.len());
        let mut current_offset = 0u32;
        for fragment in &fragments {
            bot.push(current_offset);
            current_offset += fragment.len() as u32;
        }

        EncapsulatedPixels {
            bot,
            current_offset,
            fragments: fragments.into(),
            number_of_fragments: 1,
        }
    }
}

/// Create the fragments for a single frame. It returns a list with the fragments.
pub fn fragment_frame(data: Vec<u8>, number_of_fragments: u32) -> Vec<Vec<u8>> {
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
    let mut data = data;
    let encapsulated_size = (fragment_size * number_of_fragments) as usize;
    if encapsulated_size > data.len() {
        data.resize(encapsulated_size, 0);
    }

    data.chunks_exact(fragment_size as usize)
        .map(|fragment| (*fragment).to_vec())
        .collect::<Vec<Vec<u8>>>()
}

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
    let number_of_fragments = if frames.len() > 1 { 1 } else { number_of_fragments };
    let mut encapsulated_data = EncapsulatedPixels::new(number_of_fragments);

    for frame in frames {
        encapsulated_data.add_frame(frame);
    }

    encapsulated_data.into()
}

