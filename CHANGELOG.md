# Changelog

Changes in published and prepared preview builds.

## 0.4.2 — Preview, build 8

- Mark Intel WoW64 engines as a fallback for 32-bit applications in the library, engine selection, and environment header.
- Keep the native ARM64/FEX engine first in the selection order; retaining an Intel engine does not switch existing environments.
- Explain the fallback's Rosetta dependency without promising availability in future macOS versions.

## 0.4.1 — Preview, build 7

- Replace engine dropdowns with cards showing Windows application bitness first and Mac architecture/Rosetta requirements separately.
- Show the same capability summary in the engine library and environment header.
- Infer Intel WoW64 support only from matching Windows module headers when no explicit runtime declaration exists; preserve explicit restrictions.
- Disable incompatible engine cards for launcher installers that require 32-bit support.

## 0.4.0 — Preview, build 6

- Add environment deletion from settings and the sidebar context menu, with confirmation and a scoped Wine stop.
- Add Vela Trash with restore and separately confirmed permanent deletion of prefix files, cached installers, and logs.
- Prevent deletion and launch/initialization operations from racing through per-environment locks.
- Add English and Russian interface resources, including catalog text, dialogs, progress, and core errors.
- Follow device languages by default; offer persistent language overrides in App settings and the standard Settings window.

## 0.3.0 — Preview, build 5

- Add a native game launcher catalog with ten official Windows installer presets.
- Download installers with HTTPS, exact redirect-host restrictions, progress, cancellation, and a 512 MiB limit.
- Create a separate environment, cache the installer, and open its interactive installation wizard.
- Detect known client paths, create shortcuts, and offer installer retry in the same environment.
- Show installer architecture requirements and block known 32-bit-only installer / 64-bit-only engine mismatches.
- Include a Riot Client information card describing the Vanguard limitation.

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
