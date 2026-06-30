# TFSAppCargoTemplate

A [`cargo generate`](https://cargo-generate.github.io/cargo-generate/) template that scaffolds, in a
single command, the whole desktop wrapper around a Symfony application: a **Tauri FrankenPHP Symfony
application** ("TFS app"). Tauri opens a WebView onto a local Symfony backend served by FrankenPHP;
in dev that backend runs in Docker, in the packaged app it becomes an embedded FrankenPHP sidecar
(migrations, supervised Messenger worker, Mercure hub — all over loopback).

The precise contract between the Tauri launcher and the Symfony app is described in
[CONTRACT.md](CONTRACT.md). This README is the **template's entry point** (how to generate); it is
not copied into generated projects. The **generated-app-oriented** doc (develop / build / package) is
[TFSAPP_README.md](TFSAPP_README.md): it is dropped into the generated project under that name — rename
it to `README.md` or fold its content into yours.

## What the template scaffolds

```text
app/              Base Symfony 8.1 app + demo (optional, see with_app)
desktop/          Tauri 2 application (Rust code, config, bundle)
build/            Desktop Caddyfile + build scripts (Symfony, sidecar, resources)
docker/           FrankenPHP and Node images for development
compose.yaml      app, worker, node services
Makefile          Command facade
.scaffold.toml    Provenance (template version + answers), for versioning
```

Stack: Tauri 2 · Rust · FrankenPHP · Symfony 8.1 · Doctrine ORM/DBAL/Migrations (SQLite) ·
Messenger (Doctrine transport) · Mercure (SSE) · Twig · Webpack Encore · vanilla JS ·
symfony/translation (EN/FR) · Docker Compose in dev.

## Generate a project

Prerequisite: `cargo install cargo-generate` (the Rust + Tauri toolchain is required to build anyway).

```bash
# Interactive mode (recommended): cargo generate asks every question
cargo generate --git <this-repo-url>

# Existing project, one-liner: wrapper only, no app/
cargo generate --git <this-repo-url> -d with_app=false
```

The generator asks **interactively**:

| Answer | Role |
|---|---|
| `product_name` | Tauri `productName` — drives the `.deb` name, `/usr/lib/<ProductName>/`, app-data `~/.local/share/<ProductName>`, the window title. ⚠️ immutable after release |
| `identifier` | reverse-domain Tauri identifier — stable technical app identity |
| `with_app` | include the base Symfony app + demo? (`true` = new project, `false` = existing project) |
| `with_async` | enable async processing (Messenger worker)? (`true` by default) |

The project name (`project-name`) is used as the binary/crate name. Each answer can also be passed on
the command line via `-d key=value` for scripting. Everything else — paths, app-data, window title —
**derives automatically** (the Rust code is decoupled from the name).

| Case | Command | Result |
|---|---|---|
| **New project** (default) | `cargo generate …` | wrapper + base app + demo → **boots straight away**; then replace the contents of `app/` with your own |
| **Existing project** | `… -d with_app=false` | bare wrapper (no `app/`) → see [Existing project](#existing-project-with_appfalse) |

Only `tauri.conf.json`, `Cargo.toml`, `main.rs` (the `ASYNC_ENABLED` const) and
`.scaffold.toml` receive substitutions; everything else is copied verbatim (notably the Twig
templates, whose `{{ }}` syntax is deliberately preserved).

After generation, to customize the icon: replace `desktop/src-tauri/icons/icon.png` (**RGBA PNG**
required) then run `cargo tauri icon <source.png>`.

### Existing project (`with_app=false`)

Without the base app, the wrapper expects a Symfony app that **conforms to the contract**
([CONTRACT.md](CONTRACT.md), checklist §11). The minimum to wire up in your `app/`:

- A **`Kernel`** overriding `getCacheDir()` / `getBuildDir()` / `getLogDir()` to use `APP_CACHE_DIR` /
  `APP_BUILD_DIR` / `APP_LOG_DIR` (never write into the sources at runtime).
- A **`GET /healthz`** endpoint → `200 {"status":"ok"}`.
- Config read **from the environment** (`DATABASE_URL`, `MESSENGER_TRANSPORT_DSN`, `MERCURE_*`,
  `APP_*`), nothing hardcoded.
- **`doctrine.yaml` / `mercure.yaml` / `messenger.yaml`** plus the `SqlitePragmasMiddleware`
  (`busy_timeout`, `journal_mode=WAL`, `synchronous=NORMAL`).
- A Messenger transport read from `MESSENGER_TRANSPORT_DSN`: **Doctrine** (shared across processes) if
  you want async, **`sync://`** if you accept a worker-less build (see `with_async`).
- Schema managed by **Doctrine Migrations**, applied at launch.
- Assets compiled for prod (`public/build/entrypoints.json` present in the bundle).
- A **desktop Caddyfile** conforming to [CONTRACT.md §7](CONTRACT.md) and `public/` at the expected location.

Easiest path: generate **once** with `with_app=true`, read the base `app/` as a living reference for
each point above, then transpose into your app.

## Maintaining the template

- **Smoke test**: `make smoke` (or `build/scripts/smoke.sh`) generates several variants (greenfield,
  async off, brownfield) in a temp dir, `cargo check`s each, and validates the Caddyfile. The template
  repo does not `cargo check` at its root (`Cargo.toml`/`main.rs` carry Liquid placeholders): the smoke
  test is **the** verification surface. Run it after any Tauri/Rust/Symfony bump.
- **Versioning**: SemVer in [CHANGELOG.md](CHANGELOG.md). A `MAJOR` bump = a *wrapper-breaking* change;
  document the migration steps in the changelog. A generated project compares its `.scaffold.toml`
  (`template_version`) against this changelog to know whether it is behind.

See [Plan/SCAFFOLDER.md](Plan/SCAFFOLDER.md) for design decisions and the roadmap.
