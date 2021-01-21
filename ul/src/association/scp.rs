use dicom_encoding::transfer_syntax::TransferSyntaxIndex;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

/// Check that a transfer syntax repository
/// supports the given transfer syntax,
/// meaning that it can parse and decode DICOM data sets.
///
/// ```
/// # use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
/// # use dicom_ul::association::scp::is_supported_with_repo;
/// // Implicit VR Little Endian is guaranteed to be supported
/// assert!(is_supported_with_repo(TransferSyntaxRegistry, "1.2.840.10008.1.2"));
/// ```
pub fn is_supported_with_repo<R>(ts_repo: R, ts_uid: &str) -> bool
where
    R: TransferSyntaxIndex,
{
    ts_repo.get(ts_uid).filter(|ts| !ts.unsupported()).is_some()
}

/// Check that the main transfer syntax registry
/// supports the given transfer syntax,
/// meaning that it can parse and decode DICOM data sets.
///
/// ```
/// # use dicom_ul::association::scp::is_supported;
/// // Implicit VR Little Endian is guaranteed to be supported
/// assert!(is_supported("1.2.840.10008.1.2"));
/// ```
pub fn is_supported(ts_uid: &str) -> bool {
    is_supported_with_repo(TransferSyntaxRegistry, ts_uid)
}

/// From a sequence of transfer syntaxes,
/// choose the first transfer syntax to be supported
/// by the given transfer syntax repository.
pub fn choose_supported_with_repo<R, I, T>(ts_repo: R, it: I) -> Option<T>
where
    R: TransferSyntaxIndex,
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    it.into_iter()
        .find(|ts| is_supported_with_repo(&ts_repo, ts.as_ref()))
}

/// From a sequence of transfer syntaxes,
/// choose the first transfer syntax to be supported
/// by the main transfer syntax registry.
pub fn choose_supported<I, T>(it: I) -> Option<T>
where
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    it.into_iter().find(|ts| is_supported(ts.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::choose_supported;

    #[test]
    fn test_choose_supported() {
        assert_eq!(choose_supported(vec!["1.1.1.1.1"]), None,);

        // string slices, impl VR first
        assert_eq!(
            choose_supported(vec!["1.2.840.10008.1.2", "1.2.840.10008.1.2.1"]),
            Some("1.2.840.10008.1.2"),
        );

        // heap allocated strings slices, expl VR first
        assert_eq!(
            choose_supported(vec![
                "1.2.840.10008.1.2.1".to_string(),
                "1.2.840.10008.1.2".to_string()
            ]),
            Some("1.2.840.10008.1.2.1".to_string()),
        );
    }
}
