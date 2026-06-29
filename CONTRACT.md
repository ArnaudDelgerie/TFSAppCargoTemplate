# Contrat Tauri ↔ Symfony — TFS app

Ce document décrit le **contrat** que doivent respecter un launcher Tauri et une application
Symfony pour être emballés ensemble selon l'architecture de ce dépôt : la **Tauri FrankenPHP
Symfony application**, alias **TFS app**.

L'idée directrice : Tauri n'est qu'un **launcher**. Toute la logique reste dans Symfony. Si ton
app Symfony respecte ce contrat, elle devient packageable en desktop sans réécriture métier — il
suffit de poser le shell Tauri autour.

Ce n'est pas un POC jetable : c'est une **base réutilisable** pour des applications desktop locales.
Le contrat est volontairement minimal et normalisé. Il décrit **ce qui passe entre les deux
mondes** (variables d'environnement, endpoints HTTP, chemins, cycle de vie), pas l'implémentation
interne de chacun.

> Source de vérité : ce contrat est extrait du code réel.
> Launcher : `desktop/src-tauri/src/main.rs`.
> Dev : `compose.yaml`, `docker/frankenphp/Caddyfile`.
> Prod : `build/Caddyfile.desktop`, `app/src/Kernel.php`.

> Naming : `productName` = `TFSApp`, `identifier` = `dev.local.tfs-app`, binaire = `tfs-app`. Dans ce
> document, `<ProductName>` et `<identifier>` désignent ces valeurs, qui pilotent les chemins dérivés
> (voir §10).

---

## 1. Rôles

| Acteur | Responsabilité |
|---|---|
| **Launcher Tauri (Rust)** | Détecter dev/prod, résoudre les chemins, choisir un port libre, générer les secrets d'infra, injecter l'environnement, lancer FrankenPHP (serveur + worker), appliquer les migrations, superviser le worker, attendre `/healthz`, ouvrir la WebView, arrêter proprement les sidecars. |
| **Application Symfony** | Servir l'app HTTP, exposer `/healthz`, lire toute sa config depuis l'environnement, ne jamais écrire dans ses propres sources, publier les events via le hub Mercure. |
| **FrankenPHP / Caddy** | Servir Symfony (worker mode), héberger le hub Mercure, écouter uniquement sur `127.0.0.1`. |

Le launcher ne connaît pas le métier. L'app Symfony ne connaît pas Tauri (elle ne lit que des
variables d'environnement). C'est ce découplage qui rend le wrapping reproductible.

---

## 2. Contrat d'environnement

Variables injectées dans **chaque** process FrankenPHP (serveur ET worker).

| Variable | Dev (Docker) | Prod (Tauri) | Produite par | Consommée par |
|---|---|---|---|---|
| `APP_ENV` | `dev` | `prod` | launcher / compose | Symfony |
| `APP_DEBUG` | `1` | `0` | launcher / compose | Symfony |
| `APP_SECRET` | valeur fixe jetable | **généré + persisté `0600`** | launcher / compose | Symfony (CSRF, signed URIs…) |
| `APP_PORT` | `8080` (host `${APP_PORT:-8080}`) | port libre dynamique | launcher / compose | Caddyfile |
| `APP_ORIGIN` | `http://127.0.0.1:8080` | `http://127.0.0.1:<port>` | launcher / compose | Caddyfile (CORS Mercure) |
| `APP_PUBLIC_DIR` | `/app/public` (implicite) | `<resources>/app/public` | launcher | Caddyfile (`root`, `worker`) |
| `APP_CACHE_DIR` | défaut Symfony | `<app-data>/cache` | launcher | `Kernel::getCacheDir()` |
| `APP_BUILD_DIR` | défaut Symfony | `<app-data>/build` | launcher | `Kernel::getBuildDir()` |
| `APP_LOG_DIR` | défaut Symfony | `<app-data>/log` | launcher | `Kernel::getLogDir()` |
| `DATABASE_URL` | `sqlite:////app/var/data/app.db` | `sqlite:///<app-data>/data/app.db` | launcher / compose | Doctrine DBAL |
| `MESSENGER_TRANSPORT_DSN` | `doctrine://default?queue_name=async` | idem | launcher / compose | Messenger |
| `MERCURE_URL` | `http://127.0.0.1:8080/.well-known/mercure` (serveur) · `http://app:8080/...` (worker) | `http://127.0.0.1:<port>/.well-known/mercure` | launcher / compose | `symfony/mercure-bundle` (publish) |
| `MERCURE_PUBLIC_URL` | `http://127.0.0.1:<host-port>/.well-known/mercure` | idem `MERCURE_URL` | launcher / compose | bundle (URL publique) |
| `MERCURE_JWT_SECRET` | valeur fixe jetable ≥ 32 o | **généré éphémère** ≥ 32 o | launcher / compose | bundle (signe) **et** Caddy (valide publisher + subscriber) |

### Deux familles de secrets

- **Secrets d'infra** (nécessaires au boot, jamais vus par l'utilisateur) → générés par le launcher.
  - `APP_SECRET` : **persistant**. Lu depuis `<app-data>/data/app.secret`, généré (32 octets, unique
    par installation) et écrit en `0600` au premier lancement. Il doit rester stable : tout ce que
    Symfony signe (CSRF, signed URIs, futurs cookies remember-me) doit continuer à valider après un
    redémarrage.
  - `MERCURE_JWT_SECRET` : **éphémère**. Régénéré à chaque lancement, jamais persisté. Le hub est
    interne (loopback, `transport local`, en mémoire), personne d'externe n'a besoin de ce secret.
