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

## AI Policy

DICOM-rs, as a open source software project,
is a social endeavor as much as it is a technical one.
One cannot set that aside and and still have a balanced and sustainable FOSS initiative.
This is why, although external contributions are highly appreciated,
contributors are expected to follow these guidelines in good faith,
including the AI policy described next.

For the time being and until the contributor guidelines are updated again,
the stance for the DICOM-rs project is that
any kind of contribution to its repositories should be genuinely authored by a human.

- Please refrain from providing content which was _substantially generated_ by LLMs.
  - What is deemed "substantially generated" will be assessed by maintainers. When in doubt, prefer writing it yourself, which allows you to confidently claim authorship of the contribution and leave less uncertainty in terms of whether you have copyright for those changes.
  - Commits authored or co-authored by coding agent profiles (such as copilot or claude) are not allowed. Do not include files which are meant to be primarily read by coding agents (e.g. `AGENTS.md`, `CLAUDE.md`, ...).
- All descriptions and comments on repositories and other official communication venues (GitHub, Zulip) should be written by you rather than generated automatically based on context.
  - In pull requests, for example, depending on the size of the contribution, a brief paragraph of context and a summary are often enough, even when there is no associated issue already.
  - When using LLM-based technology to analyze and review code, carefully assess and test all outputs before reporting the curated findings, while refraining from reporting those which you have not understood.
- As has been in the full lifetime of DICOM-rs, project maintainers will determine whether submissions are reasonably reviewable and acceptable, without the obligation of having to explain these decisions.

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

This will also build the various
command line tools and development tools of the project,
such as the dictionary builder.
To build only the library crates,
you can build the parent package named `dicom` or add the `--lib` option:

```sh
cargo build --lib
```

It is recommended that you ensure that all tests pass before sending your contribution.
Writing tests for your own contributions is greatly appreciated as well.

```sh
cargo test
```

Some capabilities are gated behind Cargo features.
Do not forget to test your contributions with and without relevant features
to ensure that a change compiles under different combinations.

```sh
# compile with many more features
cargo test --features=image,ndarray,sop-class,rle,cli,jpegxl,tls
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

## Security policy

Potential vulnerabilities can be reported in private to one of the maintainers by email,
or through the vulnerability reporting mechanism on GitHub.
Please note that, similarly to proposing other contributions,
the bandwidth available to attend to the report in a timely fashion may be limited.

## Project team and governance

DICOM-rs is currently led by Eduardo Pinho ([**@Enet4**](https://github.com/Enet4), <enet4mikeenet@gmail.com>).
