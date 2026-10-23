# GitHub publication kit

The copy below is ready to use for the current preview. Repository creation, uploads, and publication are separate steps for the maintainer.

## Repository name

`AxiraDev/Vela` — https://github.com/AxiraDev/Vela

## Repository description

Free, open-source Wine manager for macOS and Apple Silicon. Install game launchers, run Windows apps, and manage Wine prefixes with a native SwiftUI interface and Rust core.

## README title

Vela — Wine Manager for macOS

## Suggested topics

~~~text
wine
macos
apple-silicon
windows-apps
wine-manager
wine-prefix
swiftui
rust
open-source
~~~

These topics describe the current project. GitHub topics help users discover related repositories; they do not guarantee a position in GitHub or web search.

Use the clear description and the actual setup instructions in the README rather than repeating keywords. Google recommends concise, descriptive titles and avoiding keyword stuffing. Search visibility also depends on the content and references to the page.

References: [GitHub topics](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/classifying-your-repository-with-topics) and [Google title guidance](https://developers.google.com/search/docs/appearance/title-link).

## Repository contents

Upload the contents of the project directory, so README.md, core/, macos/, and .github/ are at the repository root. Do not upload the surrounding workspace.

The prepared files include the English README, contributor guide, security policy, changelog, technical docs, issue forms, pull request template, and a Boosty funding configuration. The Russian README and earlier Russian technical notes are retained as additional documentation.

The application interface supports English and Russian, with the device language selected by default.

## Create the initial Git history

From the project root:

~~~sh
git init -b main
git add .cargo .github .gitattributes .gitignore LICENSE README.md README.ru.md THIRD_PARTY.md CONTRIBUTING.md SECURITY.md CHANGELOG.md Portable.example core docs macos runtime scripts tests vendor
git diff --cached --stat
git commit -m "Prepare Vela 0.4.0 preview"
~~~

Review the staged files before committing. Generated apps, engines, libraries, and build output are excluded by .gitignore.

The repository URL is:

~~~sh
git remote add origin https://github.com/AxiraDev/Vela.git
git push -u origin main
~~~

For an existing clone, keep its origin and history rather than initializing a replacement repository.

## Repository settings

Set the description and topics above. Keep issue reporting enabled. Enable GitHub private vulnerability reporting if you want the private reporting option described in SECURITY.md.

The prepared `.github/FUNDING.yml` points to the author's Boosty page. GitHub supports custom funding URLs in this file; see [GitHub's funding documentation](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/displaying-a-sponsor-button-in-your-repository).

No CI or automated release workflow is included. The preview's validation is recorded in docs/validation.md.

## Build release assets

From the project root:

~~~sh
cargo test --locked --offline --manifest-path core/Cargo.toml
bash scripts/build.sh
cp build/Vela-preview.zip build/Vela-0.4.0-preview.zip
python3 scripts/package_source.py --out build/Vela-0.4.0-source.zip
cd build
shasum -a 256 Vela-0.4.0-preview.zip Vela-0.4.0-source.zip > SHA256SUMS
cd ..
~~~

Build requirements are in README.md. The app is locally signed; this is not a notarized distribution pipeline.

For the prepared release, use:

| Field | Value |
| --- | --- |
| Tag | `v0.4.0-preview.1` |
| Title | `Vela 0.4.0 Preview` |
| Status | Pre-release |
| App version / build | `0.4.0 / 6` |
| Release body | Contents of `docs/releases/v0.4.0-preview.1.md` |

Create the tag against the commit whose source produced the uploaded app. Attach the app ZIP, the prepared source ZIP, and SHA256SUMS.

The explicit source ZIP includes vendored Rust dependencies. Wine engines and user data are excluded. Keep the preview marked as a pre-release and retain the limitations in its description.

## Short announcement

Vela is an open-source Wine manager for macOS, built with SwiftUI and Rust. The preview includes a game launcher catalog with official downloads, automatic environment creation, and shortcut detection, alongside manual Wine engine and environment controls.

The preview is available for Apple Silicon. A compatible Wine engine is required separately, and the UI supports English and Russian. Feedback and contributions are welcome.

## Support copy

Vela is free to use. If it helps you, consider supporting its development on Boosty. Your contribution helps fund maintenance, compatibility improvements, and future releases.

https://boosty.to/axira_dev/donate