- **Secrets utilisateur** (clés API…) → coffre OS (keyring) via un futur bridge natif (hors périmètre de ce contrat).
  Accès déclenché par une action UI, jamais bloquant au boot.

Conséquence : **aucun secret n'est codé en dur dans le paquet**, et **aucun secret n'est à renseigner**
côté utilisateur pour démarrer.

### Règles dérivées

- **Un seul secret Mercure.** Le bundle signe avec `MERCURE_JWT_SECRET` ; le Caddyfile valide
  `publisher_jwt` ET `subscriber_jwt` avec la même variable. Inutile d'avoir des clés séparées sur
  loopback en mode `anonymous`.
- **Le secret Mercure fait ≥ 256 bits (32 octets).** `symfony/mercure` (via `lcobucci/jwt`) refuse
  une clé HS256 plus courte : `Key provided is shorter than 256 bits`. Les valeurs générées font
  64 caractères hex (32 octets), au-dessus du seuil.
- Le launcher **supprime** `MERCURE_TRANSPORT_URL` de l'environnement hérité avant de lancer
  FrankenPHP (`env_remove`), car cette variable casse la directive `mercure` selon les runtimes.

### Différence dev/prod à connaître : `MERCURE_URL` du worker

Le worker **publie** sur le hub via HTTP. Le hub vit dans le process **serveur**.

- En **dev**, serveur et worker sont des conteneurs distincts → le worker vise le serveur par son
  nom de service : `MERCURE_URL=http://app:8080/.well-known/mercure`.
- En **prod**, serveur et worker partagent la loopback → le worker vise
  `http://127.0.0.1:<port>/.well-known/mercure`.

Invariant : **`MERCURE_URL` du worker pointe toujours vers le hub réellement en écoute.** C'est la
seule raison pour laquelle `MERCURE_URL` et `MERCURE_PUBLIC_URL` diffèrent en dev (et sont identiques
en prod).

### Worker mode FrankenPHP

`symfony/runtime` bascule automatiquement sur `FrankenPhpWorkerRunner` dès que `FRANKENPHP_WORKER`
est vrai (positionné par FrankenPHP en worker mode). Aucune dépendance supplémentaire.

- **Dev** : `FRANKENPHP_RESET_KERNEL=1` (kernel rebooté à chaque requête → pas d'état périmé pendant
  le développement).
- **Prod** : pas de reset (kernel chaud, performances).

---

## 3. Contrat HTTP

L'app Symfony **doit** exposer :

| Endpoint | Méthode | Réponse | Rôle |
|---|---|---|---|
| `/healthz` | GET | `200` `{"status":"ok"}` | Gate de readiness. Le launcher n'ouvre la WebView qu'après un `200`. |
| `/.well-known/mercure` | GET/POST | géré par Caddy | Hub Mercure (SSE + publication). |

- Le launcher interroge `/healthz` toutes les 250 ms, **timeout 20 s**. Pas de `200` → l'app ne
  démarre pas. C'est le seul signal de readiness ; ne pas le retirer.
