# Roadmap

These are intended directions, not shipping features or delivery commitments.

## Reliable environment management

- Track application startup, process trees, and exit results.
- Add stopped-environment snapshots and restore.
- Add safe environment deletion with clear data handling.
- Edit existing shortcuts and discover installed applications.
- Rotate logs and improve recovery after interrupted initialization or a stale lock.
- Improve library relocation and portable-mode path handling.

## Engine delivery

- Establish a reproducible build pipeline for supported native runtimes.
- Publish exact source inputs, patches, component notices, and build metadata.
- Verify signed release manifests and downloaded archives.
- Keep engine versions side by side.
- Validate engine changes against snapshots before offering rollback.
- Expand the macOS and hardware validation matrix.

## Application setup

- Expand the bundled launcher presets into richer, versioned setup recipes and compatibility reports.
- Evaluate optional Winetricks integration.
- Evaluate compatible graphics backends and dependency installation.
- Detect available graphics capabilities before exposing configuration controls.
- Improve EXE discovery and Windows shortcut handling.

## Interface and release quality

- Add English interface localization.
- Prepare Developer ID signing and Apple notarization.
- Improve diagnostics, accessibility, and onboarding.
- Add ongoing automated checks and reproducible release packaging.

Support development through [Boosty](https://boosty.to/axira_dev/donate). Contributions are voluntary and do not guarantee a particular feature or timeline.
