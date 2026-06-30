# &lt;ProductName&gt;

> 📄 **This file is the reference README for the TFS app wrapper**, dropped by the template at
> generation time. It describes how to develop, build and package the generated app. Rename it to
> `README.md` or take from it whatever you need for your own business-oriented README.

A Tauri 2 desktop application that embeds a Symfony application served locally by FrankenPHP — a
**Tauri FrankenPHP Symfony application** ("TFS app"), generated from the `TFSAppCargoTemplate`
template.

The principle is deliberately simple: Tauri contains almost no business logic. It opens a WebView onto
a local HTTP backend. In development, that backend runs in Docker. In the packaged app, Tauri itself
starts a FrankenPHP sidecar, serves the embedded Symfony app, applies migrations, starts and
supervises a Messenger worker, then displays the UI in the WebView.

This is not a throwaway PoC: it is a **starting base for real local apps**. The precise contract
between the launcher and the app is described in [CONTRACT.md](CONTRACT.md).

> This app's identity: `productName` (`<ProductName>`), `identifier` (`<identifier>`) and the binary
> (`<project-name>`) are defined in `desktop/src-tauri/tauri.conf.json` and
> `desktop/src-tauri/Cargo.toml`. These values drive the `.deb` name
> (`<ProductName>_<version>_amd64.deb`), the `/usr/lib/<ProductName>/` path and the app-data
> `~/.local/share/<identifier>`. Everything else **derives automatically** (the Rust code is decoupled
> from the name). ⚠️ **Never change the `identifier` after release** → loss of user app-data (see
> [Adapting this app](#adapting-this-app) and [CONTRACT.md §10](CONTRACT.md)).

## Stack

- Tauri 2
- Rust
- FrankenPHP
- Symfony 8.1
- Doctrine ORM + DBAL + Migrations (SQLite)
- Messenger with the Doctrine DBAL SQLite transport
- Mercure for SSE events
- Twig
- Webpack Encore
- vanilla JS (no frontend framework imposed)
- symfony/translation (EN/FR)
- Docker Compose for development

## Concept

```text
Development

Browser or Tauri dev
  -> http://127.0.0.1:${APP_PORT:-8080}
  -> FrankenPHP container
  -> Symfony
  -> Messenger worker container
  -> Mercure
  -> EventSource in the frontend
```

```text
Packaged application

Tauri
  -> picks a free local port
  -> generates infra secrets (persistent APP_SECRET, ephemeral Mercure secret)
  -> starts the FrankenPHP sidecar
  -> serves the embedded Symfony app
  -> applies migrations (blocking)
  -> starts + supervises a Messenger worker sidecar
  -> opens the WebView on http://127.0.0.1:<port>
  -> stops the sidecars on close
```

The final application does not need Docker, PHP, Composer, Node or the Symfony CLI on the user's
machine. The PHP runtime is provided by the embedded FrankenPHP binary. **No secret has to be
supplied**: they are generated at launch.

## Why this architecture?

It lets you reuse an almost-standard Symfony web application inside a native desktop shell.

Pros:

- the backend stays a real Symfony HTTP backend;
- business logic is not duplicated between web and desktop;
- Doctrine, Messenger, Twig, Mercure and the Symfony components stay usable as normal;
- Tauri brings desktop packaging, the WebView and native integrations if needed;
- FrankenPHP provides a self-contained PHP runtime, quick to start and simple to embed;
- development stays reproducible thanks to Docker Compose;
- the end user installs a desktop package, not a server stack.

Cons:

- the packaged application starts several local processes;
- the sidecar lifecycle must be handled cleanly;
- local ports, environment variables and data paths must be kept under control;
- debugging spans Rust, Tauri, FrankenPHP, Symfony and Messenger;
- SQLite is convenient locally, but must be configured correctly for multi-process use.

## Included demo

> This section describes the base app shipped by the template. If you have replaced the contents of
> `app/` with your own domain, it no longer applies as-is — but the flow below stays the reference for
> the HTTP → async → SSE wiring.

The page is a **pedagogical showcase**: a hero, a "Try it" block (button + 3 captioned counters),
three didactic cards ("async is optional", "when do I need SSE?", "where does my data go?") and a
roadmap card. It is **bilingual EN/FR** (toggle in the top-right, via `symfony/translation` + a
`LocaleSubscriber` on `?_locale=`), and its frontend is **vanilla JS** bundled by webpack — **no
frontend framework imposed** (wire up Stimulus, Turbo, React… as you like). The page **adapts to the
build**: with async on it advertises the worker + SSE; with async off it switches to "synchronous
task" and greys out the SSE counter.

On click, on an **async-on** build:

1. The (vanilla) JS sends `POST /api/dispatch`.
2. Symfony creates a `jobId`, **persists a `DemoJob` (`pending`) in the database**, then dispatches `DemoPingMessage`.
3. Symfony responds immediately in JSON; the HTTP and `Persisted jobs` counters increase (live DB, no reload).
4. The Messenger worker consumes the message from SQLite.
5. The handler **moves the `DemoJob` to `done`** (a DB write from the worker process) and publishes a
   Mercure event on the `app://demo` topic.
6. The frontend's EventSource receives the event, the SSE counter increases.

```text
WebView -> Symfony HTTP (persist) -> Messenger async -> worker (update DB) -> Mercure -> SSE -> WebView
```

On an **async-off** build, the transport is `sync://`: the **same handler** runs **inline in the
request** (the `DemoJob` is marked `done` before the response), nothing is queued, and there is no SSE
callback. The button and the copy reflect this. See
[Enabling / disabling async](#enabling--disabling-async).

## Repository structure

```text
app/                         Symfony 8.1 application
app/assets/                  vanilla JS, CSS, Encore entry
app/src/Entity/              Doctrine entities (DemoJob)
app/src/Doctrine/            SQLite PRAGMA middleware
app/migrations/              Doctrine migrations
app/src/                     Controllers, messages, handlers
app/templates/               Twig templates
build/Caddyfile.desktop      Caddyfile used by the desktop package
build/scripts/               Symfony, sidecar and Tauri resource build scripts
desktop/                     Tauri 2 application
desktop/src-tauri/           Rust code, Tauri config, bundle
docker/frankenphp/           Development FrankenPHP image
docker/node/                 Node image for Encore
compose.yaml                 app, worker and node services
Makefile                     Command facade
```

## Development prerequisites

On the dev machine:

- Docker and Docker Compose;
- Rust and Cargo;
- the Tauri CLI;
- your OS's Tauri system dependencies;
- `curl`, `make`, `dpkg` on Linux if you build the `.deb` package.

PHP, Composer, Node and FrankenPHP are provided by Docker for development. The packaged app embeds its
own FrankenPHP.

## Dev setup

```bash
make composer-install   # PHP dependencies
make assets-install     # JS dependencies
make db                 # Messenger tables + Doctrine migrations
make assets-build       # asset compilation
make dev                # Symfony + worker
```

The application is available at `http://127.0.0.1:8080`. If port 8080 is already taken:

```bash
APP_PORT=18080 make dev
```

## Tauri development

```bash
make tauri-dev
```

In this mode, Tauri opens `http://127.0.0.1:8080`. The backend stays the Docker one: useful for working
on the desktop integration without rebuilding a package.

## Desktop build

```bash
make tauri-build
```

This command:

1. installs the Symfony dependencies without `require-dev`;
2. compiles the Encore assets for production;
3. clears and warms up the Symfony prod cache;
4. copies the Symfony application into `desktop/src-tauri/resources/app`;
5. downloads or reuses the FrankenPHP sidecar;
6. copies the desktop Caddyfile;
7. runs `cargo tauri build`.

The Linux `.deb` package is generated here:

```text
desktop/src-tauri/target/release/bundle/deb/<ProductName>_<version>_amd64.deb
```

To avoid `_apt` warnings when the package is in a user directory:

```bash
cp desktop/src-tauri/target/release/bundle/deb/<ProductName>_<version>_amd64.deb /tmp/
sudo apt install --reinstall /tmp/<ProductName>_<version>_amd64.deb
<project-name>
```

## Enabling / disabling async

Asynchronous processing (the Messenger worker) is driven by a **single Rust constant**, set at
generation time from the `with_async` answer:

```rust
// desktop/src-tauri/src/main.rs
const ASYNC_ENABLED: bool = true; // or false
```

This constant drives two things in the packaged app: the **worker spawn** and the **injected Messenger
transport**.

- `true`: `doctrine://…` transport. The app starts and **supervises** the Messenger worker; jobs are
  consumed in the background and the SSE callback arrives (as the demo illustrates).
- `false`: `sync://` transport, **no worker** (~60 MB saved). Dispatched messages are handled **inline
  in the request** by the same handler — **nothing is queued**. Important consequence: a build that
  later switches back to async **finds no backlog** to drain (no risk of retroactive spam). The
  Messenger bundle, the handler and the Mercure hub stay installed, **inert at rest**.

To switch later: set `ASYNC_ENABLED` to the other value in `desktop/src-tauri/src/main.rs`, then
**rebuild** (`make tauri-build`).

> **In dev (compose), it's a file, not the constant.** Dev mode honours the same choice via
> `compose.override.yaml` (`doctrine://` transport + a `worker` service), merged automatically by
> `docker compose` on top of `compose.yaml` (the `sync://` base, no worker). The scaffolder **removes**
> that override for `with_async=false` builds: `make dev` then starts only `app`. To toggle dev by
> hand, add/remove `compose.override.yaml`.

> Why a constant and not an environment variable: the desktop app is launched by a click, with no
> shell or exported env — a "hardcoded" default is needed anyway. A build-baked const is simpler and
> trap-free compared to a runtime override system.

## FrankenPHP mode: classic (default) vs worker

By default, FrankenPHP serves Symfony in **classic mode**: one kernel boot per request (like php-fpm).
This is driven by `Caddyfile.desktop` (`php_server` without a `worker` directive).

Why classic by default for a desktop app:

- **Single-user on loopback**: a per-request boot costs a few ms (opcache on) — **imperceptible**.
  Measured on the base app: median **2.7 ms** (worker mode) vs **7.6 ms** (classic). Under ~100 ms,
  the user sees no difference.
- **~50 MB less resident RAM** on the server process (worker mode keeps the Symfony kernel warm in
  memory between requests). Measured: **127 MB PSS** (worker) → **75 MB PSS** (classic).
- **No worker-state footgun**: in worker mode, code that isn't "worker-safe" (statics, singletons) can
  leak state between requests. In classic, a fresh kernel every time → foolproof.

**Enable worker mode (opt-in)** if your app is chatty (many XHR requests) or perf-critical: in
`build/Caddyfile.desktop`, replace `php_server` with

```caddyfile
php_server {
    worker {$APP_PUBLIC_DIR}/index.php
}
```

then **rebuild** (`make tauri-build`). You regain ~5 ms of latency per request at the cost of ~50 MB of
RAM. (The dev `Caddyfile` — `docker/frankenphp/Caddyfile` — carries the same setting, to align if
needed.)

## Footprint & behaviour under load

Figures measured on the base app (FrankenPHP classic mode, the default), to give an order of
magnitude. RAM is expressed in **PSS** (Proportional Set Size): the share of memory genuinely
attributable to the app once shared memory is divided up — far more honest than RSS when several
processes share the same `frankenphp` binary.

**At rest** (whole app: Tauri launcher + WebKit WebView + FrankenPHP server):

| Build | Total PSS | Idle CPU | Processes |
|-------|-----------|----------|-----------|
| Sync  | ~316 MB   | 0 %      | 4         |
| Async | ~338 MB   | 0 %      | 5         |

Async adds the Messenger worker. Its **real** overhead is about **+22 MB PSS**, not +66: the worker
shares the `frankenphp` binary with the server, so the server's PSS drops by the same amount when the
worker starts. The installed disk size is dominated by the `frankenphp` binary (~163 MB); enabling or
disabling async changes nothing there.

**Under load** (server only, 30 concurrent connections, no think time). The handler simulates a
**700 ms** job:

| Scenario          | Sync          | Async         |
|-------------------|---------------|---------------|
| `GET /` (page)    | ~1000 rps     | ~980 rps      |
| job dispatch      | **~22 rps**   | **~800 rps**  |
| dispatch p50      | ~1420 ms      | ~32 ms        |
| dispatch p99      | ~1460 ms      | ~82 ms        |

Reading: the same 700 ms of work **blocks the request in sync** (the client absorbs the delay and
everything serializes) but **goes to the background in async** — the controller responds in ~9 ms and
the result arrives later via SSE. Hence ~36× more dispatches/s in async. Routes that don't do this
work (`GET /`, 422 validation) are identical between the two: the difference comes purely from the
execution model, not from the rest of the stack.

Practical conclusion: **disabling async lightens the at-rest footprint a little** (one fewer process,
~22 MB); **enabling it pays off under load** as soon as there is work to push out of the request. For a
single-user desktop app, sync is often enough — hence the configurable default at `cargo generate`.

The measurement scripts (`measure.sh` at rest, `loadtest.py` under load) are not embedded in the
template; they live alongside it to reproduce these figures on your own app.

## Desktop lifecycle

In production, the Rust code:

- resolves the embedded resources path;
- picks a free local port;
- generates infra secrets (persistent `APP_SECRET` `0600`, ephemeral Mercure secret);
- configures the Symfony, Messenger and Mercure environment variables;
- starts FrankenPHP;
- runs `messenger:setup-transports` then `doctrine:migrations:migrate` (blocking steps);
- starts `messenger:consume async` and **supervises it** (auto-restart on every exit), if async is enabled;
- waits for `/healthz` to answer;
- opens the WebView (title = `productName`);
- stops the worker and the server when the window closes;
- cleans up, at startup, any orphan sidecars of this app (crash cases).

Startup cleanup matters: if the app crashes or the Tauri process is killed abruptly, an old worker
could keep consuming the SQLite queue and publishing to a stale hub. The repo records the sidecar PIDs
and identifies orphans by criteria derived from the install paths (so they are safe even if several
apps built from this base run in parallel).

## Data and runtime paths

The embedded Symfony app is installed read-only:

```text
/usr/lib/<ProductName>/resources/app
```

You must not write into this folder at runtime.

Mutable data is placed in the Tauri application data directory, e.g. on Linux:

```text
~/.local/share/<identifier>
```

There you find: the SQLite database, the Symfony runtime cache, the Symfony **build dir**, the logs,
`data/app.secret` and the sidecar PID file.

Symfony overrides `getCacheDir()`, `getBuildDir()` and `getLogDir()` to use `APP_CACHE_DIR`,
`APP_BUILD_DIR` and `APP_LOG_DIR`, so it never writes into the read-only resources. The build dir is
crucial: its Symfony default points **inside the project** (hence read-only once packaged), and Symfony
writes the compiled container, cache pools and ORM proxies there.

## Caddy and Mercure

In dev, Docker uses `docker/frankenphp/Caddyfile`. In the desktop package, Tauri embeds
`build/Caddyfile.desktop`. The desktop Caddyfile listens on a port provided by Tauri
(`http://127.0.0.1:{$APP_PORT}`).

Do not use a fixed port in desktop production: an old sidecar could stay alive and steal the traffic or
the messages. The dynamic port avoids talking to a stale server.

The Mercure transport uses `transport local`: the hub lives in the server process. `transport_url` and
the `transport bolt { ... }` syntax are rejected by some runtime versions.

The Mercure secrets are **consolidated into a single variable**: `MERCURE_JWT_SECRET` is used both by
`symfony/mercure-bundle` (signing) and by the Caddy `mercure` directive (validating `publisher_jwt`
AND `subscriber_jwt`). In prod, the launcher generates it randomly on each launch (ephemeral, internal
loopback hub). Mind the length: `lcobucci/jwt` requires an HS256 key of at least 256 bits (32 bytes) —
the generated values are 64 hex characters.

## SQLite, Doctrine and Messenger

The Messenger transport uses Doctrine DBAL with SQLite:

```text
MESSENGER_TRANSPORT_DSN=doctrine://default?queue_name=async
```

The `default` connection and the registry are provided by DoctrineBundle; the Messenger transport
reuses them. The `messenger_messages` table is managed by `messenger:setup-transports` and **excluded
from migrations** via `schema_filter` (`~^(?!messenger_messages)~`).

The SQLite PRAGMAs essential for multi-process use are applied by a **DBAL 4 middleware**
(`App\Doctrine\SqlitePragmasMiddleware`):

```sql
PRAGMA busy_timeout = 5000;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
```

(In DBAL 4 the `postConnect` events are gone; a middleware is the supported way.)

### Migrations

The application schema is managed by **Doctrine Migrations**. The database lives in app-data and
**persists across updates**: a new version does not start from a fresh database. Any schema change must
therefore go through a migration, applied **at launch** (a blocking step of the launcher, and `make db`
in dev). `migrate` only runs migrations not yet applied → incremental upgrade v1 → v2 on an existing
install.

⚠️ **Never edit a migration that has already shipped — add a new one.** A user's DB already records the
old version as applied, so an edit would never re-run; only a new version file bridges the schema
forward.

Generate a migration after modifying an entity:

```bash
make symfony-console CMD="doctrine:migrations:diff"
make symfony-console CMD="doctrine:migrations:migrate"
```

## Updating the wrapper from the template

At generation time, a **`.scaffold.toml`** file was written at the project root: it records the
original template version (`template_version`) and the answers used. Don't edit it by hand — it serves
two purposes:

- **Knowing whether you are behind**: compare `template_version` with the latest published version of
  the `TFSAppCargoTemplate` template (its `CHANGELOG.md`). A `MAJOR` bump signals a *wrapper-breaking*
  change whose migration steps are described in the template's changelog.
- **Updating the wrapper**: since the wrapper (`desktop/`, `docker/`, `build/`, `compose.yaml`,
  `Makefile`, Caddyfiles) is orthogonal to `app/`, updating means rewriting that layer **without
  touching `app/` or the identity**. Until a dedicated script exists: `git diff` between two template
  tags on those paths, applied by hand.

⚠️ **Never** change the `identifier` after release (loss of user app-data — see
[CONTRACT.md §10](CONTRACT.md)).

## Mistakes to avoid

- **A fixed port for the desktop backend.** A fixed port can stay occupied by an old FrankenPHP; the
  window would then load a stale backend / cache / hub.
- **Multiple desktop workers.** On the same SQLite queue they steal each other's messages and publish
  to different hubs. Symptom: 5 HTTP responses, 1–2 SSE.
- **Writing into the installed resources.** `/usr/lib/<ProductName>/resources/app` belongs to the
  package (read-only). Cache, build, logs, DB → app-data, via the `APP_*_DIR` variables.
- **Embedding `var/cache` or `var/build`.** An embedded cache/container may reference old Encore
  hashes. The build excludes them and the runtime wipes them at launch.
- **Assuming a fresh database on an update.** App-data persists → handle migrations.
- **Editing the Docker Caddyfile on every desktop hypothesis.** Docker mode is the witness: if it works
  outside Tauri, keep that reference and focus debugging on launcher / ports / env / sidecars.
- **Forgetting to restore dev dependencies after a build** (`composer install --working-dir=app`).
- **Assuming Tauri kills the sidecars on its own.** Lifecycle handled explicitly + cleanup on next
  startup.

## Diagnostics

List the Tauri/FrankenPHP processes:

```bash
pgrep -af '<project-name>|frankenphp'
```

Expected dev Docker processes:

```text
frankenphp run --config /etc/caddy/Caddyfile
frankenphp php-cli bin/console messenger:consume async -vvv
```

Expected desktop processes while the packaged app is open:

```text
/usr/bin/frankenphp run --config /usr/lib/<ProductName>/resources/Caddyfile.desktop
/usr/bin/frankenphp php-cli bin/console messenger:consume async --time-limit=3600 --memory-limit=256M --env=prod --no-debug
```

(In prod, no `-vvv`: logs go through the Symfony logger under `APP_LOG_DIR`. The worker only appears if
async is enabled.)

Check the Caddyfile embedded in the package:

```bash
dpkg-deb --fsys-tarfile /tmp/<ProductName>_<version>_amd64.deb \
  | tar -xO usr/lib/<ProductName>/resources/Caddyfile.desktop
```

Check the embedded Encore assets:

```bash
dpkg-deb --fsys-tarfile /tmp/<ProductName>_<version>_amd64.deb \
  | tar -xO usr/lib/<ProductName>/resources/app/public/build/entrypoints.json
```

Check the Symfony container and the Rust code:

```bash
php app/bin/console lint:container --no-debug
cargo check --manifest-path desktop/src-tauri/Cargo.toml
```

Check the desktop Caddyfile:

```bash
APP_PORT=38124 APP_ORIGIN=http://127.0.0.1:38124 APP_PUBLIC_DIR=/tmp \
MERCURE_JWT_SECRET=0123456789abcdef0123456789abcdef \
frankenphp adapt --config build/Caddyfile.desktop --validate
```

## Commands

### Makefile

```bash
make composer-install   # PHP dependencies in the container
make assets-install     # Node dependencies in the container
make assets-build       # assets for production
make assets-watch       # Encore in watch mode
make db                 # Messenger tables + Doctrine migrations
make dev                # FrankenPHP + worker in dev
make symfony-console CMD="debug:router"
make tauri-dev          # Tauri dev with the Docker backend
make build-app          # prod Symfony resources
make sidecar            # download/verify the FrankenPHP sidecar
make tauri-build        # Tauri package
```

### Docker Compose

```bash
docker compose up app worker
docker compose up -d app worker
docker compose stop
docker compose down
APP_PORT=18080 docker compose up app worker
```

### Symfony

```bash
make symfony-console CMD="debug:router"
make symfony-console CMD="debug:config framework messenger"
make symfony-console CMD="doctrine:migrations:status"
make symfony-console CMD="messenger:consume async -vvv"
make symfony-console CMD="messenger:stop-workers"
```

### Assets

```bash
npm --prefix app run build
npm --prefix app run watch
```

> Note: `app/package.json` contains an `overrides` forcing `serialize-javascript` ≥ 7.0.6 (a
> vulnerability in the webpack-encore build toolchain). Don't remove it; `npm audit` must stay at 0.

### Tauri and Cargo

```bash
. "$HOME/.cargo/env"          # load the Cargo env if needed
cd desktop && cargo tauri dev
cd desktop && cargo tauri build
cargo check --manifest-path desktop/src-tauri/Cargo.toml
```

### Package installation

```bash
cp desktop/src-tauri/target/release/bundle/deb/<ProductName>_<version>_amd64.deb /tmp/
sudo apt install --reinstall /tmp/<ProductName>_<version>_amd64.deb
<project-name>
```

## Adapting this app

The identity was set at generation time. To change it later (the Rust code is **decoupled from the
name** — window title read from config, cleanup paths derived at runtime, generic logs):

**Identity:**

- `desktop/src-tauri/tauri.conf.json`: `productName`, `version`, `identifier`.
  ⚠️ **Never change the `identifier` after release** → loss of user app-data.
- `desktop/src-tauri/Cargo.toml`: `name` (= binary name), `version` (in sync), `description`,
  `authors`.

**Recommended:** `app/composer.json` (`name`, `description`).

**Icons:** replace `desktop/src-tauri/icons/icon.png` (**RGBA PNG** required) and regenerate the full
set via `cargo tauri icon <source.png>`.

**Derived automatically (do not edit):** `.deb` name, `/usr/lib/<ProductName>/`, app-data
`~/.local/share/<identifier>`, binary, window title.

Then: replace the Symfony demo content, add authentication if the app exposes sensitive data, add
tests, and the packaging targets you need (`appimage`, `dmg`, `msi`…). The full contract is in
[CONTRACT.md](CONTRACT.md).

## Current limitations

- No authentication.
- No local database encryption.
- Native bridge (user keyring secrets, dialogs, notifications) not yet implemented.
- Packaging targets `.deb`; the sidecar script covers Linux x86_64, macOS ARM64 and x86_64.
- Windows is not configured yet.
- SQLite suits local apps; reconsider for heavy loads.

## Useful mental rule

Docker mode is the witness. If it works outside Tauri but not in the package, look first at:

1. sidecar processes left alive;
2. local ports;
3. environment variables injected by Rust;
4. read-only vs app-data paths (cache, build, log);
5. Symfony caches and Encore assets;
6. the number of active Messenger workers.