- Le front s'abonne au hub via `EventSource` sur l'origine courante (`window.location.origin`), pas
  besoin de connaître le port à l'avance.

### Pas de HTTPS — invariant assumé

Tout écoute uniquement sur `127.0.0.1`. Le trafic ne quitte jamais la loopback : il ne touche aucune
interface réseau, donc TLS ne protégerait contre aucune menace réelle (le seul attaquant pertinent
est un malware déjà présent sous le compte utilisateur, contre lequel HTTPS ne protège pas).
`127.0.0.1` est par ailleurs traité comme *secure context* par la WebView même en HTTP.

Ce n'est **pas** une dette de POC : c'est le bon choix tant que l'architecture reste loopback. HTTPS
n'aurait de sens que si l'app sortait de la loopback (réseau partagé, bridge multi-machines) — ce qui
sort du périmètre de ce contrat.

---

## 4. Contrat de système de fichiers

Deux zones strictement séparées en prod.

| Zone | Chemin (exemple Linux) | Accès | Contenu |
|---|---|---|---|
| **Ressources embarquées** | `/usr/lib/<ProductName>/resources/app` | **lecture seule** | sources Symfony, vendor, assets compilés |
| **App-data** | `~/.local/share/<identifier>` | lecture/écriture | DB SQLite, cache, build dir, logs, secret d'app, fichier PID |

Invariants :

- **Ne jamais écrire dans les ressources embarquées** à l'exécution (le paquet en est propriétaire).
- **Les trois répertoires d'écriture Symfony sont relocalisés en app-data** via l'environnement :
  `APP_CACHE_DIR`, `APP_BUILD_DIR`, `APP_LOG_DIR`. Le `Kernel` surcharge `getCacheDir()`,
  `getBuildDir()` et `getLogDir()` en conséquence (voir `app/src/Kernel.php`). Oublier le build dir
  est un piège silencieux : Symfony y écrit (container compilé, pools de cache, proxies ORM) et son
  défaut pointe **dans le projet** — donc dans les ressources read-only.
- Le cache et le build dir prod **ne sont pas embarqués** dans le paquet, et le launcher **les vide
  au démarrage** (un cache Twig ou un container embarqué peut référencer d'anciens hash Encore ou
  chemins de version précédente).
- DB SQLite, transport Mercure (si applicable), logs, `app.secret` et PID vivent **uniquement** en
  app-data.
- **L'app-data persiste à travers les mises à jour** du paquet (réinstallation `.deb`). Voir §8
  (migrations) : ne jamais supposer une base vierge sur une nouvelle version.

> Règle mentale fiable : **tout chemin d'écriture Symfony doit dériver d'une variable d'env injectée,
> jamais d'un défaut relatif au projet** (uploads, sessions fichier, etc. inclus).

---

## 5. Séquence de démarrage (prod)

Ordre imposé par le launcher (`main.rs`) :

1. Résoudre `resources/app`, `resources/app/public`, `resources/Caddyfile.desktop`.
2. Résoudre l'app-data ; définir `pid_file`, `cache_dir`, `build_dir`.
3. **Cleanup** : tuer les sidecars listés dans l'ancien `sidecars.pids`, puis scanner `/proc` pour
   tuer d'éventuels sidecars orphelins **de cette app** (crash précédent). L'identification est
   dérivée des chemins d'install au runtime (Caddyfile résolu pour le serveur ; commande Messenger
   + répertoire de travail pour le worker), donc sûre même si plusieurs apps issues de cette base
   tournent en parallèle.
