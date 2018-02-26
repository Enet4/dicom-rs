# DICOM-rs

[![Build Status](https://travis-ci.org/Enet4/dicom-rs.svg?branch=master)](https://travis-ci.org/Enet4/dicom-rs) [![dependency status](https://deps.rs/repo/github/Enet4/dicom-rs/status.svg)](https://deps.rs/repo/github/Enet4/dicom-rs)


An efficient and practical base library for DICOM compliant systems.

At its core, this library is a pure Rust implementation of the DICOM representation format,
allowing users to read and write DICOM objects over files and other sources, while
remaining intrinsically fast and safe to use.

This project is a work in progress. Nevertheless, the first usable version may hopefully arrive
eventually. Any feedback during the development of these solutions is welcome.

## Components

- [`core`](core) represents all of the base traits, data structures and functions for
 reading and writing DICOM content.
- [`dictionary_builder`](dictionary_builder) is a Rust application that generates code and
 other data structures for a DICOM standard dictionary using entries from the web.
