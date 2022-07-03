use crate::dicom_value;
use crate::value::PrimitiveValue;
use snafu::{OptionExt, Snafu};
use std::fmt::{Display, Formatter};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("PatientName cannot start with '^'"))]
    LeadingWhitespace {},
    #[snafu(display("Family name is mandatory"))]
    NoFamily {},
    #[snafu(display("Family name is empty"))]
    EmptyFamily {},
    #[snafu(display("Given name is mandatory"))]
    NoGiven {},
    #[snafu(display("Given name is epmty"))]
    EmptyGiven {},
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Represents a Dicom PersonName.
/// Contains family, given, middle name, prefix and suffix.
/// For practical purposes, family and given name are mandatory components, not optional as per standard.
/// The Dicom string representation is split by "^" separator into its respective components.
/// Does not support component group delimiter "=", ideographic and phonetic characters.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonName<'a> {
    prefix: Option<&'a str>,
    family: &'a str,
    middle: Option<&'a str>,
    given: &'a str,
    suffix: Option<&'a str>,
}

impl<'a> From<PersonName<'a>> for PrimitiveValue {
    fn from(p: PersonName) -> Self {
        dicom_value!(Str, p.to_dicom_string())
    }
}

macro_rules! write_if_some {
    ($formater: expr, $option: expr,  $format: expr) => {
        if let Some(opt) = $option {
            write!($formater, $format, opt)?
        }
    };
}

impl Display for PersonName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write_if_some!(f, self.prefix, "{} ");
        write!(f, "{} ", self.family)?;
        write_if_some!(f, self.middle, "{} ");
        write!(f, "{}", self.given)?;
        write_if_some!(f, self.suffix, " {}");
        Ok(())
    }
}

impl<'a> PersonName<'a> {
    pub fn new(
        prefix: Option<&'a str>,
        given: &'a str,
        family: &'a str,
        middle: Option<&'a str>,
        suffix: Option<&'a str>,
    ) -> Self {
        Self {
            prefix,
            family,
            middle,
            given,
            suffix,
        }
    }
    pub fn to_dicom_string(&self) -> String {
        let mut name = format!("{}^{}", self.family, self.given);
        match self.middle {
            Some(middle) => name.push_str(&format!("^{}", middle)),
            None => {
                if self.prefix.is_some() || self.suffix.is_some() {
                    name.push('^')
                }
            }
        }
        match self.prefix {
            Some(prefix) => match self.suffix {
                Some(suffix) => name.push_str(&format!("^{}^{}", prefix, suffix)),
                None => name.push_str(&format!("^{}", prefix)),
            },
            None => match self.suffix {
                Some(suffix) => name.push_str(&format!("^^{}", suffix)),
                None => {}
            },
        }
        name
    }
    pub fn prefix(&self) -> Option<&str> {
        self.prefix
    }
    pub fn suffix(&self) -> Option<&str> {
        self.suffix
    }
    pub fn family(&self) -> &str {
        self.family
    }
    pub fn given(&self) -> &str {
        self.given
    }
    pub fn middle(&self) -> Option<&str> {
        self.middle
    }

    // exact match to dicom formatted string
    pub fn is(&self, search: &str) -> bool {
        self.to_dicom_string() == search
    }

    pub fn contains(&self, search: &str) -> bool {
        let prefix = match &self.prefix {
            Some(p) => p.contains(search),
            None => false,
        };
        let suffix = match &self.suffix {
            Some(s) => s.contains(search),
            None => false,
        };

        prefix || self.family.contains(search) || self.given.contains(search) || suffix
    }

