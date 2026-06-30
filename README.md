# TFSAppCargoTemplate

Template [`cargo generate`](https://cargo-generate.github.io/cargo-generate/) qui pose, en une
commande, tout le wrapper desktop autour d'une application Symfony : une **Tauri FrankenPHP Symfony
application** (« TFS app »). Tauri ouvre une WebView vers un backend Symfony local servi par
FrankenPHP ; en dev ce backend tourne dans Docker, en paquet il devient un sidecar FrankenPHP
embarqué (migrations, worker Messenger supervisé, hub Mercure, le tout sur loopback).

Le contrat précis entre le launcher Tauri et l'app Symfony est décrit dans [CONTRACT.md](CONTRACT.md).
Ce README est la **porte d'entrée du template** (comment générer) ; il n'est pas copié dans les
projets générés. Le doc **orienté app produite** (développer / builder / packager) est
[TFSAPP_README.md](TFSAPP_README.md) : il est posé dans le projet généré sous ce nom, à charge pour
toi de le renommer en `README.md` ou d'en reprendre le contenu dans le tien.

## Ce que pose le template

```text
app/              Application Symfony 8.1 de base + démo (optionnelle, voir with_app)
desktop/          Application Tauri 2 (code Rust, config, bundle)
build/            Caddyfile desktop + scripts de build (Symfony, sidecar, ressources)
docker/           Images FrankenPHP et Node pour le développement
compose.yaml      Services app, worker, node
Makefile          Façade de commandes
.scaffold.toml    Provenance (version du template + réponses), pour le versionnage
```

Stack : Tauri 2 · Rust · FrankenPHP · Symfony 8.1 · Doctrine ORM/DBAL/Migrations (SQLite) ·
Messenger (transport Doctrine) · Mercure (SSE) · Twig · Webpack Encore · Stimulus · Turbo · Docker
Compose en dev.

## Générer un projet

Prérequis : `cargo install cargo-generate` (la toolchain Rust + Tauri est de toute façon requise pour
builder).

```bash
# Mode interactif (recommandé) : cargo generate pose toutes les questions
cargo generate --git <url-de-ce-repo>

# Projet existant en une ligne : wrapper seul, sans app/
cargo generate --git <url-de-ce-repo> -d with_app=false
```

Le générateur demande **interactivement** :

| Réponse | Rôle |
|---|---|
| `product_name` | `productName` Tauri — pilote le nom du `.deb`, `/usr/lib/<ProductName>/`, le titre de fenêtre |
| `identifier` | reverse-domain — pilote l'app-data `~/.local/share/<identifier>`. ⚠️ immuable après publication |
| `with_app` | inclure l'app Symfony de base + démo ? (`true` = nouveau projet, `false` = projet existant) |
| `with_async` | activer le traitement async (worker Messenger) ? (`true` par défaut) |

Le nom du projet (`project-name`) sert de nom de binaire/crate. Chaque réponse peut aussi être passée
en ligne via `-d clé=valeur` pour scripter. Tout le reste — chemins, app-data, titre de fenêtre — en
**dérive automatiquement** (le code Rust est découplé du nom).

| Cas | Commande | Produit |
|---|---|---|
| **Nouveau projet** (défaut) | `cargo generate …` | wrapper + app de base + démo → **boote direct** ; remplace ensuite le contenu de `app/` par ton métier |
| **Projet existant** | `… -d with_app=false` | wrapper nu (pas de `app/`) → voir [Projet existant](#projet-existant-with_appfalse) |

Seuls `tauri.conf.json`, `Cargo.toml`, `composer.json`, `main.rs` (la const `ASYNC_ENABLED`) et
`.scaffold.toml` reçoivent des substitutions ; tout le reste est copié verbatim (notamment les
templates Twig, dont la syntaxe `{{ }}` est volontairement préservée).

Après génération, pour personnaliser l'icône : remplacer `desktop/src-tauri/icons/icon.png`
(**PNG RGBA** obligatoire) puis `cargo tauri icon <source.png>`.

### Projet existant (`with_app=false`)

Sans l'app de base, le wrapper attend une app Symfony **conforme au contrat**
([CONTRACT.md](CONTRACT.md), checklist §11). Le minimum à câbler dans ton `app/` :

- **`Kernel`** surchargeant `getCacheDir()` / `getBuildDir()` / `getLogDir()` sur `APP_CACHE_DIR` /
  `APP_BUILD_DIR` / `APP_LOG_DIR` (ne jamais écrire dans les sources au runtime).
- Endpoint **`GET /healthz`** → `200 {"status":"ok"}`.
- Config lue **depuis l'environnement** (`DATABASE_URL`, `MESSENGER_TRANSPORT_DSN`, `MERCURE_*`,
  `APP_*`), rien en dur.
- **`doctrine.yaml` / `mercure.yaml` / `messenger.yaml`** + le `SqlitePragmasMiddleware`
  (`busy_timeout`, `journal_mode=WAL`, `synchronous=NORMAL`).
- Transport Messenger lu depuis `MESSENGER_TRANSPORT_DSN` : **Doctrine** (partagé entre process) si tu
  veux de l'async, **`sync://`** si tu assumes un build sans worker (voir `with_async`).
- Schéma géré par **Doctrine Migrations**, appliqué au lancement.
- Assets compilés en prod (`public/build/entrypoints.json` présent dans le bundle).
- Un **Caddyfile desktop** conforme à [CONTRACT.md §7](CONTRACT.md) et `public/` à l'emplacement attendu.

Le plus simple : générer **une fois** en mode `with_app=true`, lire l'`app/` de base comme référence
vivante de chaque point ci-dessus, puis transposer dans ton app.

## Maintenance du template

- **Smoke test** : `make smoke` (ou `build/scripts/smoke.sh`) génère plusieurs variantes
  (greenfield, async off, brownfield) dans un dossier temporaire, `cargo check` chacune et valide le
  Caddyfile. Le repo template ne `cargo check` pas à sa racine (`Cargo.toml`/`main.rs` portent des
  placeholders Liquid) : le smoke est **la** surface de vérification. Le lancer après tout bump
  Tauri/Rust/Symfony.
- **Versionnage** : SemVer dans [CHANGELOG.md](CHANGELOG.md). Un bump `MAJOR` = changement
  *wrapper-breaking* ; documenter les étapes de migration dans le changelog. Un projet généré compare
  son `.scaffold.toml` (`template_version`) à ce changelog pour savoir s'il est en retard.

Voir [Plan/SCAFFOLDER.md](Plan/SCAFFOLDER.md) pour les décisions de conception et la feuille de route.
