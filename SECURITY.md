# Security Policy

## Supported versions

Security fixes are accepted against the latest `main` branch and the newest released
semver version once releases begin.

## Reporting a vulnerability

Please report security issues privately. Do **not** open a public GitHub issue for
vulnerabilities that could affect end users (installer, profile hooks, plugin trust,
update integrity).

Until a dedicated security email/alias is published, contact the maintainers via a
private GitHub Security Advisory on the repository.

We aim to acknowledge reports within 72 hours and follow a 90-day coordinated
disclosure norm.

## Supply chain

- Release artifacts will be checksummed and signed.
- Plugin registry packages require signatures before leaving experimental status.
- `cargo deny` runs in CI for licenses and advisories.