    pub fn from_slice(slice: &'a str) -> Result<PersonName<'a>> {
        // As per standard, the slice can contain leading "^" separators.
        // This implementation treats family and given name as mandatory, thus this error.
        if slice.starts_with('^') {
            LeadingWhitespaceSnafu {}.fail()
        } else {
            let mut parts = slice.split('^');
            let family = parts.next().context(NoFamilySnafu)?;
            if family.is_empty() {
                return EmptyFamilySnafu {}.fail();
            }
            let given = parts.next().context(NoGivenSnafu)?;
            if given.is_empty() {
                return EmptyGivenSnafu {}.fail();
            }

            let middle = parts
                .next()
                .and_then(|s| if s.is_empty() { None } else { Some(s) });

            let prefix = parts
                .next()
                .and_then(|s| if s.is_empty() { None } else { Some(s) });

            let suffix = parts
                .next()
                .and_then(|s| if s.is_empty() { None } else { Some(s) });

            Ok(PersonName::new(prefix, given, family, middle, suffix))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_person_name_to_dicom() {
        let p = PersonName {
            prefix: Some("Rev."),
            family: "Adams",
            given: "John",
            middle: Some("Robert"),
            suffix: Some("B.A. M.Div."),
        };
        assert_eq!(
            PrimitiveValue::from(p),
            PrimitiveValue::Str("Adams^John^Robert^Rev.^B.A. M.Div.".to_owned())
        );

        let p = PersonName {
            prefix: None,
            family: "Adams",
            given: "John",
            middle: Some("Robert"),
            suffix: Some("B.A. M.Div."),
        };

        assert_eq!(
            PrimitiveValue::from(p),
            PrimitiveValue::Str("Adams^John^Robert^^B.A. M.Div.".to_owned())
        );

        let p = PersonName {
            prefix: Some("Rev."),
            family: "Adams",
            given: "John",
            middle: Some("Robert"),
            suffix: None,
        };

        assert_eq!(
            PrimitiveValue::from(p),
            PrimitiveValue::Str("Adams^John^Robert^Rev.".to_owned())
        );

        let p = PersonName {
            prefix: None,
            family: "Adams",
            given: "John",
            middle: Some("Robert"),
            suffix: None,
        };

        assert_eq!(
            PrimitiveValue::from(p),
            PrimitiveValue::Str("Adams^John^Robert".to_owned())
        );
        let p = PersonName {
            prefix: None,
            family: "Adams",
            given: "John",
            middle: None,
            suffix: None,
        };

        assert_eq!(
            PrimitiveValue::from(p),
            PrimitiveValue::Str("Adams^John".to_owned())
        );
        let p = PersonName {
            prefix: None,
            family: "Adams",
            given: "John",
            middle: None,
            suffix: Some("B.A. M.Div."),
        };

        assert_eq!(
            PrimitiveValue::from(p),
            PrimitiveValue::Str("Adams^John^^^B.A. M.Div.".to_owned())
        );
    }
    #[test]
    fn person_name_from_string() {
        let full = "Adamson^John^Doolittle^Prof^PhD";
        let no_middle = "Adamson^John^^Prof^PhD";
        let no_prefix = "Adamson^John^Doolittle^^PhD";
        let no_suffix = "Adamson^John^Doolittle^Prof";
        let only_suffix = "Adamson^John^^^PhD";
        let wildcard = "A?am*^John*^^Prof";

        assert_eq!(
            PersonName::from_slice(full).ok(),
            Some(PersonName {
                prefix: Some("Prof"),
                family: "Adamson",
                given: "John",
                middle: Some("Doolittle"),
                suffix: Some("PhD")
            })
        );
        assert_eq!(
            PersonName::from_slice(no_middle).ok(),
            Some(PersonName {
                prefix: Some("Prof"),
                family: "Adamson",
                given: "John",
                middle: None,
                suffix: Some("PhD")
            })
        );
        assert_eq!(
            PersonName::from_slice(no_prefix).ok(),
            Some(PersonName {
                prefix: None,
                family: "Adamson",
                given: "John",
                middle: Some("Doolittle"),
                suffix: Some("PhD")
            })
        );
        assert_eq!(
            PersonName::from_slice(no_suffix).ok(),
            Some(PersonName {
                prefix: Some("Prof"),
                family: "Adamson",
                middle: Some("Doolittle"),
                given: "John",
                suffix: None
            })
        );
        assert_eq!(
            PersonName::from_slice(only_suffix).ok(),
            Some(PersonName {
                prefix: None,
                family: "Adamson",
                given: "John",
                middle: None,
                suffix: Some("PhD")
            })
        );
        assert_eq!(
            PersonName::from_slice(wildcard).ok(),
            Some(PersonName {
                prefix: Some("Prof"),
                family: "A?am*",
                given: "John*",
                middle: None,
                suffix: None
            })
        );
    }
    #[test]
    fn test_person_name_contains() {
        let p = PersonName {
            prefix: Some("Prof"),
            family: "Adamson",
            given: "John",
            middle: None,
            suffix: Some("PhD"),
        };
        assert!(p.contains("damson"));
    }
}
