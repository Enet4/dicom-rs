//! Data types for addresses to nodes in DICOM networks.
//!
//! This module provides the definitions for [`FullAeAddr`] and [`AeAddr`],
//! which enable consumers to couple a socket address with an expected
//! application entity (AE) title.
//!
//! The syntax is `«ae_title»@«network_address»:«port»`,
//! which works not only with IPv4 and IPv6 addresses,
//! but also with domain names.
use std::{
    convert::TryFrom,
    net::{SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
    str::FromStr,
};

use snafu::{ensure, AsErrorSource, ResultExt, Snafu};

/// A specification for a full address to the target SCP:
/// an application entity title, plus a generic  address,
/// typically a socket address.
///
/// These addresses can be serialized and parsed
/// with the syntax `{ae_title}@{address}`,
/// where the socket address is parsed according to
/// the expectations of the parameter type `T`.
///
/// For the version of the struct without a mandatory AE title,
/// see [`AeAddr`].
///
/// # Example
///
/// ```
/// # use dicom_ul::FullAeAddr;
/// # use std::net::SocketAddr;
/// #
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # // socket address can be a string
/// let addr: FullAeAddr<String> = "SCP-STORAGE@127.0.0.1:104".parse()?;
/// assert_eq!(addr.ae_title(), "SCP-STORAGE");
/// assert_eq!(addr.socket_addr(), "127.0.0.1:104");
/// # // or anything else which can be parsed into a socket address
/// let addr: FullAeAddr<SocketAddr> = "SCP-STORAGE@127.0.0.1:104".parse()?;
/// assert_eq!(addr.ae_title(), "SCP-STORAGE");
/// assert_eq!(addr.socket_addr(), &SocketAddr::from(([127, 0, 0, 1], 104)));
/// assert_eq!(&addr.to_string(), "SCP-STORAGE@127.0.0.1:104");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FullAeAddr<T> {
    ae_title: String,
    socket_addr: T,
}

impl<T> FullAeAddr<T> {
    /// Create an AE address from its bare constituent parts.
    pub fn new(ae_title: impl Into<String>, socket_addr: T) -> Self {
        FullAeAddr {
            ae_title: ae_title.into(),
            socket_addr,
        }
    }

    /// Retrieve the application entity title portion.
    pub fn ae_title(&self) -> &str {
        &self.ae_title
    }

    /// Retrieve the network address portion.
    pub fn socket_addr(&self) -> &T {
        &self.socket_addr
    }

    /// Convert the full address into its constituent parts.
    pub fn into_parts(self) -> (String, T) {
        (self.ae_title, self.socket_addr)
    }
}

impl<T> From<(String, T)> for FullAeAddr<T> {
    fn from((ae_title, socket_addr): (String, T)) -> Self {
        Self::new(ae_title, socket_addr)
    }
}

/// A error which occurred when parsing an AE address.
#[derive(Debug, Clone, Eq, PartialEq, Snafu)]
pub enum ParseAeAddressError<E>
where
    E: std::fmt::Debug + AsErrorSource,
{
    /// Missing `@` in full AE address
    MissingPart,

    /// Could not parse network socket address
    ParseSocketAddress { source: E },
}

impl<T> FromStr for FullAeAddr<T>
where
    T: FromStr,
    T::Err: std::fmt::Debug + AsErrorSource,
{
    type Err = ParseAeAddressError<<T as FromStr>::Err>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // !!! there should be a way to escape the `@`
        if let Some((ae_title, addr)) = s.split_once('@') {
            ensure!(!ae_title.is_empty(), MissingPartSnafu);
            Ok(FullAeAddr {
                ae_title: ae_title.to_string(),
                socket_addr: addr.parse().context(ParseSocketAddressSnafu)?,
            })
        } else {
            Err(ParseAeAddressError::MissingPart)
        }
    }
}

impl<T> ToSocketAddrs for FullAeAddr<T>
where
    T: ToSocketAddrs,
{
    type Iter = T::Iter;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        self.socket_addr.to_socket_addrs()
    }
}

impl<T> std::fmt::Display for FullAeAddr<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ae_title.replace("@", "\\@"))?;
        f.write_str("@")?;
        std::fmt::Display::fmt(&self.socket_addr, f)
    }
}

