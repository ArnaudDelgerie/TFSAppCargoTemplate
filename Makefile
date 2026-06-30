# Pass the host UID/GID so containers write bind-mounted files (var/, vendor/,
# node_modules, public/build) as you instead of root. See compose.yaml.
DOCKER_UID := $(shell id -u)
DOCKER_GID := $(shell id -g)
COMPOSE=UID=$(DOCKER_UID) GID=$(DOCKER_GID) docker compose
APP_RUN=$(COMPOSE) run --rm app
NODE_RUN=$(COMPOSE) run --rm node

.PHONY: dev assets-install assets-watch assets-build composer-install db symfony-console tauri-dev build-app tauri-build sidecar smoke clean

# Template self-test: generate representative variants into a temp dir and
# cargo-check each (the template repo does not cargo check at its root).
smoke:
	build/scripts/smoke.sh

# Remove dev build artifacts from app/. Runs as root inside the container to also
# clear any root-owned leftovers from before the host-user mapping was in place.
clean:
	$(COMPOSE) run --rm --user 0:0 app rm -rf var node_modules public/build

dev:
	$(COMPOSE) up app worker

assets-install:
	$(NODE_RUN) npm install

assets-watch:
	$(NODE_RUN) npm run watch

assets-build:
	$(NODE_RUN) npm run build

composer-install:
	$(APP_RUN) composer install

db:
	$(APP_RUN) mkdir -p var/data
	$(APP_RUN) frankenphp php-cli bin/console messenger:setup-transports --no-interaction
	$(APP_RUN) frankenphp php-cli bin/console doctrine:migrations:migrate --no-interaction --allow-no-migration

symfony-console:
	$(APP_RUN) frankenphp php-cli bin/console $(CMD)

tauri-dev:
	$(COMPOSE) up -d app worker
	cd desktop && cargo tauri dev

build-app:
	build/scripts/build-app.sh

sidecar:
	build/scripts/download-frankenphp-sidecar.sh

tauri-build: build-app sidecar
	build/scripts/prepare-tauri-resources.sh
	cd desktop && cargo tauri build
