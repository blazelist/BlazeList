<div align="center">

<img src="assets/bl-icon.png" width="120">

# BlazeList
**Blazingly fast sorted list of Markdown cards. 🔥**

[![project status: alpha](https://img.shields.io/badge/project%20status-alpha-orange)](https://github.com/blazelist/BlazeList)

[![blazelist-server](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fblazelist%2FBlazeList%2Fmain%2Fserver%2FCargo.toml&query=package.version&prefix=v&label=blazelist-server)](https://github.com/blazelist/BlazeList/tree/main/server)
[![blazelist-protocol](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fblazelist%2FBlazeList%2Fmain%2Fprotocol%2FCargo.toml&query=package.version&prefix=v&label=blazelist-protocol)](https://github.com/blazelist/BlazeList/tree/main/protocol)
[![blazelist-client-lib](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fblazelist%2FBlazeList%2Fmain%2Fclients%2Flib%2FCargo.toml&query=package.version&prefix=v&label=blazelist-client-lib)](https://github.com/blazelist/BlazeList/tree/main/clients/lib)
[![blazelist-wasm](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fblazelist%2FBlazeList%2Fmain%2Fclients%2Fwasm%2FCargo.toml&query=package.version&prefix=v&label=blazelist-wasm)](https://github.com/blazelist/BlazeList/tree/main/clients/wasm)
[![blazelist-dev-seeder](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fblazelist%2FBlazeList%2Fmain%2Fclients%2Fdev-seeder%2FCargo.toml&query=package.version&prefix=v&label=blazelist-dev-seeder)](https://github.com/blazelist/BlazeList/tree/main/clients/dev-seeder)

A TODO list of sorts—one list to rule them all: centered around a mono-list called the Blaze List.

Designed to scale to thousands of cards without introducing noticeable latency or lag.

</div>

<p align="center">
  <img src="screenshots/card_list.png" width="38%">&nbsp;&nbsp;&nbsp;&nbsp;<img src="screenshots/edit_card.png" width="38%">
</p>

---

> [!WARNING]
> **This project is in alpha — breaking changes are expected.**
>
> - Not recommended for production use with data you care about unless you have proper precautions and backups in place.
> - No network security or credential management is implemented — you are responsible for securing your deployment.
> - Initial iterations rely heavily on vibe-coded code with little review — code quality standards will be raised as the architecture stabilizes.

## Quick Start (Docker)

```bash
docker compose up
# Web UI at https://localhost:47800
```

The container can run as any UID/GID — see [DOCS.md](DOCS.md) for details.

## Documentation

| Document | Description |
|---|---|
| **[DOCS.md](DOCS.md)** | User guide — deployment, configuration, environment variables |
| **[DEV.md](DEV.md)** | Developer guide — local development workflow, building, testing |

## Release Signing

Release commits are signed with the following PGP key:

- **Fingerprint:** `9EA6 C866 165A 3A86 08BE  3568 EA2D C27E 87A4 94F2`
- **Public key:** [`release-signing-key.asc`](release-signing-key.asc)

## Licensing

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
