export ISOLATION_ID=local

MAKEFILE_DIR := $(dir $(lastword $(MAKEFILE_LIST)))

export AWS_REGION ?= us-east-1
export AWS_ACCESS_KEY_ID ?=
export AWS_SECRET_ACCESS_KEY ?=
export OPENSSL_STATIC=1

CLEAN_DIRS := $(CLEAN_DIRS) 

clean: clean_containers

distclean: clean_docker clean_markers

build: $(MARKERS)/build_musl $(MARKERS)/build_cargo 

package: $(MARKERS)/package_cargo $(MARKERS)/package_docker

test: $(MARKERS)/test_cargo 

analyze: analyze_fossa analyze_sonar_cargo

publish: $(MARKERS)/publish_cargo

$(MARKERS)/aarch64:
	docker run --rm -it -v "$(PWD)":/home/rust/src -w /home/rust/src $(ISOLATION_ID)_chronicle_musl  cargo build --release --target=aarch64-unknown-linux-musl

$(MARKERS)/x86_64:
	docker run --rm -it -v "$(PWD)":/home/rust/src -w /home/rust/src $(ISOLATION_ID)_chronicle_musl cargo build --release --target=x86_64-unknown-linux-musl

$(MARKERS)/build_musl: 
	docker build docker/build/ -t $(ISOLATION_ID)_chronicle_musl

$(MARKERS)/build_cargo: $(MARKERS)/x86_64 # $(MARKERS)/aarch64

.PHONY: clean_containers
clean_containers:
	docker rm $(ISOLATION_ID)_chronicle_musl_aarch64
	docker rm $(ISOLATION_ID)_chronicle_musl_x86_64
