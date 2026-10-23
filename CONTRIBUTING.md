# Contributing to Vela

Thank you for helping improve Vela. The project is an early preview, and contributions should make its actual behavior easier to use, understand, and verify.

## Before opening an issue

Search existing issues first. Distinguish an interface or manager problem from a Windows application compatibility issue where possible. If you are unsure, describe what you observed.

For a useful bug report, include:

- Vela version and build number.
- macOS version and Mac processor.
- Exact Wine engine version, architecture, and source.
- Windows application's name, version, and executable architecture.
- Reproduction steps, expected behavior, and actual behavior.
- Relevant log output, with personal paths, tokens, and other private data removed.

Do not attach proprietary installers, license keys, or an entire Wine environment. Link to a legitimate application source instead.

For security issues, follow [SECURITY.md](SECURITY.md).

## Development setup

Use macOS with Swift 6, Xcode or Command Line Tools, and Rust/Cargo. Run commands from the repository root:

~~~sh
cargo test --locked --offline --manifest-path core/Cargo.toml
bash scripts/build.sh
~~~

The normal build does not require Wine. Real Wine integration checks require a separately installed, compatible engine.

The Rust core is in `core/`. The SwiftUI application is in `macos/`. Third-party Rust sources and notices are in `vendor/`. Wine patches and their separate license files are in `runtime/patches/`.

## Pull requests

Keep each pull request focused on one problem. Explain the user-visible change and how you checked it. Include a screenshot for a visible interface change when useful.

Preserve environment isolation, literal argument handling, existing user libraries, and the separation between the manager and external Wine engines. Do not silently switch an existing environment to a different engine.

Run the checks relevant to your change. Rust behavior changes should include meaningful regression coverage where appropriate. For a small visual change, build and inspect the affected interface rather than adding tests that only repeat the layout implementation.

Do not claim a Wine application or processor architecture works unless you tested it. State what remains unverified.

## Real Wine checks

Use a new disposable library directory. Never run integration tests against your normal Vela library.

`scripts/smoke_test.py` checks a separately installed engine with working 64-bit and WoW64 command processors. It is not suitable for the 64-bit-only experimental native runtime.

`scripts/native_smoke_test.py` checks the specific ARM64/FEX configuration using original Windows PE fixtures. See [Native runtime notes](docs/native-runtime.md) for its prerequisites.

## Dependencies and licensing

Retain third-party notices and keep `core/Cargo.lock` consistent with vendored sources. Explain dependency changes and their licenses.

Contributions to the original manager and interface are submitted under MIT. Changes to Wine-derived files must retain the applicable Wine license. Only submit code and assets you have the right to contribute; credit external sources and preserve their notices.

## Communication

Be respectful, explain disagreements with concrete examples, and avoid personal attacks. Suggestions and reports are welcome even if you do not have a code change ready.
