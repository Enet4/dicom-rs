/// How to process retired entries
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RetiredOptions {
    /// ignore retired attributes
    Ignore,
    /// include retired attributes
    Include {
        /// mark constants as deprecated
        deprecate: bool,
    },
}

impl RetiredOptions {
    /// Create retired options from two flags.
    /// `ignore` takes precedence over `deprecate.
    pub fn from_flags(ignore: bool, deprecate: bool) -> Self {
        if ignore {
            RetiredOptions::Ignore
        } else {
            RetiredOptions::Include { deprecate }
        }
    }
}
