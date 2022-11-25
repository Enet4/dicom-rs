use dicom_core::value::{Value, C};
use dicom_core::DataDictionary;
use dicom_object::mem::InMemFragment;
use dicom_object::InMemDicomObject;
use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum EncapsulationError {
    TooManyFragments,
}

pub fn encapsulate<D>(
    frames: &mut [Vec<u8>],
    fragments: u32,
) -> Result<Value<InMemDicomObject<D>, InMemFragment>, EncapsulationError>
where
    D: DataDictionary,
    D: Clone,
{
    let mut processed = C::new();
    let mut bot = C::new();
    frames
        .iter_mut()
        .map(|data| {
            let fragment_size = (data.len() as f32 / fragments as f32).ceil() as u32;
            if fragment_size > 2 {
                // Fragment size has to be even and 4 bytes in size
                let fragment_size = {
                    if fragment_size % 2 == 0 {
                        fragment_size
                    } else {
                        fragment_size + 1
                    }
                } as u32;

                let encapsulated_size = (fragment_size * fragments) as usize;
                if encapsulated_size > data.len() {
                    // Pad data of the last fragment
                    data.resize(encapsulated_size, 0);
                }
                Ok(data.chunks_exact(fragment_size as usize))
            } else {
                Err(EncapsulationError::TooManyFragments)
            }
        })
        .collect::<Result<Vec<_>, EncapsulationError>>()?
        .into_iter()
        .flatten()
        .fold(0, |acc, fragment| {
            let length = fragment.len() as u32;
            processed.push(Vec::from(fragment));
            bot.push(acc as u32);

            acc + length + 1
        });

    Ok(Value::PixelSequence {
        offset_table: bot,
        fragments: processed,
    })
}
