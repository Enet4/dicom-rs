//! Handling of DICOM values with the VR PN (person name) as per PS3.5 sect 6.2.
use std::fmt::{Display, Formatter};

/*#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Write error: '{source}'"))]
    Write { source: std::fmt::Error },
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
*/
/// Represents a Dicom `PersonName` (PN).
/// Stores family, given, middle name, prefix and suffix as borrowed values.
/// All name components are optional.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PersonName<'a> {
    prefix: Option<&'a str>,
    family: Option<&'a str>,
    middle: Option<&'a str>,
    given: Option<&'a str>,
    suffix: Option<&'a str>,
}

/// A builder to construct a `PersonName` from it's components.
#[derive(Debug, Copy, Clone)]
pub struct PersonNameBuilder<'a> {
    person_name: PersonName<'a>,
}

impl Display for PersonName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let components: &[Option<&str>] = &[
            self.prefix,
            self.given,
            self.middle,
            self.family,
            self.suffix,
        ];

        let mut c_iter = components.iter().flatten().peekable();

        while let Some(component) = c_iter.next() {
            if c_iter.peek().is_some() {
                write!(f, "{} ", component)?
            } else {
                write!(f, "{}", component)?
            }
        }
        Ok(())
    }
}

impl<'a> PersonName<'a> {
    /// Retrieve PersonName prefix
    pub fn prefix(&self) -> Option<&str> {
        self.prefix
    }
    /// Retrieve PersonName suffix
    pub fn suffix(&self) -> Option<&str> {
        self.suffix
    }
    /// Retrieve family name from PersonName
    pub fn family(&self) -> Option<&str> {
        self.family
    }
    /// Retrieve given name from PersonName
    pub fn given(&self) -> Option<&str> {
        self.given
    }
    /// Retrieve middle name from PersonName
    pub fn middle(&self) -> Option<&str> {
        self.middle
    }
    /// Convert PersonName into a Dicom formatted string.
    /// Name components are interspersed with a '^' separator.
    /// Leading null components produce a separator, while trailing do not.
    pub fn to_dicom_string(&self) -> String {
        let mut name = String::new();

        let components: &[Option<&str>] = &[
            self.family,
            self.given,
            self.middle,
            self.prefix,
            self.suffix,
        ];

        let last_non_empty = components
            .iter()
            .enumerate()
            .rev()
            .find(|(_i, opt)| opt.is_some())
            .map(|(i, _opt)| i + 1)
            .unwrap_or(0);

        let mut it = components.iter().take(last_non_empty).peekable();

        while let Some(option) = it.next() {
            if let Some(component) = option {
                name.push_str(component);
            }
            if it.peek().is_some() {
                name.push('^');
            }
        }

        name
    }
    /// Retrieves a PersonName from a Dicom formatted string slice.
    /// The Dicom string representation is split by '^' separator into its respective components.
    pub fn from_slice(slice: &'a str) -> PersonName<'a> {
        let mut parts = slice.split('^');

        macro_rules! get_component {
            () => {
                parts
                    .next()
                    .and_then(|s| if s.is_empty() { None } else { Some(s) })
            };
        }

        let family = get_component!();
        let given = get_component!();
        let middle = get_component!();
        let prefix = get_component!();
        let suffix = get_component!();

        PersonName {
            prefix,
            given,
            family,
            middle,
            suffix,
        }
    }
    /// Retrieve a builder for a PersonName
    pub fn builder() -> PersonNameBuilder<'a> {
        PersonNameBuilder::new()
    }
}

