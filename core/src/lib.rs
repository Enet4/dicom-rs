#![crate_type = "lib"]
#![deny(trivial_numeric_casts, unsafe_code, unstable_features)]
#![warn(
    missing_debug_implementations,
    unused_qualifications,
    unused_import_braces
)]

//! This is the core library of DICOM-rs containing various concepts,
//! data structures and traits specific to DICOM content.
//!
//! The current structure of this crate is as follows:
//!
//! - [`header`] comprises various data types for DICOM element header,
//!   including common definitions for DICOM tags and value representations.
//! - [`dictionary`] describes common behavior of DICOM data dictionaries,
//!   which translate attribute names and/or tags to a dictionary entry
//!   containing relevant information about the attribute.
//! - [`value`] holds definitions for values in standard DICOM elements,
//!   with the awareness of multiplicity, representation,
//!   and the possible presence of sequences.
//! - [`error`] contains crate-level error and result types.
//!
//! [`dictionary`]: ./dictionary/index.html
//! [`error`]: ./error/index.html
//! [`header`]: ./header/index.html
//! [`value`]: ./value/index.html

pub mod dictionary;
pub mod header;
pub mod value;

pub use dictionary::DataDictionary;
pub use header::{DataElement, DataElementHeader, Length, Tag, VR};
pub use value::{PrimitiveValue, Value as DicomValue};

// re-export crates that are part of the public API
pub use chrono;
pub use smallvec;

mod util;

/// Helper macro for constructing a DICOM primitive value,
/// of an arbitrary variant and multiplicity.
///
/// The base syntax is a value type identifier,
/// which is one of the variants of [`PrimitiveValue`],
/// followed by either an expression resolving to one standard Rust value,
/// or an explicitly laid out array of Rust values.
/// The type variant may be omitted in some cases.
///
/// Passing a single expression for multiple values is not supported.
/// Please use standard `From` conversions instead.
///
/// ```none
/// dicom_value!() // empty value
/// dicom_value!(«Type», «expression») // one value
/// dicom_value!(«Type», [«expression1», «expression2», ...]) // multiple values
/// dicom_value!(«expression») // a single value, inferred variant
/// ```
///
/// # Examples:
///
/// Strings are automatically converted to retain ownership.
///
/// ```
/// use dicom_core::value::PrimitiveValue;
/// use dicom_core::{DicomValue, dicom_value};
///
/// let value = dicom_value!(Str, "Smith^John");
/// assert_eq!(
///     value,
///     PrimitiveValue::Str("Smith^John".to_owned()),
/// );
/// ```
///
/// A DICOM value may also have multiple elements:
///
/// ```
/// # use dicom_core::value::PrimitiveValue;
/// # use dicom_core::dicom_value;
/// let value = dicom_value!(Strs, [
///     "Smith^John",
///     "Simões^João",
/// ]);
/// assert_eq!(
///     value,
///     PrimitiveValue::Strs([
///         "Smith^John".to_string(),
///         "Simões^João".to_string(),
///     ][..].into()),
/// );
/// let value = dicom_value!(U16, [5, 6, 7]);
/// assert_eq!(
///     value,
///     PrimitiveValue::U16([5, 6, 7][..].into()),
/// );
/// ```
///
/// The output is a [`PrimitiveValue`],
/// which can be converted to a `DicomValue` as long as its type parameters
/// are specified or inferable.
///
/// ```
/// # use dicom_core::header::EmptyObject;
/// # use dicom_core::value::PrimitiveValue;
/// # use dicom_core::{DicomValue, dicom_value};
/// # let value = dicom_value!(U16, [5, 6, 7]);
/// // conversion to a DicomValue only requires its type parameters
/// // to be specified or inferable.
/// assert_eq!(
///     DicomValue::from(value),
///     DicomValue::<EmptyObject, ()>::Primitive(
///         PrimitiveValue::U16([5, 6, 7][..].into())),
/// );
/// ```
///
/// [`PrimitiveValue`]: ./enum.PrimitiveValue.html
#[macro_export]
macro_rules! dicom_value {
    // Empty value
    () => { $crate::value::PrimitiveValue::Empty };
    // Multiple strings
    (Strs, [ $($elem: expr),+ , ]) => {
        {
            use smallvec::smallvec; // import smallvec macro
            $crate::value::PrimitiveValue :: Strs (smallvec![$($elem.to_owned(),)*])
        }
    };
    (Strs, [ $($elem: expr),+ ]) => {
        {
            use smallvec::smallvec; // import smallvec macro
            $crate::value::PrimitiveValue :: Strs (smallvec![$($elem.to_owned(),)*])
        }
    };
    ($typ: ident, [ $($elem: expr),+ , ]) => {
        {
            use smallvec::smallvec; // import smallvec macro
            $crate::value::PrimitiveValue :: $typ (smallvec![$($elem,)*])
        }
    };
    ($typ: ident, [ $($elem: expr),+ ]) => {
        {
            use smallvec::smallvec; // import smallvec macro
            $crate::value::PrimitiveValue :: $typ (smallvec![$($elem,)*])
        }
    };
    (Str, $elem: expr) => {
        $crate::value::PrimitiveValue :: Str (String::from($elem))
    };
    ($typ: ident, $elem: expr) => {
        $crate::value::PrimitiveValue :: $typ ($crate::value::C::from_elem($elem, 1))
    };
    ($elem: expr) => {
        $crate::value::PrimitiveValue::from($elem)
    };
}

