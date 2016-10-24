//! This module includes a high level abstraction over a DICOM data element's value.

use error::InvalidValueReadError;
use std::result;

/// Result type alias for this module.
pub type Result<T> = result::Result<T, InvalidValueReadError>;

#[inline]
fn read_first<T: Copy>(array_res: Result<&[T]>) -> Result<T> {
    array_res.and_then(|arr| { arr.first().map(|v|{*v}).ok_or(InvalidValueReadError::EmptyElement)})
}

/// A trait for retrieving a value from a DICOM element.
pub trait DicomValue {
    /// Check whether the value is empty (0 length).
    fn is_null(&self) -> bool { false }

    // possible contained data types: [i32], [i64], [u32], [u64], [f32], [f64], [String]

    /// Typed value getter
    fn get_i32_array(&self) -> Result<&[i32]> { Err(InvalidValueReadError::IncompatibleType) }
    /// Typed value getter
    fn get_i64_array(&self) -> Result<&[i64]> { Err(InvalidValueReadError::IncompatibleType) }
    /// Typed value getter
    fn get_u32_array(&self) -> Result<&[u32]> { Err(InvalidValueReadError::IncompatibleType) }
    /// Typed value getter
    fn get_u64_array(&self) -> Result<&[u64]> { Err(InvalidValueReadError::IncompatibleType) }
    /// Typed value getter
    fn get_f32_array(&self) -> Result<&[f32]> { Err(InvalidValueReadError::IncompatibleType) }
    /// Typed value getter
    fn get_f64_array(&self) -> Result<&[f64]> { Err(InvalidValueReadError::IncompatibleType) }
    /// Typed value getter
    fn get_string_array(&self) -> Result<&[&str]> { Err(InvalidValueReadError::IncompatibleType) }
    /*
    fn get_object_array(&self) -> ReadResult<&[&DicomObject<Element=DicomElement<Value=DicomValue, Tag=Tag>>]> { Err(InvalidValueReadError::IncompatibleType) }
    */
    /// Typed value getter
    fn get_i32(&self) -> Result<i32> {
        read_first(self.get_i32_array())
    }

    /// Typed value getter
    fn get_i64(&self) -> Result<i64> {
        read_first(self.get_i64_array())
    }

    /// Typed value getter
    fn get_u32(&self) -> Result<u32> {
        read_first(self.get_u32_array())
    }

    /// Typed value getter
    fn get_u64(&self) -> Result<u64> {
        read_first(self.get_u64_array())
    }

    /// Typed value getter
    fn get_f32(&self) -> Result<f32> {
        read_first(self.get_f32_array())
    }

    /// Typed value getter
    fn get_f64(&self) -> Result<f64> {
        read_first(self.get_f64_array())
    }

    /// Typed value getter
    fn get_string(&self) -> Result<&str> {
        read_first(self.get_string_array())
    }

//    fn get_object(&self) -> ReadResult<&DynDicomObject> {
//        read_first(self.get_object_array())
//    }
}

/// Data type for a value with null content
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct NullValue;
impl DicomValue for NullValue {
    fn is_null(&self) -> bool { true }
}

macro_rules! DicomValue_slice_implement {
    ( $t:ty, $f:ident ) => {
        impl<'a> DicomValue for &'a[$t] {
            fn $f(&self) -> Result<&[$t]> { Ok(self) }
        }
    };
}

impl<'a> DicomValue for &'a[i32] {
    fn get_i32_array(&self) -> Result<&[i32]> { Ok(self) }
}
impl<'a> DicomValue for &'a[i64] {
    fn get_i64_array(&self) -> Result<&[i64]> { Ok(self) }
}
impl<'a> DicomValue for &'a[u32] {
    fn get_u32_array(&self) -> Result<&[u32]> { Ok(self) }
}
impl<'a> DicomValue for &'a[u64] {
    fn get_u64_array(&self) -> Result<&[u64]> { Ok(self) }
}
impl<'a> DicomValue for &'a[f32] {
    fn get_f32_array(&self) -> Result<&[f32]> { Ok(self) }
}
impl<'a> DicomValue for &'a[f64] {
    fn get_f64_array(&self) -> Result<&[f64]> { Ok(self) }
}
impl<'a, 'b> DicomValue for &'a[&'b str] {
    fn get_string_array(&self) -> Result<&[&str]> { Ok(self) }
}
//impl<'a, 'b> DicomValue for &'a[&'b DicomObject] {
//    fn get_object_array(&self) -> ReadResult<&[&DicomObject]> { Ok(self) }
//}

macro_rules! DicomValue_array_implement {
    ( $t:ty, $f:ident, $($n:expr),* ) => {
        $(// for each $n
        impl DicomValue for [$t; $n] {
            fn $f(&self) -> Result<&[$t]> { Ok(self.as_ref()) }
        })*
    };
}

// implement DICOM value for all arrays up to size 32
DicomValue_array_implement!(i32, get_i32_array,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);
DicomValue_array_implement!(i64, get_i64_array,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);
DicomValue_array_implement!(u32, get_u32_array,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);
DicomValue_array_implement!(u64, get_u64_array,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);
DicomValue_array_implement!(f32, get_f32_array,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);
DicomValue_array_implement!(f64, get_f64_array,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);

macro_rules! DicomValue_array_implement_lifetime {
    ( $t:ty, $f:ident, $($n:expr),* ) => {
        $(// for each $n
        impl<'a> DicomValue for [&'a $t; $n] {
            fn $f(&self) -> Result<&[&$t]> { Ok(self.as_ref()) }
        })*
    };
}
// &str needs a specific lifetime
DicomValue_array_implement_lifetime!(str, get_string_array,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);
// &DicomObject needs a specific lifetime
//DicomValue_array_implement_lifetime!(DicomObject, get_object_array,
//    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32);



