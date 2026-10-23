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

The interface invokes a short-lived core process away from the main UI thread. Each request and reply is a single JSON line. The interface waits up to 180 seconds for initialization and launcher setup, and 30 seconds for other core operations. Network downloads run separately through URLSession.

The core starts Wine with an argument array rather than a macOS shell. It sets the selected prefix, matching Wine loader/server paths, working directory, and supported environment variables explicitly. Long-lived Windows programs write to log files rather than holding the core's output pipe open.

A successful launch request means that the process was handed to Wine. It does not confirm that the Windows application finished loading.

## Engine registration

The core reads Mach-O headers to identify ARM64, x86_64, and universal engines. It looks for the engine's matching wineserver and rejects an ARM64 loader paired with an Intel-only server.

Native engines appear first in the interface. Intel engines are labelled as requiring Rosetta on Apple Silicon. An environment remains attached to its selected engine; there is no automatic fallback to another architecture.

An optional `vela-runtime.json` file declares whether an engine supports 32-bit Windows executables. When no declaration exists, an Intel engine with matching PE headers for i386 `ntdll.dll` and x86_64 `ntdll.dll`, `wow64.dll`, `wow64cpu.dll`, and `wow64win.dll` is identified as providing WoW64. Missing or malformed modules leave support unknown. An explicit declaration, including a restriction, always wins; ARM64/Intel alone is not used to infer 32-bit support. For a runtime declaring `supports_x86_32: false`, Vela rejects recognized i386 PE launch targets. This does not inspect every payload or child process inside an installer.

## Wine, CPU translation, and graphics

Wine implements Windows APIs and Windows execution behavior. Its components handle DLL loading, the registry, processes, windows, files, and other operating-system services. Wineserver coordinates shared Wine objects and state.

CPU instruction translation is a separate concern. On Apple Silicon, an Intel Wine engine uses Rosetta. The experimental native configuration uses ARM64 Wine with FEX for Windows x64 instructions.

Graphics backends have their own compatibility and release requirements. The preview does not install or manage DXMT, DXVK, MoltenVK, or other graphics packages.

Upstream projects: [Wine](https://github.com/wine-mirror/wine), [FEX](https://github.com/FEX-Emu/FEX), and [macOS Wine builds](https://github.com/Gcenx/macOS_Wine_builds).

## Storage

The normal root is `~/Library/Application Support/Vela`. It contains:

| Location | Purpose |
| --- | --- |
| `state.json` | Engines, active/deleted environments, shortcuts, variables, and saved scaling |
| `prefixes/<id>/` | Environment files and Windows registry |
| `logs/` | Per-operation output |
| `installers/<id>/` | Cached official launcher installer for retry |

State updates use a temporary file, synchronization, atomic rename, and a lock to serialize concurrent writers. Corrupt state or an unsupported future schema is reported instead of being overwritten with an empty library.

Engine paths and shortcut targets are absolute. Portable mode changes the library root but does not rewrite paths or migrate data.

## Initialization, scaling, and stop

Initialization runs wineboot and checks its exit status. Because Wine saves registry files asynchronously, the core then waits up to 20 seconds for `system.reg`.

Windows scaling writes the environment's `HKCU\Control Panel\Desktop\LogPixels` registry value. Existing windows need to be restarted to use the new setting. Vela does not kill them as part of a scale change.

Stopping an environment invokes its matching wineserver with its explicit prefix. An empty-output exit code of 1 is treated as an already stopped server.

## Deletion and recovery

`delete_environment` acquires a nonblocking per-environment OS file lock, invokes the matching Wine server with `-k`, and waits for it with `-w`. If stopping fails, the environment remains active. A successful operation atomically moves its record from `bottles` to the defaulted `deleted_bottles` list. Prefix, shortcut, and installer paths stay unchanged, so restoration does not rewrite application paths.

Vela Trash keeps files until `purge_environment` is separately confirmed. Purging rejects active environments, stops the selected server again, and removes only that ID's prefix, installer cache, and logs. Parent/final prefix symlinks and installer/log directory symlinks are rejected; internal Wine symlinks are not followed by recursive deletion. A failed partial purge leaves its trash record available to retry. The manager does not automatically empty its trash.

Launch and initialization share the per-environment lock so they cannot race with deletion. Locks are held by open file descriptors and released by the kernel on exit or crash. The small files in `locks/` can remain after release. State writes retain their separate atomic replacement lock.

Older Vela versions do not preserve the deleted-environment metadata when rewriting state. Avoid opening a library containing Vela Trash records with an older app version.

## Interface languages

English and Russian strings ship in `.lproj` resources. The default `system` preference matches the device's preferred supported language, falling back to English. An explicit `en` or `ru` preference is saved in UserDefaults and applies immediately to the interface. The core receives the resolved `VELA_LANGUAGE` so its generated errors and messages use the same language. User-provided names and paths are kept verbatim; Wine and Windows application output is not translated.

## Updates

The built-in launcher catalog uses publisher download endpoints and exact redirect-host allowlists. Downloads happen in Swift; the Rust core validates headers, creates the environment, starts the installer, and detects known client paths. The core stores optional installation metadata alongside each environment, preserving compatibility with older state files. See [Launcher catalog](launcher-catalog.md).

Catalog changes ship with Vela updates. Installed launchers manage their own client and game updates inside their environments.

The app, Wine source releases, macOS binary distributions, FEX, and graphics libraries have independent release cycles.

The current manager performs no engine downloads or automatic updates. Install versions side by side and test a different engine in a new environment. Updating a prefix can change its registry and installed files; returning to an older Wine binary alone is not a complete rollback.

A future update system is intended to use signed manifests, verified archives, separate engine versions, and stopped-environment snapshots. These features are not implemented in the preview.

## Security boundary

An environment separates Windows configuration. It does not prevent Windows code from accessing files available to the macOS user. See [Security policy](../SECURITY.md).

## Licenses

The original manager and interface use MIT. Wine, FEX, vendored Rust crates, and Wine-derived patches retain their own licenses. See [Third-party components](../THIRD_PARTY.md).
