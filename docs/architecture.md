# Architecture

Vela consists of a native SwiftUI interface and a separate Rust command-line core. Wine engines are external programs registered by the user.

## Execution flow

~~~mermaid
flowchart TD
    UI[SwiftUI interface] -->|One JSON request and reply| Core[Rust: vela-core]
    Core --> State[Library state]
    Core --> Logs[Operation logs]
    Core -->|WINEPREFIX and arguments| Wine[Selected Wine engine]
    Wine --> Prefix[Windows drive and registry]
    Wine --> Mac[macOS windows, files, and devices]
~~~

The interface invokes a short-lived core process away from the main UI thread. Each request and reply is a single JSON line. The interface waits up to 180 seconds for initialization and 30 seconds for other operations.

The core starts Wine with an argument array rather than a macOS shell. It sets the selected prefix, matching Wine loader/server paths, working directory, and supported environment variables explicitly. Long-lived Windows programs write to log files rather than holding the core's output pipe open.

A successful launch request means that the process was handed to Wine. It does not confirm that the Windows application finished loading.

## Engine registration

The core reads Mach-O headers to identify ARM64, x86_64, and universal engines. It looks for the engine's matching wineserver and rejects an ARM64 loader paired with an Intel-only server.

Native engines appear first in the interface. Intel engines are labelled as requiring Rosetta on Apple Silicon. An environment remains attached to its selected engine; there is no automatic fallback to another architecture.

An optional `vela-runtime.json` file declares whether an engine supports 32-bit Windows executables. For a runtime declaring `supports_x86_32: false`, Vela rejects recognized i386 PE launch targets. This does not inspect every payload or child process inside an installer.

## Wine, CPU translation, and graphics

Wine implements Windows APIs and Windows execution behavior. Its components handle DLL loading, the registry, processes, windows, files, and other operating-system services. Wineserver coordinates shared Wine objects and state.

CPU instruction translation is a separate concern. On Apple Silicon, an Intel Wine engine uses Rosetta. The experimental native configuration uses ARM64 Wine with FEX for Windows x64 instructions.

Graphics backends have their own compatibility and release requirements. The preview does not install or manage DXMT, DXVK, MoltenVK, or other graphics packages.

Upstream projects: [Wine](https://github.com/wine-mirror/wine), [FEX](https://github.com/FEX-Emu/FEX), and [macOS Wine builds](https://github.com/Gcenx/macOS_Wine_builds).

## Storage

The normal root is `~/Library/Application Support/Vela`. It contains:

| Location | Purpose |
| --- | --- |
| `state.json` | Engines, environments, shortcuts, variables, and saved scaling |
| `prefixes/<id>/` | Environment files and Windows registry |
| `logs/` | Per-operation output |

State updates use a temporary file, synchronization, atomic rename, and a lock to serialize concurrent writers. Corrupt state or an unsupported future schema is reported instead of being overwritten with an empty library.

Engine paths and shortcut targets are absolute. Portable mode changes the library root but does not rewrite paths or migrate data.

## Initialization, scaling, and stop

Initialization runs wineboot and checks its exit status. Because Wine saves registry files asynchronously, the core then waits up to 20 seconds for `system.reg`.

Windows scaling writes the environment's `HKCU\Control Panel\Desktop\LogPixels` registry value. Existing windows need to be restarted to use the new setting. Vela does not kill them as part of a scale change.

Stopping an environment invokes its matching wineserver with its explicit prefix. An empty-output exit code of 1 is treated as an already stopped server.

## Updates

The app, Wine source releases, macOS binary distributions, FEX, and graphics libraries have independent release cycles.

The current manager performs no engine downloads or automatic updates. Install versions side by side and test a different engine in a new environment. Updating a prefix can change its registry and installed files; returning to an older Wine binary alone is not a complete rollback.

A future update system is intended to use signed manifests, verified archives, separate engine versions, and stopped-environment snapshots. These features are not implemented in the preview.

## Security boundary

An environment separates Windows configuration. It does not prevent Windows code from accessing files available to the macOS user. See [Security policy](../SECURITY.md).

## Licenses

The original manager and interface use MIT. Wine, FEX, vendored Rust crates, and Wine-derived patches retain their own licenses. See [Third-party components](../THIRD_PARTY.md).
