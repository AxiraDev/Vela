# Security policy

## Scope

Vela is an early preview. Security fixes are considered for the current preview; older builds do not have a separate maintenance or backport commitment.

Vela starts Windows programs through an external Wine engine. A Wine environment separates configuration and installed files, but is not a security sandbox. Windows applications run with the permissions of the macOS user.

The manager does not automatically download engines. The release's app signature is local ad hoc signing; Developer ID signing and notarization are not configured.

## Reporting a vulnerability

If this repository has GitHub private vulnerability reporting enabled, use **Report a vulnerability** in its security section.

If that option is unavailable, open a public issue asking for a private security contact. Include no exploit, credentials, private files, or details that expose affected users while arranging contact.

A useful private report includes the affected version, a description of the impact, reproduction steps, and a minimal proof of concept when appropriate. Redact personal data.

The project does not promise a response deadline or offer a bug bounty.

For the reporting process, see [GitHub's private reporting documentation](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately).

## External components

Wine, FEX, and other dependencies have their own upstream maintainers and release processes. If a finding belongs to an upstream component, identify the exact revision and coordinate with the appropriate upstream project as well.

Vela's MIT license does not change third-party license or security obligations.
