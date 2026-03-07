# DICOM-rs `ul`

[![CratesIO](https://img.shields.io/crates/v/dicom-ul.svg)](https://crates.io/crates/dicom-ul)
[![Documentation](https://docs.rs/dicom-ul/badge.svg)](https://docs.rs/dicom-ul)

An implementation of the DICOM upper layer protocol.
This crate contains the types and methods needed
to interact with DICOM nodes through the upper layer protocol.
It can be used as a base for finite-state machines and higher-level helpers,
enabling the creation of concrete
service class users (SCUs) and service class providers (SCPs).
TLS support for secure transport connections
is also available via [Rustls](https://crates.io/crates/rustls).

Examples of DICOM network tools constructed using `dicom-ul` include
[dicom-storescp](https://crates.io/crates/dicom-storescp),
[dicom-storescu](https://crates.io/crates/dicom-storescu),
and [dicom-findscu](https://crates.io/crates/dicom-findscu).

This crate is part of the [DICOM-rs](https://github.com/Enet4/dicom-rs) project
and is contained by the parent crate [`dicom`](https://crates.io/crates/dicom)
for convenience.

## Testing

### TLS

TLS testing requires a Certificate authority, and signed client/server key pairs

A function is provided within `tests/association.rs` which:

1. Creates a CA with a `US` country code
2. Creates certificate signing requests for two "clients" (one client and one server)
3. Signs the certificates using the CA

When finished, there should be a `.pem`, and a `.key.pem` for the client, server and CA client and server,

You can change the country or the IP/DNS of the configured
client/server
by modifying `country_name` and/or `organization_name` in the test code.