4. Vider puis recréer `cache_dir` et `build_dir` ; créer `data/` et `log/`.
5. Générer les secrets d'infra : `MERCURE_JWT_SECRET` (éphémère) et `APP_SECRET` (lu ou créé `0600`).
6. Valider la présence de `bin/console`, `public/index.php`, `Caddyfile.desktop` (sinon abort).
7. Résoudre le binaire FrankenPHP (à côté de l'exécutable, sinon `/usr/bin/frankenphp`).
8. Choisir un **port libre** (`127.0.0.1:0`).
9. Lancer le **serveur** : `frankenphp run --config <Caddyfile.desktop>`.
10. **Étapes bloquantes** (avant la WebView, doivent réussir) :
    - `messenger:setup-transports`
    - `doctrine:migrations:migrate --allow-no-migration`
11. Lancer le **worker** : `messenger:consume async --time-limit=3600 --memory-limit=256M --env=prod --no-debug`.
12. Démarrer le **superviseur** de worker (thread Rust) : il relance le worker à chaque sortie
    (limite de temps/mémoire ou crash), tant que l'app n'est pas en fermeture.
13. Écrire les PID (serveur, worker) dans `sidecars.pids` (réécrit à chaque respawn).
14. Attendre `/healthz == 200`.
15. Ouvrir la WebView sur `http://127.0.0.1:<port>` (titre = `productName`).
16. À la fermeture (`CloseRequested`) : signaler l'arrêt au superviseur, tuer worker puis serveur,
    supprimer `sidecars.pids`.

En **dev**, le launcher ne lance **aucun** sidecar : il ouvre simplement `http://127.0.0.1:8080`,
le backend étant fourni par `docker compose up app worker`.

---

## 6. Contrat de cycle de vie des process

- **Écoute uniquement sur `127.0.0.1`.** Jamais de bind `0.0.0.0`. Pas de HTTPS (voir §3).
- **Port dynamique en prod.** Un port fixe peut être squatté par un ancien sidecar → la fenêtre
  parlerait à un vieux backend / vieux hub. Le port libre garantit qu'on parle au bon serveur.
- **Un seul worker par queue SQLite.** Plusieurs workers se volent les messages et publient sur des
  hubs différents. Symptôme : 5 réponses HTTP mais 1–2 SSE.
- **Le worker est supervisé.** `--time-limit`/`--memory-limit` le font se recycler (sain pour un
  process PHP long-vécu) ; le superviseur le relance ensuite. Sans ça, le worker s'arrêterait après
  sa première limite et les jobs async ne seraient plus jamais consommés (panne silencieuse).
- **Le launcher possède le cycle de vie.** Ne pas supposer que Tauri tue les enfants seul : kill
  explicite à la fermeture + cleanup au prochain démarrage pour les cas de crash.
- **PID persistés** dans `sidecars.pids` pour permettre le cleanup après un crash.

---

## 7. Contrat Caddyfile

Le Caddyfile desktop (`build/Caddyfile.desktop`) doit :

```caddy
{
    auto_https off
    admin off
}

http://127.0.0.1:{$APP_PORT} {
    root * {$APP_PUBLIC_DIR}

    mercure {
        publisher_jwt {$MERCURE_JWT_SECRET}
        subscriber_jwt {$MERCURE_JWT_SECRET}
        anonymous
        transport local
        cors_origins {$APP_ORIGIN} tauri://localhost
    }

    php_server {
        worker {$APP_PUBLIC_DIR}/index.php
    }
}
```

Invariants :

- Écoute sur `{$APP_PORT}` (jamais en dur).
- `root` et `worker` dérivent de `{$APP_PUBLIC_DIR}`.
- `publisher_jwt` et `subscriber_jwt` utilisent **la même** variable `{$MERCURE_JWT_SECRET}`.
- `transport local` : le hub vit dans le process serveur (choix le plus stable cross-runtime).
  `transport_url` et `transport bolt { ... }` sont rejetés par certains binaires.
- `cors_origins` inclut `{$APP_ORIGIN}` et `tauri://localhost`.
- Le Caddyfile **dev** (`docker/frankenphp/Caddyfile`) reste le **témoin** : même structure, port
  `8080`, `root */app/public`. S'il marche hors Tauri, on garde cette référence stable et on
  concentre le debug desktop sur ports / env / sidecars.

---

## 8. Contrat de migration de schéma

La DB SQLite vit en app-data et **persiste à travers les mises à jour** (l'`identifier` ne change
pas → même dossier app-data → même base). Conséquence :

> **Tout changement de schéma entre deux versions livrées DOIT être appliqué par une migration
> exécutée au lancement, en étape bloquante, avant la WebView, de façon idempotente. Ne jamais
> supposer une base vierge sur une nouvelle version.**

- L'outillage est **Doctrine ORM + Doctrine Migrations**. `doctrine:migrations:migrate` n'applique
  que les migrations non encore jouées (table `doctrine_migration_versions` en app-data), ce qui
  réalise le pont v1 → v2 sur une install existante.
- L'étape tourne dans la séquence de démarrage (§5.10), juste après `messenger:setup-transports`,
  avec `--allow-no-migration` (succès même si rien à appliquer).
- En dev, `make db` exécute `setup-transports` puis `migrations:migrate`.

---

## 9. Contrat stack base de données

- **Doctrine DBAL + ORM + Migrations** via DoctrineBundle. La connexion `default` et le registre sont
  fournis par le bundle ; le transport Messenger `doctrine://default` les réutilise.
- **PRAGMA SQLite** (`busy_timeout=5000`, `journal_mode=WAL`, `synchronous=NORMAL`) appliqués via un
  **middleware DBAL 4** (`App\Doctrine\SqlitePragmasMiddleware`), les events `postConnect` ayant
  disparu en DBAL 4. Indispensable car serveur HTTP et worker sont deux process partageant le fichier.
- **`schema_filter`** (`~^(?!messenger_messages)~`) exclut la table Messenger des diffs de migration :
  elle reste gérée par `messenger:setup-transports`, pas par l'ORM.
- Assets compilés en prod et présents dans le bundle (`public/build/entrypoints.json`).

---

## 10. Personnalisation : dupliquer cette base

Le code Rust est **découplé du nom de l'app** (titre de fenêtre lu depuis la config, chemins de
cleanup dérivés au runtime, logs génériques). Points à renseigner pour une nouvelle app :

