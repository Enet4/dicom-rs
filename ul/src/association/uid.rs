//! Private utility module for working with UIDs

use std::borrow::Cow;

pub(crate) fn trim_uid<'a>(uid: Cow<'a, str>) -> Cow<'a, str> {
    if uid.ends_with('\0') {
        Cow::Owned(uid.trim_end_matches(|c| c == '\0').to_string())
    } else {
        uid
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::trim_uid;

    #[test]
    fn test_trim_uid() {
        let uid = trim_uid(Cow::from("1.2.3.4"));
        assert_eq!(uid, "1.2.3.4");
        let uid = trim_uid(Cow::from("1.2.3.4\0"));
        assert_eq!(uid, "1.2.3.4");
        let uid = trim_uid(Cow::from("1.2.3.45\0"));
        assert_eq!(uid, "1.2.3.45");
    }
}