export ISOLATION_ID=local
export REGISTRY=localhost:5000

MAKEFILE_DIR := $(dir $(lastword $(MAKEFILE_LIST)))

export AWS_REGION ?= us-east-1
export AWS_ACCESS_KEY_ID ?=
export AWS_SECRET_ACCESS_KEY ?=
export OPENSSL_STATIC=1

CLEAN_DIRS := $(CLEAN_DIRS)

clean: clean_containers

distclean: clean_docker clean_markers

build: $(MARKERS)/build

package: $(MARKERS)/package

test: $(MARKERS)/test

analyze: analyze_fossa analyze_sonar_cargo

publish: $(MARKERS)/publish_cargo

run: $(MARKERS)/run

$(MARKERS)/run:
	docker compose -f ./docker-compose.yaml up --force-recreate

$(MARKERS)/test:
	docker buildx build --cache-from src=./docker/cache,type=local,dest=./docker/cache --cache-to  type=local,dest=./docker/cache,mode=max --target test .

$(MARKERS)/build:
	docker buildx build --cache-from src=./docker/cache,type=local,dest=./docker/cache --cache-to  type=local,dest=./docker/cache,mode=max --output=type=registry,registry.insecure=true --target chronicle -t $(REGISTRY)/chronicle:$(ISOLATION_ID) .

