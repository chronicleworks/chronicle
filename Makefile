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

package: $(MARKERS)/package_cargo $(MARKERS)/package_docker

test: $(MARKERS)/test_cargo 

analyze: analyze_fossa analyze_sonar_cargo

publish: $(MARKERS)/publish_cargo

run: $(MARKERS)/run

$(MARKERS)/run:
	docker compose -f ./docker-compose.yaml up --force-recreate

$(MARKERS)/build:
	docker buildx build --platform linux/arm64,linux/amd64 --cache-from src=./docker/cache,type=local,dest=./docker/cache --cache-to  type=local,dest=./docker/cache,mode=max --output=type=registry,registry.insecure=true --target chronicle -t $(REGISTRY)/chronicle:$(ISOLATION_ID) .
	docker buildx build --platform linux/arm64,linux/amd64 --cache-from src=./docker/cache,type=local,dest=./docker/cache --cache-to  type=local,dest=./docker/cache,mode=max --output=type=registry,registry.insecure=true --target chronicle_sawtooth_tp -t $(REGISTRY)/chronicle-sawtooth-tp:$(ISOLATION_ID) .

$(MARKERS)/build_cargo: $(MARKERS)/x86_64 # $(MARKERS)/aarch64

.PHONY: clean_containers
clean_containers:
	docker rm $(ISOLATION_ID)_chronicle_musl_aarch64
	docker rm $(ISOLATION_ID)_chronicle_musl_x86_64
