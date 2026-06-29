# Tauri FrankenPHP Symfony application (TFS app)

Base réutilisable pour construire une application desktop Tauri 2 qui embarque une application
Symfony servie localement par FrankenPHP — la **Tauri FrankenPHP Symfony application**, alias
**TFS app**.

Le principe est volontairement simple : Tauri ne contient presque pas de logique métier. Il ouvre
une WebView vers un backend HTTP local. En développement, ce backend tourne dans Docker. En
application packagée, Tauri lance lui-même un sidecar FrankenPHP, sert l'app Symfony embarquée,
applique les migrations, lance et supervise un worker Messenger, puis affiche l'interface dans la
WebView.

Ce n'est pas un POC jetable : l'objectif est une **base de départ pour de vraies apps locales**,
duplicable sans piège. Le contrat précis entre le launcher et l'app est décrit dans
[CONTRACT.md](CONTRACT.md).

> Identité : `productName` = `TFSApp`, `identifier` = `dev.local.tfs-app`, binaire = `tfs-app`.
> Ces valeurs pilotent le nom du `.deb`, le chemin `/usr/lib/TFSApp/` et l'app-data
> `~/.local/share/dev.local.tfs-app` (voir [Adapter cette base](#adapter-cette-base-a-une-vraie-app)
> pour les renommer dans une app dérivée).

## Générer un projet depuis ce template

Ce dépôt est un template [`cargo generate`](https://cargo-generate.github.io/cargo-generate/) : il
pose tout le wrapper desktop (Tauri, Docker, `compose.yaml`, `Makefile`, Caddyfiles) en une commande.
Prérequis : `cargo install cargo-generate` (la toolchain Rust + Tauri est de toute façon requise pour
builder).

```bash
# Mode interactif (recommandé) : cargo generate pose toutes les questions
cargo generate --git <url-de-ce-repo>

# Projet existant en une ligne : wrapper seul, sans app/
cargo generate --git <url-de-ce-repo> -d with_app=false
```

Le générateur demande **interactivement** `product_name` (= `productName`), `identifier`
(reverse-domain) et **« Inclure l'app Symfony de base ? »** (menu `true`/`false`, défaut `true` =
nouveau projet ; choisir `false` pour un projet existant). Chaque question peut aussi être passée en
ligne via `-d clé=valeur` pour scripter. Le nom du projet (`project-name`) sert de nom de
binaire/crate. Tout le reste — nom du `.deb`, chemins
`/usr/lib/<ProductName>/`, app-data `~/.local/share/<identifier>`, titre de fenêtre — en **dérive
automatiquement** (le code Rust est découplé du nom). Seuls `tauri.conf.json`, `Cargo.toml` et
`composer.json` reçoivent des substitutions ; tout le reste est copié verbatim (notamment les
templates Twig, dont la syntaxe `{{ }}` est volontairement préservée).

| Cas | Commande | Produit |
|---|---|---|
| **Nouveau projet** (défaut) | `cargo generate …` | wrapper + app de base + démo → **boote direct** ; remplace ensuite le contenu de `app/` par ton métier |
| **Projet existant** | `… -d with_app=false` | wrapper nu (pas de `app/`) → voir [Projet existant](#projet-existant-with_appfalse) |

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
- Transport Messenger **partagé entre process** (Doctrine), pas `sync://`.
- Schéma géré par **Doctrine Migrations**, appliqué au lancement.
- Assets compilés en prod (`public/build/entrypoints.json` présent dans le bundle).
- Un **Caddyfile desktop** conforme à [CONTRACT.md §7](CONTRACT.md) et `public/` à l'emplacement attendu.

Le plus simple : générer **une fois** en mode `with_app=true`, lire l'`app/` de base comme
référence vivante de chaque point ci-dessus, puis transposer dans ton app.

### Versionnage et mise à jour du wrapper

À la génération, un fichier **`.scaffold.toml`** est écrit à la racine du projet : il enregistre la
version du template d'origine (`template_version`) et les réponses utilisées. Ne pas l'éditer à la
main — il sert à deux choses :

- **Savoir si tu es en retard** : comparer `template_version` avec la dernière entrée de
  [CHANGELOG.md](CHANGELOG.md) du template. Un bump `MAJOR` signale un changement *wrapper-breaking*
  dont les étapes de migration sont décrites dans le changelog.
- **Mettre à jour le wrapper** (à venir) : comme le wrapper (`desktop/`, `docker/`, `build/`,
  `compose.yaml`, `Makefile`, Caddyfiles) est orthogonal à `app/`, la mise à jour consiste à réécrire
  cette couche **sans toucher `app/` ni l'identité**. En attendant un script dédié : `git diff` entre
  deux tags du template sur ces chemins, appliqué à la main.

⚠️ Ne jamais re-scaffolder `app/`, et ne **jamais** changer l'`identifier` après publication (perte
de l'app-data utilisateur — voir [CONTRACT.md §10](CONTRACT.md)).

## Stack

- Tauri 2
- Rust
- FrankenPHP
- Symfony 8.1
- Doctrine ORM + DBAL + Migrations (SQLite)
- Messenger avec transport Doctrine DBAL SQLite
- Mercure pour les évènements SSE
- Twig
- Webpack Encore
- Stimulus
- Turbo
- Docker Compose pour le développement

## Concept

```text
Développement

Navigateur ou Tauri dev
  -> http://127.0.0.1:${APP_PORT:-8080}
  -> container FrankenPHP
  -> Symfony
  -> container worker Messenger
  -> Mercure
  -> EventSource dans le front
```

```text
Application packagée

Tauri
  -> choisit un port local libre
  -> génère les secrets d'infra (APP_SECRET persistant, secret Mercure éphémère)
  -> lance FrankenPHP sidecar
  -> sert l'app Symfony embarquée
  -> applique les migrations (bloquant)
  -> lance + supervise un worker Messenger sidecar
  -> ouvre la WebView sur http://127.0.0.1:<port>
  -> arrête les sidecars à la fermeture
```

L'application finale n'a pas besoin de Docker, PHP, Composer, Node ou Symfony CLI sur la machine de
l'utilisateur. Le runtime PHP est fourni par le binaire FrankenPHP embarqué. **Aucun secret n'est à
renseigner** : ils sont générés au lancement.

## Pourquoi cette architecture ?

Elle permet de réutiliser une application web Symfony quasi standard dans un shell desktop natif.

Avantages :

- le backend reste un vrai backend HTTP Symfony ;
- la logique métier n'est pas dupliquée entre web et desktop ;
- Doctrine, Messenger, Twig, Mercure et les composants Symfony restent utilisables normalement ;
- Tauri apporte le packaging desktop, la WebView et les intégrations natives si besoin ;
- FrankenPHP fournit un runtime PHP autonome, rapide à lancer et simple à embarquer ;
- le développement reste reproductible grâce à Docker Compose ;
- l'utilisateur final installe un paquet desktop, pas une pile serveur.

Inconvénients :

- l'application packagée lance plusieurs process locaux ;
- il faut gérer proprement le cycle de vie des sidecars ;
- les ports locaux, variables d'environnement et chemins de données doivent être maîtrisés ;
- le debugging mêle Rust, Tauri, FrankenPHP, Symfony et Messenger ;
- SQLite est pratique localement, mais doit être configuré correctement en multi-process.

## Démo incluse

La page d'accueil affiche :

- un bouton `Dispatch async job` ;
- un compteur de réponses HTTP ;
- un compteur de réponses SSE ;
- un compteur `Persisted jobs (DB)` ;
- une zone de logs.

Au clic :

1. Stimulus envoie `POST /api/dispatch`.
2. Symfony crée un `jobId`, **persiste un `DemoJob` (`pending`) en base**, puis dispatch `DemoPingMessage`.
3. Symfony répond immédiatement en JSON.
4. Les compteurs HTTP et `Persisted jobs` augmentent (le compteur DB est mis à jour en live, sans reload).
5. Le worker Messenger consomme le message depuis SQLite.
6. Le handler **passe le `DemoJob` à `done`** (écriture DB depuis le process worker) et publie un
   évènement Mercure sur le topic `app://demo`.
7. L'EventSource du front reçoit l'évènement, le compteur SSE augmente.

Ce flux valide la chaîne complète, y compris la **persistance partagée entre process** :

```text
WebView -> Symfony HTTP (persist) -> Messenger async -> worker (update DB) -> Mercure -> SSE -> WebView
```

## Structure du repo

```text
app/                         Application Symfony 8.1
app/assets/                  Stimulus, Turbo, CSS, entrée Encore
app/src/Entity/              Entités Doctrine (DemoJob)
app/src/Doctrine/            Middleware PRAGMA SQLite
app/migrations/              Migrations Doctrine
app/src/                     Controllers, messages, handlers
app/templates/               Templates Twig
build/Caddyfile.desktop      Caddyfile utilisé par le paquet desktop
build/scripts/               Scripts de build Symfony, sidecar, ressources Tauri
desktop/                     Application Tauri 2
desktop/src-tauri/           Code Rust, config Tauri, bundle
docker/frankenphp/           Image FrankenPHP de développement
docker/node/                 Image Node pour Encore
compose.yaml                 Services app, worker et node
Makefile                     Façade de commandes
```

## Prérequis développement

Sur la machine de dev :

- Docker et Docker Compose ;
- Rust et Cargo ;
- Tauri CLI ;
- les dépendances système Tauri de votre OS ;
- `curl`, `make`, `dpkg` sur Linux si vous construisez le paquet `.deb`.

PHP, Composer, Node et FrankenPHP sont fournis par Docker pour le développement. L'app packagée
embarque son propre FrankenPHP.

## Installation dev

```bash
make composer-install   # dépendances PHP
make assets-install     # dépendances JS
make db                 # tables Messenger + migrations Doctrine
make assets-build       # compilation des assets
make dev                # Symfony + worker
```

L'application est disponible sur `http://127.0.0.1:8080`. Si le port 8080 est déjà pris :

```bash
APP_PORT=18080 make dev
```

## Développement Tauri

```bash
make tauri-dev
```

Dans ce mode, Tauri ouvre `http://127.0.0.1:8080`. Le backend reste celui de Docker : utile pour
travailler l'intégration desktop sans reconstruire un paquet.

## Build desktop

```bash
make tauri-build
```

Cette commande :

1. installe les dépendances Symfony sans `require-dev` ;
2. compile les assets Encore en production ;
3. nettoie et réchauffe le cache Symfony prod ;
4. copie l'application Symfony dans `desktop/src-tauri/resources/app` ;
5. télécharge ou réutilise le sidecar FrankenPHP ;
6. copie le Caddyfile desktop ;
7. lance `cargo tauri build`.

Le paquet Linux `.deb` est généré ici :

```text
desktop/src-tauri/target/release/bundle/deb/TFSApp_0.1.0_amd64.deb
```

Pour éviter les warnings `_apt` quand le paquet est dans un répertoire utilisateur :

```bash
cp desktop/src-tauri/target/release/bundle/deb/TFSApp_0.1.0_amd64.deb /tmp/
sudo apt install --reinstall /tmp/TFSApp_0.1.0_amd64.deb
tfs-app
```

## Cycle de vie desktop

En production, le code Rust :

- résout le chemin des ressources embarquées ;
- choisit un port local libre ;
- génère les secrets d'infra (`APP_SECRET` persistant `0600`, secret Mercure éphémère) ;
- configure les variables d'environnement Symfony, Messenger et Mercure ;
- lance FrankenPHP ;
- exécute `messenger:setup-transports` puis `doctrine:migrations:migrate` (étapes bloquantes) ;
- lance `messenger:consume async` et **le supervise** (relance automatique à chaque sortie) ;
- attend que `/healthz` réponde ;
- ouvre la WebView (titre = `productName`) ;
- arrête le worker et le serveur à la fermeture de la fenêtre ;
- nettoie au démarrage d'éventuels sidecars orphelins de cette app (cas de crash).

Le nettoyage au démarrage est important : si l'app crash ou si le process Tauri est tué brutalement,
un vieux worker pourrait continuer à consommer la queue SQLite et publier vers un ancien hub. Le repo
enregistre les PID des sidecars et identifie les orphelins par des critères dérivés des chemins
d'install (donc sûrs même si plusieurs apps issues de cette base tournent en parallèle).

## Données et chemins runtime

L'app Symfony embarquée est installée en lecture seule :

```text
/usr/lib/TFSApp/resources/app
```

Il ne faut pas écrire dans ce dossier à l'exécution.

Les données modifiables sont placées dans le répertoire de données applicatif Tauri, par ex. sous
Linux :

```text
~/.local/share/dev.local.tfs-app
```

On y trouve : la base SQLite, le cache Symfony runtime, le **build dir** Symfony, les logs,
`data/app.secret` et le fichier PID des sidecars.

Symfony surcharge `getCacheDir()`, `getBuildDir()` et `getLogDir()` pour utiliser `APP_CACHE_DIR`,
`APP_BUILD_DIR` et `APP_LOG_DIR`, afin de ne jamais écrire dans les ressources read-only. Le build
dir est crucial : son défaut Symfony pointe **dans le projet** (donc read-only une fois packagé), et
Symfony y écrit container compilé, pools de cache et proxies ORM.

## Caddy et Mercure

En dev, Docker utilise `docker/frankenphp/Caddyfile`. En paquet desktop, Tauri embarque
`build/Caddyfile.desktop`. Le Caddyfile desktop écoute un port fourni par Tauri
(`http://127.0.0.1:{$APP_PORT}`).

Ne pas utiliser un port fixe en production desktop : un vieux sidecar pourrait rester lancé et voler
le trafic ou les messages. Le port dynamique évite de parler à un ancien serveur.

Le transport Mercure utilise `transport local` : le hub vit dans le process serveur. `transport_url`
et la syntaxe `transport bolt { ... }` sont refusés par certaines versions runtime.

Les secrets Mercure sont **consolidés en une seule variable** : `MERCURE_JWT_SECRET` est utilisée à
la fois par `symfony/mercure-bundle` (signature) et par la directive Caddy `mercure` (validation
`publisher_jwt` ET `subscriber_jwt`). En prod, le launcher la génère aléatoirement à chaque
lancement (éphémère, hub interne loopback). Attention à la longueur : `lcobucci/jwt` impose une clé
HS256 d'au moins 256 bits (32 octets) — les valeurs générées font 64 caractères hex.

## SQLite, Doctrine et Messenger

Le transport Messenger utilise Doctrine DBAL avec SQLite :

```text
MESSENGER_TRANSPORT_DSN=doctrine://default?queue_name=async
```

La connexion `default` et le registre sont fournis par DoctrineBundle ; le transport Messenger les
réutilise. La table `messenger_messages` est gérée par `messenger:setup-transports` et **exclue des
migrations** via `schema_filter` (`~^(?!messenger_messages)~`).

Les PRAGMA SQLite indispensables au multi-process sont appliqués par un **middleware DBAL 4**
(`App\Doctrine\SqlitePragmasMiddleware`) :

```sql
PRAGMA busy_timeout = 5000;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
```

(En DBAL 4 les events `postConnect` ont disparu ; un middleware est la voie supportée.)

### Migrations

Le schéma applicatif est géré par **Doctrine Migrations**. La base vit en app-data et **persiste à
travers les mises à jour** : une nouvelle version ne repart pas d'une base vierge. Toute évolution de
schéma doit donc passer par une migration, appliquée **au lancement** (étape bloquante du launcher,
et `make db` en dev). `migrate` n'exécute que les migrations non encore jouées → upgrade incrémental
v1 → v2 sur une install existante.

Générer une migration après modification d'une entité :

```bash
make symfony-console CMD="doctrine:migrations:diff"
make symfony-console CMD="doctrine:migrations:migrate"
```

## Erreurs à ne pas faire

- **Port fixe pour le backend desktop.** Un port fixe peut rester occupé par un ancien FrankenPHP ;
  la fenêtre chargerait alors un vieux backend / cache / hub.
- **Plusieurs workers desktop.** Sur la même queue SQLite ils se volent les messages et publient
  dans différents hubs. Symptôme : 5 réponses HTTP, 1–2 SSE.
- **Écrire dans les ressources installées.** `/usr/lib/TFSApp/resources/app` appartient au
  paquet (read-only). Cache, build, logs, DB → app-data, via les variables `APP_*_DIR`.
- **Embarquer `var/cache` ou `var/build`.** Un cache/container embarqué peut référencer d'anciens
  hash Encore. Le build les exclut et le runtime les vide au lancement.
- **Supposer une base vierge sur une mise à jour.** L'app-data persiste → gérer les migrations.
- **Modifier le Caddyfile Docker à chaque hypothèse desktop.** Le mode Docker est le témoin : s'il
  marche hors Tauri, garder cette référence et concentrer le debug sur launcher / ports / env /
  sidecars.
- **Oublier de restaurer les dépendances dev après un build** (`composer install --working-dir=app`).
- **Supposer que Tauri tue les sidecars seul.** Cycle de vie géré explicitement + cleanup au prochain
  démarrage.

## Diagnostic

Lister les process Tauri/FrankenPHP :

```bash
pgrep -af 'tfs-app|frankenphp'
```

Process dev Docker attendus :

```text
frankenphp run --config /etc/caddy/Caddyfile
frankenphp php-cli bin/console messenger:consume async -vvv
```

Process desktop attendus pendant que l'app packagée est ouverte :

```text
/usr/bin/frankenphp run --config /usr/lib/TFSApp/resources/Caddyfile.desktop
/usr/bin/frankenphp php-cli bin/console messenger:consume async --time-limit=3600 --memory-limit=256M --env=prod --no-debug
```

(En prod, pas de `-vvv` : les logs passent par le logger Symfony sous `APP_LOG_DIR`.)

Vérifier le Caddyfile embarqué dans le paquet :

```bash
dpkg-deb --fsys-tarfile /tmp/TFSApp_0.1.0_amd64.deb \
  | tar -xO usr/lib/TFSApp/resources/Caddyfile.desktop
```

Vérifier les assets Encore embarqués :

```bash
dpkg-deb --fsys-tarfile /tmp/TFSApp_0.1.0_amd64.deb \
  | tar -xO usr/lib/TFSApp/resources/app/public/build/entrypoints.json
```

Vérifier le container Symfony et le code Rust :

```bash
php app/bin/console lint:container --no-debug
cargo check --manifest-path desktop/src-tauri/Cargo.toml
```

Vérifier le Caddyfile desktop :

```bash
APP_PORT=38124 APP_ORIGIN=http://127.0.0.1:38124 APP_PUBLIC_DIR=/tmp \
MERCURE_JWT_SECRET=0123456789abcdef0123456789abcdef \
frankenphp adapt --config build/Caddyfile.desktop --validate
```

## Commandes

### Makefile

```bash
make composer-install   # dépendances PHP dans le container
make assets-install     # dépendances Node dans le container
make assets-build       # assets en production
make assets-watch       # Encore en watch
make db                 # tables Messenger + migrations Doctrine
make dev                # FrankenPHP + worker en dev
make symfony-console CMD="debug:router"
make tauri-dev          # Tauri dev avec backend Docker
make build-app          # ressources Symfony de prod
make sidecar            # télécharge/vérifie le sidecar FrankenPHP
make tauri-build        # paquet Tauri
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

> Note : `app/package.json` contient un `overrides` forçant `serialize-javascript` ≥ 7.0.6 (faille
> de la toolchain de build webpack-encore). Ne pas le retirer ; `npm audit` doit rester à 0.

### Tauri et Cargo

```bash
. "$HOME/.cargo/env"          # charger l'env Cargo si besoin
cd desktop && cargo tauri dev
cd desktop && cargo tauri build
cargo check --manifest-path desktop/src-tauri/Cargo.toml
```

### Installation du paquet

```bash
cp desktop/src-tauri/target/release/bundle/deb/TFSApp_0.1.0_amd64.deb /tmp/
sudo apt install --reinstall /tmp/TFSApp_0.1.0_amd64.deb
tfs-app
```

## Adapter cette base à une vraie app

Le contrat complet (variables d'environnement, endpoints, chemins, cycle de vie, invariants,
migrations) est dans [CONTRACT.md](CONTRACT.md). Le code Rust est **découplé du nom de l'app** (titre
de fenêtre lu depuis la config, chemins de cleanup dérivés au runtime, logs génériques), donc la
personnalisation se limite à :

**Obligatoire — identité :**

- `desktop/src-tauri/tauri.conf.json` : `productName`, `version`, `identifier`.
  ⚠️ **Ne jamais changer l'`identifier` après publication** → perte de l'app-data utilisateur.
- `desktop/src-tauri/Cargo.toml` : `name` (= nom du binaire), `version` (synchro), `description`,
  `authors`.

**Recommandé :** `app/composer.json` (`name`, `description`).

**Icônes :** remplacer `desktop/src-tauri/icons/icon.png` (**PNG RGBA** obligatoire) et régénérer le
jeu complet via `cargo tauri icon <source.png>`.

**Dérivé automatiquement (ne pas éditer) :** nom du `.deb`, `/usr/lib/<ProductName>/`, app-data
`~/.local/share/<identifier>`, binaire, titre de fenêtre.

Ensuite : remplacer le contenu de la démo Symfony, ajouter une authentification si l'app expose des
données sensibles, ajouter des tests, et les targets de packaging nécessaires (`appimage`, `dmg`,
`msi`…).

## Limites actuelles

- Pas d'authentification.
- Pas de chiffrement de la base locale.
- Bridge natif (secrets keyring utilisateur, dialogs, notifications) non encore implémenté.
- Packaging ciblé sur `.deb` ; le script sidecar couvre Linux x86_64, macOS ARM64 et x86_64.
- Windows n'est pas encore configuré.
- SQLite convient aux apps locales ; à reconsidérer pour des charges importantes.

## Règle mentale utile

Le mode Docker est le témoin. Si cela fonctionne hors Tauri mais pas dans le paquet, regarder en
premier :

1. les process sidecars restés en vie ;
2. les ports locaux ;
3. les variables d'environnement injectées par Rust ;
4. les chemins read-only vs app data (cache, build, log) ;
5. les caches Symfony et assets Encore ;
6. le nombre de workers Messenger actifs.