#[cfg(test)]
mod tests {
    use crate::dicom_value;
    use crate::value::PrimitiveValue;
    use smallvec::smallvec;

    #[test]
    fn macro_dicom_value() {
        // single string with variant
        assert_eq!(
            dicom_value!(Str, "PALETTE COLOR "),
            PrimitiveValue::Str("PALETTE COLOR ".to_owned()),
        );

        // single string without variant
        assert_eq!(
            dicom_value!("PALETTE COLOR "),
            PrimitiveValue::Str("PALETTE COLOR ".to_owned()),
        );

        // multiple string literals with variant, no trailing comma
        assert_eq!(
            dicom_value!(Strs, ["BASE", "LIGHT", "DARK"]),
            PrimitiveValue::Strs(smallvec![
                "BASE".to_owned(),
                "LIGHT".to_owned(),
                "DARK".to_owned(),
            ]),
        );

        // multiple strings and string slices with variant, no trailing comma
        assert_eq!(
            dicom_value!(
                Strs,
                [
                    "DERIVED",
                    "PRIMARY".to_string(), // accepts both &str and String
                    "WHOLE BODY",
                    "EMISSION"
                ]
            ),
            PrimitiveValue::Strs(smallvec![
                "DERIVED".to_string(),
                "PRIMARY".to_string(),
                "WHOLE BODY".to_string(),
                "EMISSION".to_string(),
            ]),
        );

        // multiple string literals with variant, with trailing comma
        assert_eq!(
            dicom_value!(Strs, ["DERIVED", "PRIMARY", "WHOLE BODY", "EMISSION",]),
            PrimitiveValue::Strs(smallvec![
                "DERIVED".to_string(),
                "PRIMARY".to_string(),
                "WHOLE BODY".to_string(),
                "EMISSION".to_string(),
            ]),
        );

        // single number with variant
        assert_eq!(dicom_value!(U16, 55), PrimitiveValue::U16(smallvec![55]),);

        // single number without variant
        assert_eq!(dicom_value!(55_u32), PrimitiveValue::U32(smallvec![55]),);

        // multiple numbers without variant, no trailing comma
        assert_eq!(
            dicom_value!(I32, [11, 22, 33]),
            PrimitiveValue::I32(smallvec![11, 22, 33]),
        );

        // empty value
        assert_eq!(dicom_value!(), PrimitiveValue::Empty,);
    }
}