impl<'a> PersonNameBuilder<'a> {
    pub fn new() -> PersonNameBuilder<'a> {
        PersonNameBuilder {
            person_name: PersonName {
                prefix: None,
                family: None,
                middle: None,
                given: None,
                suffix: None,
            },
        }
    }
    /// Insert family name component
    pub fn with_family(mut self, family_name: &'a str) -> PersonNameBuilder<'a> {
        self.person_name.family = Some(family_name);
        self
    }
    pub fn with_middle(mut self, middle_name: &'a str) -> PersonNameBuilder<'a> {
        self.person_name.middle = Some(middle_name);
        self
    }
    pub fn with_given(mut self, given_name: &'a str) -> PersonNameBuilder<'a> {
        self.person_name.given = Some(given_name);
        self
    }
    pub fn with_prefix(mut self, name_prefix: &'a str) -> PersonNameBuilder<'a> {
        self.person_name.prefix = Some(name_prefix);
        self
    }
    pub fn with_suffix(mut self, name_suffix: &'a str) -> PersonNameBuilder<'a> {
        self.person_name.suffix = Some(name_suffix);
        self
    }
    /// Builds a PersonName
    pub fn build(&self) -> PersonName<'a> {
        self.person_name
    }
}

impl<'a> Default for PersonNameBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_person_name_to_dicom_string() {
        let p = PersonName {
            prefix: None,
            given: Some("John"),
            middle: None,
            family: Some("Adams"),
            suffix: None,
        };
        assert_eq!(p.to_dicom_string(), "Adams^John".to_string());

        let p = PersonName {
            prefix: Some("Rev."),
            given: None,
            middle: None,
            family: None,
            suffix: None,
        };
        assert_eq!(p.to_dicom_string(), "^^^Rev.".to_string());
        let p = PersonName {
            prefix: None,
            given: None,
            middle: None,
            family: None,
            suffix: Some("B.A. M.Div."),
        };
        assert_eq!(p.to_dicom_string(), "^^^^B.A. M.Div.".to_string());
        let p = PersonName {
            prefix: Some("Rev."),
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: Some("B.A. M.Div."),
        };
        assert_eq!(
            p.to_dicom_string(),
            "Adams^John^Robert^Rev.^B.A. M.Div.".to_string()
        );
        let p = PersonName {
            prefix: None,
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: Some("B.A. M.Div."),
        };
        assert_eq!(
            p.to_dicom_string(),
            "Adams^John^Robert^^B.A. M.Div.".to_string()
        );
        let p = PersonName {
            prefix: Some("Rev."),
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: None,
        };
        assert_eq!(p.to_dicom_string(), "Adams^John^Robert^Rev.".to_string());
        let p = PersonName {
            prefix: None,
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: None,
        };
        assert_eq!(p.to_dicom_string(), "Adams^John^Robert".to_string());
        let p = PersonName {
            prefix: None,
            given: None,
            middle: Some("Robert"),
            family: None,
            suffix: None,
        };
        assert_eq!(p.to_dicom_string(), "^^Robert".to_string());
    }
    #[test]
    fn test_person_name_to_string() {
        let p = PersonName {
            prefix: None,
            given: Some("John"),
            middle: None,
            family: Some("Adams"),
            suffix: None,
        };
        assert_eq!(p.to_string(), "John Adams".to_string());

        let p = PersonName {
            prefix: Some("Rev."),
            given: None,
            middle: None,
            family: None,
            suffix: None,
        };
        assert_eq!(p.to_string(), "Rev.".to_string());

        let p = PersonName {
            prefix: None,
            given: None,
            middle: None,
            family: None,
            suffix: Some("B.A. M.Div."),
        };
        assert_eq!(p.to_string(), "B.A. M.Div.".to_string());
        let p = PersonName {
            prefix: Some("Rev."),
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: Some("B.A. M.Div."),
        };
        assert_eq!(
            p.to_string(),
            "Rev. John Robert Adams B.A. M.Div.".to_string()
        );
        let p = PersonName {
            prefix: None,
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: Some("B.A. M.Div."),
        };
        assert_eq!(p.to_string(), "John Robert Adams B.A. M.Div.".to_string());
        let p = PersonName {
            prefix: Some("Rev."),
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: None,
        };
        assert_eq!(p.to_string(), "Rev. John Robert Adams".to_string());
        let p = PersonName {
            prefix: None,
            given: Some("John"),
            middle: Some("Robert"),
            family: Some("Adams"),
            suffix: None,
        };
        assert_eq!(p.to_string(), "John Robert Adams".to_string());
        let p = PersonName {
            prefix: None,
            given: None,
            middle: Some("Robert"),
            family: None,
            suffix: None,
        };
        assert_eq!(p.to_string(), "Robert".to_string());
    }
    #[test]
    fn person_name_from_slice() {
        assert_eq!(
            PersonName::from_slice("^^Robert"),
            PersonName {
                prefix: None,
                given: None,
                middle: Some("Robert"),
                family: None,
                suffix: None,
            }
        );
        assert_eq!(
            PersonName::from_slice("^^^Rev."),
            PersonName {
                prefix: Some("Rev."),
                given: None,
                middle: None,
                family: None,
                suffix: None,
            }
        );
        assert_eq!(
            PersonName::from_slice("^^^^B.A. M.Div."),
            PersonName {
                prefix: None,
                given: None,
                middle: None,
                family: None,
                suffix: Some("B.A. M.Div."),
            }
        );
        assert_eq!(
            PersonName::from_slice("^^Robert"),
            PersonName {
                prefix: None,
                given: None,
                middle: Some("Robert"),
                family: None,
                suffix: None,
            }
        );
        assert_eq!(
            PersonName::from_slice("^John"),
            PersonName {
                prefix: None,
                given: Some("John"),
                middle: None,
                family: None,
                suffix: None,
            }
        );
        assert_eq!(
            PersonName::from_slice("Adams"),
            PersonName {
                prefix: None,
                given: None,
                middle: None,
                family: Some("Adams"),
                suffix: None,
            }
        );
        assert_eq!(
            PersonName::from_slice("Adams^^^^B.A. M.Div."),
            PersonName {
                prefix: None,
                given: None,
                middle: None,
                family: Some("Adams"),
                suffix: Some("B.A. M.Div."),
            }
        );
        assert_eq!(
            PersonName::from_slice("Adams^^Robert^^B.A. M.Div."),
            PersonName {
                prefix: None,
                given: None,
                middle: Some("Robert"),
                family: Some("Adams"),
                suffix: Some("B.A. M.Div."),
            }
        );
        assert_eq!(
            PersonName::from_slice("Adams^John^Robert^Rev.^B.A. M.Div."),
            PersonName {
                prefix: Some("Rev."),
                given: Some("John"),
                middle: Some("Robert"),
                family: Some("Adams"),
                suffix: Some("B.A. M.Div."),
            }
        );
    }
}