**Obligatoire — identité :**

- [ ] `desktop/src-tauri/tauri.conf.json` : `productName`, `version`, `identifier`.
      ⚠️ **Ne jamais changer l'`identifier` après publication** → les utilisateurs perdraient leur
      app-data (DB, secret).
- [ ] `desktop/src-tauri/Cargo.toml` : `name` (= nom du binaire), `version` (synchro avec tauri.conf),
      `description`, `authors`.

**Recommandé — cosmétique :**

- [ ] `app/composer.json` : `name`, `description`.

**Icônes :**

- [ ] Remplacer `desktop/src-tauri/icons/icon.png` (**PNG RGBA** obligatoire — Tauri rejette un PNG
      non-RGBA) et régénérer le jeu complet via `cargo tauri icon <source.png>`.

**Dérivé automatiquement — ne pas éditer à la main :** nom du `.deb`, `/usr/lib/<ProductName>/`,
app-data `~/.local/share/<identifier>`, binaire, titre de fenêtre.

---

## 11. Checklist : emballer ta propre app Symfony

Pour rendre une app Symfony existante compatible avec ce launcher :

- [ ] Exposer `GET /healthz` → `200 {"status":"ok"}`.
- [ ] Lire toute la config depuis l'environnement (`DATABASE_URL`, `MESSENGER_TRANSPORT_DSN`,
      `MERCURE_*`, `APP_*`). Aucune valeur en dur dépendante de l'install.
- [ ] Surcharger `Kernel::getCacheDir()` / `getBuildDir()` / `getLogDir()` sur `APP_CACHE_DIR` /
      `APP_BUILD_DIR` / `APP_LOG_DIR`.
- [ ] Ne jamais écrire dans les sources à l'exécution (cache, build, logs, DB, uploads → app-data).
- [ ] Transport Messenger **partagé entre process** (Doctrine), pas `sync://` ni in-memory.
- [ ] Un seul `MERCURE_JWT_SECRET`, ≥ 32 octets, utilisé par le bundle et par Caddy.
- [ ] SQLite multi-process via le middleware PRAGMA (`busy_timeout`, `journal_mode=WAL`,
      `synchronous=NORMAL`).
- [ ] Schéma applicatif géré par Doctrine Migrations, appliqué au lancement (voir §8).
- [ ] Assets compilés en prod et présents dans le bundle (`public/build/entrypoints.json`).
- [ ] Fournir un Caddyfile desktop conforme à la section 7.
- [ ] Garder le mode Docker fonctionnel comme témoin.

Si toutes les cases sont cochées, poser le shell Tauri autour est mécanique : copier `desktop/`,
ajuster l'identité (§10), fournir le sidecar FrankenPHP, builder.

---

## 12. Hors contrat (volontairement)

Ces points ne font pas partie du contrat minimal et sont laissés à l'app :

- Authentification, chiffrement de la DB locale.
- Bridge natif Tauri ↔ Symfony (secrets keyring utilisateur, dialogs, notifications).
- Auto-update, signature de code, notarisation.
- Packaging Windows (`.msi`) — frontière connue, non couverte aujourd'hui.
