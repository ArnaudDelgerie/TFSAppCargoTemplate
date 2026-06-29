# Template changelog

Versions of the **template** (not the generated app's version, which lives in `tauri.conf.json` /
`Cargo.toml`). SemVer: `MAJOR` = *wrapper-breaking* change (major Tauri bump, Caddyfile rework…) that
requires manual intervention in already-generated projects. Each entry lists the **migration steps**,
if any.

A generated project records its origin version in `.scaffold.toml` (`template_version`) — compare it
with the latest entry below to tell whether the project is behind.

## [1.0.0] — 2026-06-30

First release of the `cargo generate` template.

- Parameterizable desktop wrapper (Tauri 2 + FrankenPHP + Symfony): `product_name`, `identifier`,
  crate/binary name derived from `project-name`.
- `with_app` toggle (greenfield with base app + demo / brownfield bare wrapper).
- `.scaffold.toml` manifest (provenance + answers).
- FrankenPHP sidecar shipped as a bundled resource under `/usr/lib/<ProductName>/resources/`
  (not `/usr/bin/frankenphp`), so two TFS apps can coexist on the same machine without a dpkg
  file conflict.

Migration: —
