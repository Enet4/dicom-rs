# Contributing to DICOM-rs

The DICOM-rs project is open for external contributions
in the form of issues, pull requests, and otherwise constructive discussion
about design concerns and future perspectives of the project.
Although this project combines two major domains of expertize,
namely the DICOM standard and the Rust programming language,
it is acceptable for a contributor of DICOM-rs
to only possess basic knowledge in either one.

Please do not forget to follow the [Code of Conduct](CODE_OF_CONDUCT.md)
in all interactions through the given project communication venues.

## Contributing with code

Please check out the list of existing issues in the [GitHub issue tracker].
Should you be interested in helping but not know where to begin,
please look for issues tagged `help wanted` and `good first issue`.
Announcing your interest in pursuing an issue is recommended.
This will prevent people from concurrently working on the same issue,
and you may also receive some guidance along the way.
No need to be shy!

Pull requests are likely to be subjected to constructive and careful reviewing,
and it may take some time before they are accepted and merged.
Please do not be discouraged to contribute when not facing the expected outcome,
or feeling that your work or proposal goes unheard.
The project is primarily maintained by volunteers outside of work hours.

[GitHub issue tracker]: https://github.com/Enet4/dicom-rs/issues

### Building the project

The DICOM-rs ecosystem is built on Rust ecosystem.
it may use bindings for integration with other non-Rust libraries
providing additional capabilities, such as more image encodings,
but in this case they are automatically fetched and built
during the standard building process.

Cargo is the main tool for building all crates in DICOM-rs.
[Rustup] is the recommended way to set up a development environment
for working with Rust.
See also the current [Minimum Supported Rust Version (MSRV) policy][msrv].

Currently, all crates are gathered in the same workspace,
which means that running the command below
at the root of the repository will build all crates:

```sh
cargo build
```

This will also build the CLI and helper tools of the project,
such as the dictionary builder and `dcmdump`.
To build only the library crates,
you can build the parent package named `dicom`:

```sh
cargo build -p dicom
```

Please ensure that all tests pass before sending your contribution.
Writing tests for your own contributions is greatly appreciated as well.

```sh
cargo test
```

We also recommend formatting your code before submitting,
to prevent discrepancies in code style.

```sh
cargo fmt
```

[Rustup]: https://rustup.rs
[msrv]: README.md#Minimum-Supported-Rust-version

## Discussion and roadmapping

If you have more long-termed ideas about what DICOM-rs should include next,
please have a look at the [roadmap] and look into existing issues to provide feedback.
You can also talk about the project at the official [DICOM-rs Zulip organization][zulip].

If you have any further questions or concerns,
or would like to be deeper involved in the project,
please reach out to the project maintainers.

[roadmap]: https://github.com/Enet4/dicom-rs/wiki/Roadmap
[zulip]: https://dicom-rs.zulipchat.com

## Project team and governance

DICOM-rs is currently led by Eduardo Pinho ([**@Enet4**](https://github.com/Enet4), <enet4mikeenet@gmail.com>).
