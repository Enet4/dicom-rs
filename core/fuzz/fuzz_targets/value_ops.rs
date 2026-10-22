#![no_main]
use dicom_core::header::Length;
use dicom_core::value::fragments::Fragments;
use dicom_core::value::{PrimitiveValue, Value, C};
use dicom_core::Tag;
use libfuzzer_sys::fuzz_target;

fuzz_target!(
    |input: (u32, u32, Vec<String>, u8, Vec<u8>, u16, Vec<u16>)| {
        let (len_a, len_b, strs, truncate_limit, fragment_bytes, fragment_size, tag_words) =
            input;

        let sum = Length(len_a) + Length(len_b);
        if len_a != Length::UNDEFINED.0 && len_b != Length::UNDEFINED.0 {
            assert_ne!(
                sum.0,
                Length::UNDEFINED.0,
                "Length addition {len_a} + {len_b} produced the UNDEFINED sentinel"
            );
        }

        if !strs.is_empty() && strs.len() < 256 {
            let value = PrimitiveValue::Strs(strs.iter().cloned().collect());
            let joined = value.to_multi_str();
            assert_eq!(
                joined.len(),
                strs.len(),
                "to_multi_str() changed the item count for {strs:?}"
            );
        }

        let mut value = PrimitiveValue::Strs(strs.into_iter().collect());
        value.truncate(truncate_limit as usize);

        let tags: C<Tag> = tag_words.chunks_exact(2).map(|w| Tag(w[0], w[1])).collect();
        let value: Value = Value::Primitive(PrimitiveValue::Tags(tags));
        let _ = value.to_tag();

        if fragment_bytes.len() < 4_000_000 {
            let _ = Fragments::new(fragment_bytes, fragment_size as u32);
        }
    }
);
