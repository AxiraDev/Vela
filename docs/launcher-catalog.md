# Game launcher catalog

Vela 0.3 includes ten Windows launcher installer presets. These are links and detection rules, not bundled launcher binaries or a compatibility certification.

## Installation flow

1. Choose a launcher, engine, environment name, and Windows scale.
2. Download its installer from the publisher over HTTPS, or select an official installer already downloaded.
3. Validate its size, extension, and EXE PE header or MSI compound-file header.
4. Cache a copy in `installers/<environment-id>/` under the Vela library.
5. Create and initialize a separate Wine environment, then open the interactive installer there.
6. Finish the publisher's wizard and sign in yourself. Vela does not submit agreements or account credentials.
7. While installation is pending, Vela checks known client locations every three seconds. A detected PE client receives a shortcut. The same check runs when the library is refreshed or Vela restarts.

Finding an executable does not prove that installation has finished. Wait for the wizard before launching the client. Custom install locations require a manually added shortcut. Installer process trees and exit results are not tracked, so a failed wizard remains pending until retried or resolved.

Failed environment initialization remains visible with its log. **Open installer again** reuses that environment and cached installer. Download cancellation is available before environment creation; initialization is allowed to finish once started.

## Presets and sources

The direct installer endpoints were checked on October 23, 2026 using bounded HTTP reads. EXE architecture was read from the returned PE header; MSI payload architecture was not inspected. Full application/game compatibility was not tested.

| Launcher | Publisher page | Installer requirements |
| --- | --- | --- |
| Steam | [Valve](https://store.steampowered.com/about/) | 32-bit bootstrapper; WoW64 needed |
| Epic Games Launcher | [Epic Games](https://store.epicgames.com/download) | MSI; conservatively requires WoW64 |
| GOG GALAXY | [GOG](https://www.gog.com/galaxy) | 32-bit bootstrapper; WoW64 needed |
| Battle.net | [Blizzard](https://download.battle.net/en-us/desktop) | 32-bit bootstrapper; WoW64 needed |
| EA app | [Electronic Arts](https://www.ea.com/ea-app) | 32-bit bootstrapper, 64-bit client; WoW64 needed |
| Ubisoft Connect | [Ubisoft](https://www.ubisoft.com/en-us/ubisoft-connect) | 32-bit bootstrapper; WoW64 needed |
| Rockstar Games Launcher | [Rockstar Games](https://www.rockstargames.com/launcher) | 64-bit bootstrapper; child components may need WoW64 |
| Amazon Games | [Amazon Games support](https://www.amazongames.com/en-us/support/prime-gaming) | 32-bit bootstrapper; WoW64 needed |
| itch.io | [itch.io](https://itch.io/app) | 64-bit bootstrapper; client compatibility depends on Wine |
| Wargaming Game Center | [Wargaming EU](https://eu.wargaming.net/en/wgc) | EU installer, 32-bit bootstrapper; WoW64 needed |

Riot Client has an informational card only. Games requiring Vanguard need a supported Windows installation. See [Riot's environment restriction](https://support.riotgames.com/en-us/riot/client/error-van-9100).

The curated catalog is defined in [`core/resources/launchers.json`](../core/resources/launchers.json). It does not attempt to list every launcher or region. Changing a preset requires an application update. A publisher may change an endpoint at any time; use its official site and the local-file option when needed.

## Downloads and integrity

The Swift downloader uses an ephemeral URLSession with normal system TLS validation, no shared cookies or saved credentials, a 60-second request timeout, and a 15-minute resource timeout. Initial and redirect URLs must use HTTPS, contain no embedded credentials, and match an exact host in the preset's allowlist. Nonstandard ports are rejected. The file limit is 512 MiB, enforced during download and again before installation. Temporary downloads are removed after the request.

Publisher/CDN addresses can change. Unexpected redirect hosts are rejected; Vela does not widen its allowlist automatically. No third-party mirror is used as a fallback.

Header checks reject obvious HTML/error pages and unsupported executable architectures. They do **not** verify Authenticode publisher signatures or pin a release hash: these latest-version endpoints change over time. The download source is therefore trusted through the publisher endpoint and TLS. Launcher binaries are not redistributed in Vela's app or source archive.

Client detection checks a bounded list of relative paths under that environment's `drive_c`. Canonical paths must remain inside the environment; symlinks to external files are not accepted. It does not scan the host filesystem or Wine's `Z:` drive. The itch.io shortcut uses its installed stable `itch-setup.exe --prefer-launch --appname itch` entry point so normal client updates do not leave it pointing at a removed version folder.

## Updates and storage

Launchers manage their own client/game updates inside their Wine environments. Vela does not intercept that traffic or replace launcher files. Cached installers remain available for retry and consume library storage. Wine engine updates are separate and remain manual.

A Wine environment is not a security sandbox. Windows installers have the Mac user's file access, as described in the [security policy](../SECURITY.md).

## Validation commands

~~~sh
cargo test --locked --offline --manifest-path core/Cargo.toml
swiftc -parse-as-library -swift-version 6 macos/LauncherDownload.swift tests/launcher_download.swift -o /tmp/vela-download-check
/tmp/vela-download-check
# Optional network check: downloads the official itch.io installer without running it.
/tmp/vela-download-check --network
~~~