/// A specification for an address to the target SCP:
/// a generic network socket address
/// which may also include an application entity title.
///
/// These addresses can be serialized and parsed
/// with the syntax `{ae_title}@{address}`,
/// where the socket address is parsed according to
/// the expectations of the parameter type `T`.
///
/// For the version of the struct in which the AE title part is mandatory,
/// see [`FullAeAddr`].
///
/// # Example
///
/// ```
/// # use dicom_ul::{AeAddr, FullAeAddr};
/// # use std::net::{SocketAddr, SocketAddrV4};
/// #
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let addr: AeAddr<SocketAddrV4> = "SCP-STORAGE@127.0.0.1:104".parse()?;
/// assert_eq!(addr.ae_title(), Some("SCP-STORAGE"));
/// assert_eq!(addr.socket_addr(), &SocketAddrV4::new([127, 0, 0, 1].into(), 104));
/// assert_eq!(&addr.to_string(), "SCP-STORAGE@127.0.0.1:104");
///
/// // AE title can be missing
/// let addr: AeAddr<String> = "192.168.1.99:1045".parse()?;
/// assert_eq!(addr.ae_title(), None);
/// // but can be provided later
/// let full_addr: FullAeAddr<_> = addr.with_ae_title("SCP-QUERY");
/// assert_eq!(full_addr.ae_title(), "SCP-QUERY");
/// assert_eq!(&full_addr.to_string(), "SCP-QUERY@192.168.1.99:1045");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AeAddr<T> {
    ae_title: Option<String>,
    socket_addr: T,
}

impl<T> AeAddr<T> {
    /// Create an AE address from its bare constituent parts.
    pub fn new(ae_title: impl Into<String>, socket_addr: T) -> Self {
        AeAddr {
            ae_title: Some(ae_title.into()),
            socket_addr,
        }
    }

    /// Create an address with a missing AE title.
    pub fn new_socket_addr(socket_addr: T) -> Self {
        AeAddr {
            ae_title: None,
            socket_addr,
        }
    }

    /// Retrieve the application entity title portion, if present.
    pub fn ae_title(&self) -> Option<&str> {
        self.ae_title.as_deref()
    }

    /// Retrieve the socket address portion.
    pub fn socket_addr(&self) -> &T {
        &self.socket_addr
    }

    /// Create a new address with the full application entity target,
    /// discarding any potentially existing AE title.
    pub fn with_ae_title(self, ae_title: impl Into<String>) -> FullAeAddr<T> {
        FullAeAddr {
            ae_title: ae_title.into(),
            socket_addr: self.socket_addr,
        }
    }

    /// Create a new address with the full application entity target,
    /// using the given AE title if it is missing.
    pub fn with_default_ae_title(self, ae_title: impl Into<String>) -> FullAeAddr<T> {
        FullAeAddr {
            ae_title: self.ae_title.unwrap_or_else(|| ae_title.into()),
            socket_addr: self.socket_addr,
        }
    }

    /// Convert the address into its constituent parts.
    pub fn into_parts(self) -> (Option<String>, T) {
        (self.ae_title, self.socket_addr)
    }
}

/// This conversion provides a socket address without an AE title.
impl From<SocketAddr> for AeAddr<SocketAddr> {
    fn from(socket_addr: SocketAddr) -> Self {
        AeAddr {
            ae_title: None,
            socket_addr,
        }
    }
}

/// This conversion provides an IPv4 socket address without an AE title.
impl From<SocketAddrV4> for AeAddr<SocketAddrV4> {
    fn from(socket_addr: SocketAddrV4) -> Self {
        AeAddr {
            ae_title: None,
            socket_addr,
        }
    }
}

/// This conversion provides an IPv6 socket address without an AE title.
impl From<SocketAddrV6> for AeAddr<SocketAddrV6> {
    fn from(socket_addr: SocketAddrV6) -> Self {
        AeAddr {
            ae_title: None,
            socket_addr,
        }
    }
}

impl<T> From<FullAeAddr<T>> for AeAddr<T> {
    fn from(full: FullAeAddr<T>) -> Self {
        AeAddr {
            ae_title: Some(full.ae_title),
            socket_addr: full.socket_addr,
        }
    }
}

