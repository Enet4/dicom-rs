//! Core UID dictionary types

use std::str::FromStr;

/// UID dictionary entry type
pub trait UidDictionaryEntry {
    /// Get the UID proper.
    fn uid(&self) -> &str;

    /// Get the full name of the identifier.
    fn name(&self) -> &str;

    /// The alias of the UID, with no spaces, usually in UpperCamelCase.
    fn alias(&self) -> &str;

    /// Get whether the UID is retired.
    fn is_retired(&self) -> bool;
}

/// A data type for a dictionary entry using string slices
/// for its data.
#[derive(Debug, PartialEq, Clone)]
pub struct UidDictionaryEntryRef<'a> {
    /// The UID proper
    pub uid: &'a str,
    /// The full name of the identifier,
    /// which may contain spaces
    pub name: &'a str,
    /// The alias of the identifier,
    /// with no spaces, usually in UpperCamelCase
    pub alias: &'a str,
    /// The type of UID
    pub r#type: UidType,
    /// The _typical_  value representation of the attribute
    pub retired: bool,
}

impl<'a> UidDictionaryEntryRef<'a> {
    pub const fn new(
        uid: &'a str,
        name: &'a str,
        alias: &'a str,
        r#type: UidType,
        retired: bool,
    ) -> Self {
        UidDictionaryEntryRef {
            uid,
            name,
            alias,
            r#type,
            retired,
        }
    }
}

impl<'a> UidDictionaryEntry for UidDictionaryEntryRef<'a> {
    fn uid(&self) -> &str {
        self.uid
    }

    fn name(&self) -> &str {
        self.name
    }

    fn alias(&self) -> &str {
        self.alias
    }

    fn is_retired(&self) -> bool {
        self.retired
    }
}

/// Enum for all recognized UID types.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
pub enum UidType {
    /// SOP Class
    SopClass,
    /// Meta SOP Class
    MetaSopClass,
    /// Transfer Syntax
    TransferSyntax,
    /// Well-known SOP Instance
    WellKnownSopInstance,
    /// DICOM UIDs as a Coding Scheme
    DicomUidsAsCodingScheme,
    /// Coding Scheme
    CodingScheme,
    /// Application Context Name
    ApplicationContextName,
    /// Service Class
    ServiceClass,
    /// Application Hosting Model
    ApplicationHostingModel,
    /// Mapping Resource
    MappingResource,
    /// LDAP OID
    LdapOid,
    /// Synchronization Frame of Reference
    SynchronizationFrameOfReference,
}

impl FromStr for UidType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "SOP Class" => Ok(UidType::SopClass),
            "Meta SOP Class" => Ok(UidType::MetaSopClass),
            "Transfer Syntax" => Ok(UidType::TransferSyntax),
            "Well-known SOP Instance" => Ok(UidType::WellKnownSopInstance),
            "DICOM UIDs as a Coding Scheme" => Ok(UidType::DicomUidsAsCodingScheme),
            "Coding Scheme" => Ok(UidType::CodingScheme),
            "Application Context Name" => Ok(UidType::ApplicationContextName),
            "Service Class" => Ok(UidType::ServiceClass),
            "Application Hosting Model" => Ok(UidType::ApplicationHostingModel),
            "Mapping Resource" => Ok(UidType::MappingResource),
            "LDAP OID" => Ok(UidType::LdapOid),
            "Synchronization Frame of Reference" => Ok(UidType::SynchronizationFrameOfReference),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for UidType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            UidType::SopClass => "SOP Class",
            UidType::MetaSopClass => "Meta SOP Class",
            UidType::TransferSyntax => "Transfer Syntax",
            UidType::WellKnownSopInstance => "Well-known SOP Instance",
            UidType::DicomUidsAsCodingScheme => "DICOM UIDs as a Coding Scheme",
            UidType::CodingScheme => "Coding Scheme",
            UidType::ApplicationContextName => "Application Context Name",
            UidType::ServiceClass => "Service Class",
            UidType::ApplicationHostingModel => "Application Hosting Modle",
            UidType::MappingResource => "Mapping Resource",
            UidType::LdapOid => "LDAP OID",
            UidType::SynchronizationFrameOfReference => "Synchronization Frame of Reference",
        };
        f.write_str(str)
    }
}
