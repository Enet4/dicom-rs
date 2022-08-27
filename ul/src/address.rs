//! Data types for addresses to nodes in DICOM networks.

use std::{
    net::{AddrParseError, SocketAddr, ToSocketAddrs},
    str::FromStr,
};

use snafu::{ResultExt, Snafu};

/// A specification for a full address to the target SCP:
/// an application entity title, plus a network socket address.
///
/// These addresses can be serialized and parsed
/// with the syntax `{ae_title}@{socket_address}`.
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
/// let addr: FullAeAddr = "SCP-STORAGE@127.0.0.1:104".parse()?;
/// assert_eq!(addr.ae_title(), "SCP-STORAGE");
/// assert_eq!(addr.socket_addr(), SocketAddr::from(([127, 0, 0, 1], 104)));
/// assert_eq!(&addr.to_string(), "SCP-STORAGE@127.0.0.1:104");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FullAeAddr {
    ae_title: String,
    socket_addr: std::net::SocketAddr,
}

impl FullAeAddr {
    /// Create an AE address from its bare constituent parts.
    pub fn new(ae_title: impl Into<String>, socket_addr: SocketAddr) -> Self {
        FullAeAddr {
            ae_title: ae_title.into(),
            socket_addr,
        }
    }

    /// Retrieve the application entity title portion.
    pub fn ae_title(&self) -> &str {
        &self.ae_title
    }

    /// Retrieve the socket address portion.
    pub fn socket_addr(&self) -> std::net::SocketAddr {
        self.socket_addr
    }
}

impl From<(String, SocketAddr)> for FullAeAddr {
    fn from((ae_title, socket_addr): (String, SocketAddr)) -> Self {
        Self::new(ae_title, socket_addr)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Snafu)]
pub enum ParseAeAddressError {
    /// Missing `@` in full AE address
    MissingPart,

    /// Could not parse socket address
    ParseSocketAddress { source: AddrParseError },
}

impl FromStr for FullAeAddr {
    type Err = ParseAeAddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((ae_title, addr)) = s.split_once('@') {
            Ok(FullAeAddr {
                ae_title: ae_title.to_string(),
                socket_addr: addr.parse().context(ParseSocketAddressSnafu)?,
            })
        } else {
            Err(ParseAeAddressError::MissingPart)
        }
    }
}

impl ToSocketAddrs for FullAeAddr {
    type Iter = std::option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        self.socket_addr.to_socket_addrs()
    }
}

impl std::fmt::Display for FullAeAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ae_title)?;
        f.write_str("@")?;
        std::fmt::Display::fmt(&self.socket_addr, f)
    }
}

/// A specification for an address to the target SCP:
/// a network socket address
/// which may also include an application entity title.
///
/// These addresses can be serialized and parsed
/// with the syntax `{ae_title}@{socket_address}`,
/// where the
///
/// For the version of the struct in which the AE title part is mandatory,
/// see [`FullAeAddr`].
///
/// # Example
///
/// ```
/// # use dicom_ul::{AeAddr, FullAeAddr};
/// # use std::net::SocketAddr;
/// #
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let addr: AeAddr = "SCP-STORAGE@127.0.0.1:104".parse()?;
/// assert_eq!(addr.ae_title(), Some("SCP-STORAGE"));
/// assert_eq!(addr.socket_addr(), SocketAddr::from(([127, 0, 0, 1], 104)));
/// assert_eq!(&addr.to_string(), "127.0.0.1:104");
///
/// // AE title can be missing
/// let addr: AeAddr = "192.168.1.99:1045".parse()?;
/// assert_eq!(addr.ae_title(), None);
/// // but can be provided later
/// let full_addr: FullAeAddr = addr.with_ae_title("SCP-QUERY");
/// assert_eq!(full_addr.ae_title(), "SCP-QUERY");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AeAddr {
    ae_title: Option<String>,
    socket_addr: std::net::SocketAddr,
}

impl AeAddr {
    /// Create an AE address from its bare constituent parts.
    pub fn new(ae_title: impl Into<String>, socket_addr: SocketAddr) -> Self {
        AeAddr {
            ae_title: Some(ae_title.into()),
            socket_addr,
        }
    }

    /// Create an AE address containing only a socket address.
    pub fn new_socket_addr(socket_addr: SocketAddr) -> Self {
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
    pub fn socket_addr(&self) -> std::net::SocketAddr {
        self.socket_addr
    }

    /// Create a new address with the full application entity target,
    /// discarding any potentially existing AE title.
    pub fn with_ae_title(self, ae_title: impl Into<String>) -> FullAeAddr {
        FullAeAddr {
            ae_title: ae_title.into(),
            socket_addr: self.socket_addr,
        }
    }

    /// Create a new address with the full application entity target,
    /// using the given AE title if it is missing.
    pub fn with_default_ae_title(self, ae_title: impl Into<String>) -> FullAeAddr {
        FullAeAddr {
            ae_title: self.ae_title.unwrap_or_else(|| ae_title.into()),
            socket_addr: self.socket_addr,
        }
    }
}

/// This conversion provides an address without an AE title.
impl From<SocketAddr> for AeAddr {
    fn from(socket_addr: SocketAddr) -> Self {
        AeAddr {
            ae_title: None,
            socket_addr,
        }
    }
}

impl From<FullAeAddr> for AeAddr {
    fn from(full: FullAeAddr) -> Self {
        AeAddr {
            ae_title: Some(full.ae_title),
            socket_addr: full.socket_addr,
        }
    }
}

impl FromStr for AeAddr {
    type Err = std::net::AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((ae_title, address)) = s.split_once('@') {
            Ok(AeAddr {
                ae_title: Some(ae_title.to_string()),
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

impl ToSocketAddrs for AeAddr {
    type Iter = std::option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        self.socket_addr.to_socket_addrs()
    }
}

impl std::fmt::Display for AeAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ae_title) = &self.ae_title {
            f.write_str(ae_title)?;
            f.write_str("@")?;
        }

        std::fmt::Display::fmt(&self.socket_addr, f)
    }
}