impl<T> FromStr for AeAddr<T>
where
    T: FromStr,
{
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // !!! there should be a way to escape the `@`
        if let Some((ae_title, address)) = s.split_once('@') {
            Ok(AeAddr {
                ae_title: Some(ae_title)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                socket_addr: address.parse()?,
            })
        } else {
            Ok(AeAddr {
                ae_title: None,
                socket_addr: s.parse()?,
            })
        }
    }
}

impl<'a> TryFrom<&'a str> for AeAddr<String> {
    type Error = <AeAddr<String> as FromStr>::Err;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl<T> ToSocketAddrs for AeAddr<T>
where
    T: ToSocketAddrs,
{
    type Iter = T::Iter;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        self.socket_addr.to_socket_addrs()
    }
}

impl<T> std::fmt::Display for AeAddr<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let socket_addr = self.socket_addr.to_string();
        if let Some(ae_title) = &self.ae_title {
            f.write_str(&ae_title.replace("@", "\\@"))?;
            f.write_str("@")?;
        } else if socket_addr.contains("@") {
            // if formatted socket address contains a `@`,
            // we need to start the output with `@`
            // so that the start of the socket address
            // is not interpreted as an AE title
            f.write_str("@")?;
        }

        std::fmt::Display::fmt(&socket_addr, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ae_addr_parse() {
        // socket address can be a string
        let addr: FullAeAddr<String> = "SCP-STORAGE@127.0.0.1:104".parse().unwrap();
        assert_eq!(addr.ae_title(), "SCP-STORAGE");
        assert_eq!(addr.socket_addr(), "127.0.0.1:104");

        // or anything else which can be parsed into a socket address
        let addr: FullAeAddr<SocketAddr> = "SCP_STORAGE@127.0.0.1:104".parse().unwrap();
        assert_eq!(addr.ae_title(), "SCP_STORAGE");
        assert_eq!(addr.socket_addr(), &SocketAddr::from(([127, 0, 0, 1], 104)));
        assert_eq!(&addr.to_string(), "SCP_STORAGE@127.0.0.1:104");

        // IPv4 socket address
        let addr: FullAeAddr<SocketAddrV4> = "MAMMOSTORE@10.0.0.11:104".parse().unwrap();
        assert_eq!(addr.ae_title(), "MAMMOSTORE");
        assert_eq!(
            addr.socket_addr(),
            &SocketAddrV4::new([10, 0, 0, 11].into(), 104)
        );
        assert_eq!(&addr.to_string(), "MAMMOSTORE@10.0.0.11:104");
    }

    /// test addresses without an AE title
    #[test]
    fn ae_addr_parse_no_ae() {
        // should fail
        let res = FullAeAddr::<String>::from_str("pacs.hospital.example.com:104");
        assert!(matches!(res, Err(ParseAeAddressError::MissingPart)));
        // should also fail (AE title can't be empty)
        let res = FullAeAddr::<String>::from_str("@pacs.hospital.example.com:104");
        assert!(matches!(res, Err(ParseAeAddressError::MissingPart)));

        // should return an ae addr with no AE title
        let addr: AeAddr<String> = "pacs.hospital.example.com:104".parse().unwrap();
        assert_eq!(addr.ae_title(), None);
        assert_eq!(addr.socket_addr(), "pacs.hospital.example.com:104");
        // should also return an ae addr with no AE title
        let addr: AeAddr<String> = "@pacs.hospital.example.com:104".parse().unwrap();
        assert_eq!(addr.ae_title(), None);
        assert_eq!(addr.socket_addr(), "pacs.hospital.example.com:104");
    }

    #[test]
    fn ae_addr_parse_weird_scenarios() {
        // can parse addresses with multiple @'s
        let addr: FullAeAddr<String> = "ABC@DICOM@pacs.archive.example.com:104".parse().unwrap();
        assert_eq!(addr.ae_title(), "ABC");
        assert_eq!(addr.socket_addr(), "DICOM@pacs.archive.example.com:104");
        assert_eq!(&addr.to_string(), "ABC@DICOM@pacs.archive.example.com:104");
    }
}
