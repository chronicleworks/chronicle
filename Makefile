export ISOLATION_ID=local

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


$(MARKERS)/build:
	docker buildx build --platform linux/arm64,linux/amd64 --target chronicle -t $(ISOLATION_ID)_chronicle .
	docker buildx build --platform linux/arm64,linux/amd64 --target chronicle_sawtooth_tp -t $(ISOLATION_ID)_chronicle_sawtooth_tp .

$(MARKERS)/build_cargo: $(MARKERS)/x86_64 # $(MARKERS)/aarch64

.PHONY: clean_containers
clean_containers:
	docker rm $(ISOLATION_ID)_chronicle_musl_aarch64
	docker rm $(ISOLATION_ID)_chronicle_musl_x86_64
