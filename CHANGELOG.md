# Changelog

This file describes the prepared preview. Publication dates and future release entries will be added when releases are published.

## 0.2.0 — Preview, build 4

### Available

- Native SwiftUI interface with a separate Rust process for library and engine management.
- Multiple registered Wine engines and separate 64-bit environments.
- EXE/MSI launch, shortcuts, launch arguments, and working-directory settings.
- BAT/CMD launch through the Windows command processor.
- Wine configuration, Registry Editor, Windows console, and uninstaller access.
- Per-environment variables and operation logs.
- Windows interface scaling from 100% to 300%, with a 200% Retina default for new environments.
- Engine architecture inspection and explicit Rosetta dependency labels.
- Experimental ARM64/FEX engine recognition and 32-bit PE rejection for engines that declare 64-bit-only support.
- Scoped environment stop and handling of an already stopped Wine server.
- Atomic state-file updates and serialization of concurrent writes.
- Vela application icon and optional Boosty support link.
- English publication and contributor documentation; the current application interface remains in Russian.

### Validation

- Ten Rust unit tests and one JSON protocol integration test passed.
- The SwiftUI app built in Swift 6 mode and its local signature was verified.
- A separate ARM64/FEX runtime initialized a fresh environment, applied 192 DPI, and ran an original x64 PE test without the macOS translated-process flag.
- The support page transition was verified in a browser.

### Preview limits

Wine engine binaries are supplied separately. The locally validated native runtime requires macOS 26.5 or later and supports 64-bit Windows applications only.

Automatic engine updates, environment migration, snapshots, advanced graphics management, Developer ID signing, and notarization are not implemented.
