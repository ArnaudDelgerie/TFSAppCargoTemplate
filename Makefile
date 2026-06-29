COMPOSE=docker compose
APP_RUN=$(COMPOSE) run --rm app
NODE_RUN=$(COMPOSE) run --rm node

.PHONY: dev assets-install assets-watch assets-build composer-install db symfony-console tauri-dev build-app tauri-build sidecar

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
