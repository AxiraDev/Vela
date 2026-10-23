# Vela 0.3.0 validation

Recorded on October 23, 2026, using an Apple M1 Mac running macOS 26.5.2. The current preview is app build 5.

## Manager and interface

- Fifteen Rust unit tests and one JSON protocol integration test passed.
- Coverage includes path validation, literal launch arguments, environment-variable isolation, state preservation, concurrent writes, engine lifecycle, Mach-O/PE headers, and loader symlink handling.
- DPI handling was checked for registry command arguments, saved state, and invalid-scale rejection.
- The SwiftUI interface built in Swift 6 mode.
- Locally signed app archives were extracted and their signatures verified.
- App icon display was checked in the standard About window.
- Sidebar shortcut-menu spacing and opening were inspected.
- Both support entry points opened the support sheet; its action opened the configured Boosty donation page in a browser. No payment was performed.

## Launcher catalog

- Official endpoints for the ten installer presets returned Windows EXE/MSI headers during bounded download checks.
- The Swift downloader completed a full official itch.io Windows installer download through its allowed HTTPS redirects. No publisher installer was executed by that transport check.
- Rust fixtures cover automatic environment creation, cached-installer retry, shortcut detection without duplication, rejection of HTML/invalid downloads and incompatible 32-bit installers, and refusal to follow client symlinks outside the prefix.
- The native interface's catalog, search, installation sheet, and engine-dependent button disabling were checked. Existing user environments remained visible after updating the app.
- A disposable real ARM64/FEX environment completed automatic creation, cached launch of an original external x64 PE fixture, client-path detection, idempotent shortcut creation, and scoped stop. This used Vela's own fixture in place of a publisher installer.
- Full launcher/game installation and compatibility have not been validated. Client detection is based on expected paths and is not an installer exit-status check.

## Experimental native runtime

All 59 Mach-O files in the locally prepared runtime were identified as ARM64. Wine and FEX Unix modules used a macOS 26.5 deployment target.

A fresh disposable environment passed:

~~~text
PASS ARM64 loader, server and FEX registration
PASS native wineboot
PASS real Windows registry: LogPixels=192 (200%)
PASS real AMD64 PE: VELA_WINDOWS_X64_OK
PASS macOS P_TRANSLATED=false
PASS unsupported x86 rejected before launch
PASS scoped native wineserver stop
~~~

The x64 fixture is an original external AMD64 executable. It performs and checks a calculation, writes its marker, and remains alive briefly for process inspection. It is not an ARM64 built-in Windows command processor.

The macOS translation probe was separately checked against native ARM64 and Rosetta x86_64 control processes, which returned false and true respectively.

These results establish a basic x64 execution path without Rosetta for this configuration. They do not establish compatibility with every Windows application or a future macOS version.

## Scope and limits

The normal application deployment target is macOS 14.0, but the tested system was macOS 26.5.2. Other Macs and older systems have not been validated.

The experimental native engine supports 64-bit Windows programs only. Mono/Gecko auto-installers are suppressed during initialization, not during application launches. Advanced graphics and multimedia packages are not included in that runtime.

GnuTLS initialization was tested; application-specific HTTPS behavior was not comprehensively tested. Wine logs can contain errors for optional devices or components.

Native engine binaries are separate from the app and source release. See [Native runtime notes](native-runtime.md) and [pinned inputs](native-runtime-sources.json).

## Reproduce manager checks

From the repository root:

~~~sh
cargo test --locked --offline --manifest-path core/Cargo.toml
bash scripts/build.sh
~~~

Wine integration checks need a separately installed engine and a new test library. See [CONTRIBUTING.md](../CONTRIBUTING.md) before running them.
