# Third-party components

Vela's original Rust and SwiftUI code is MIT licensed. It is an independent Wine front end.
The original manager and interface are implemented independently. Third-party libraries keep their own licenses.

## Rust dependencies

The precise versions and registry checksums are in `core/Cargo.lock`.

- serde and serde_derive — MIT OR Apache-2.0 — https://github.com/serde-rs/serde
- serde_json — MIT OR Apache-2.0 — https://github.com/serde-rs/json
- Transitive dependencies retain their own licenses. Their full notices and the
  corresponding crate sources are in `vendor/` in the source release. Build with
  `cargo vendor --locked vendor` when preparing a new release; retain those
  notices beside binary distributions.

## Optional, separate Wine engines

Wine is LGPL-2.1-or-later: https://github.com/wine-mirror/wine/blob/master/LICENSE

Wine is run as a separate program. Registering a local engine does not copy it into Vela.app.
The `Engines/` directory, when present in a local workspace, is a separately downloaded runtime, not original Vela source.
Its own license files, notices and matching-source obligations continue to apply.

macOS packages used for local validation:
https://github.com/Gcenx/macOS_Wine_builds

Before publishing a bundled engine, include complete corresponding sources for
its actual Wine build, patches, build scripts and dependencies; keep all component
licenses and notices. A generic link to upstream HEAD is not a substitute for
matching sources. The preview source archive excludes third-party Wine binaries.

Potential future graphics components, not installed or activated by Vela 0.2:
DXMT, DXVK, MoltenVK. Review the license at the exact version selected before shipping.
Apple D3DMetal and Rosetta are not open source components supplied by this project.

Native engine build inputs (separate from Vela.app): Wine 11.18 plus the
38 Wine and 5 FEX patches from Hadron v0.3.1 (BSD-3-Clause front end, individual
components retain Wine LGPL and FEX MIT licenses). Pinned revisions and archive
digests are recorded in `docs/native-runtime-sources.json`. FEX Unix libraries are
rebuilt for macOS 26.5; FEX PE DLLs originate from the verified upstream runtime.
No Hadron main app, Steam integration or graphics payload is used by Vela.
