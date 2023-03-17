MAKEFILE_DIR := $(dir $(lastword $(MAKEFILE_LIST)))
include $(MAKEFILE_DIR)/standard_defs.mk

export OPENSSL_STATIC=1
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1


IMAGES := chronicle chronicle-tp chronicle-builder opa-tp opactl
ARCHS := amd64 arm64
COMPOSE ?= docker-compose
HOST_ARCHITECTURE ?= $(shell uname -m | sed -e 's/x86_64/amd64/' -e 's/aarch64/arm64/')

CLEAN_DIRS := $(CLEAN_DIRS)

clean: clean_containers clean_target clean-opa

distclean: clean_docker clean_markers

analyze: analyze_fossa

publish: gh-create-draft-release
	mkdir -p target/arm64
	mkdir -p target/amd64
	container_id=$$(docker create chronicle-tp-amd64:${ISOLATION_ID}); \
		docker cp $$container_id:/usr/local/bin/chronicle_sawtooth_tp `pwd`/target/amd64/;  \
		docker rm $$container_id;
	container_id=$$(docker create chronicle-amd64:${ISOLATION_ID}); \
		docker cp $$container_id:/usr/local/bin/chronicle `pwd`/target/amd64/; \
		docker rm $$container_id;
	container_id=$$(docker create chronicle-tp-arm64:${ISOLATION_ID}); \
		docker cp $$container_id:/usr/local/bin/chronicle_sawtooth_tp `pwd`/target/arm64;  \
		docker rm $$container_id;
	container_id=$$(docker create chronicle-arm64:${ISOLATION_ID}); \
		docker cp $$container_id:/usr/local/bin/chronicle `pwd`/target/arm64; \
		docker rm $$container_id;
	if [ "$(RELEASABLE)" = "yes" ]; then \
		$(GH_RELEASE) upload $(VERSION) target/* ; \
	fi

.PHONY: build-end-to-end-test
build-end-to-end-test:
	docker build -t chronicle-test:$(ISOLATION_ID) -f docker/chronicle-test/chronicle-test.dockerfile .

.PHONY: test-chronicle-e2e
test-chronicle-e2e: build-end-to-end-test
	COMPOSE_PROFILES=test $(COMPOSE) -f docker/chronicle.yaml up --exit-code-from chronicle-test

.PHONY: test-e2e
test-e2e: test-chronicle-e2e

run:
	$(COMPOSE) -f docker/chronicle.yaml up -d

.PHONY: stop
stop:
	$(COMPOSE) -f docker/chronicle.yaml down || true

$(MARKERS)/binfmt:
	mkdir -p $(MARKERS)
	if [ `uname -m` = "x86_64" ]; then \
		docker run --rm --privileged multiarch/qemu-user-static --reset -p yes; \
	fi
	touch $@

# Run the compiler for host and target, then extract the binaries
.PHONY: tested-$(ISOLATION_ID)
test: tested-$(ISOLATION_ID)
tested-$(ISOLATION_ID): $(HOST_ARCHITECTURE)-ensure-context
	docker buildx build $(DOCKER_PROGRESS)  \
		-f./docker/unified-builder \
		-t tested-artifacts:$(ISOLATION_ID) . \
		--builder ctx-$(ISOLATION_ID)-$(HOST_ARCHITECTURE) \
		--platform linux/$(HOST_ARCHITECTURE) \
		--target test \
		--load

	rm -rf .artifacts
	mkdir -p .artifacts

	container_id=$$(docker create tested-artifacts:${ISOLATION_ID}); \
		docker cp $$container_id:/artifacts `pwd`/.artifacts/ \
		&& docker rm $$container_id

.PHONY: test-e2e
test: test-e2e
define arch-contexts =
.PHONY: $(1)-ensure-context
$(1)-ensure-context: $(MARKERS)/binfmt
	docker buildx create --name ctx-$(ISOLATION_ID)-$(1) \
		--driver docker-container \
		--bootstrap || true
	docker buildx use ctx-$(ISOLATION_ID)-$(1)

.PHONY: clean-$(1)-ensure-context
clean: clean-$(1)-ensure-context
clean-$(1)-ensure-context:
	@docker buildx rm ctx-$(ISOLATION_ID)-$(1) || true

endef
$(foreach arch,$(ARCHS),$(eval $(call arch-contexts,$(arch))))


define multi-arch-docker =

.PHONY: $(1)-$(2)-build
$(1)-$(2)-build: $(2)-ensure-context  policies/bundle.tar.gz
	docker buildx build $(DOCKER_PROGRESS)  \
		-f./docker/unified-builder \
		-t $(1)-$(2):$(ISOLATION_ID) . \
		--builder ctx-$(ISOLATION_ID)-$(2) \
		--platform linux/$(2) \
		--target $(1) \
		--load

$(1)-manifest: $(1)-$(2)-manifest
$(1)-$(2)-manifest: $(1)-$(2)-build
	docker manifest create $(1):$(ISOLATION_ID) \
		-a $(1)-$(2):$(ISOLATION_ID)

ifeq ($(RELEASABLE), yes)
$(1): $(1)-$(2)-build
else
ifeq ($(2), $(HOST_ARCHITECTURE))
$(1): $(1)-$(2)-build
endif
endif

build: $(1)

build-native: $(1)-$(HOST_ARCHITECTURE)-build
endef

$(foreach image,$(IMAGES),$(foreach arch,$(ARCHS),$(eval $(call multi-arch-docker,$(image),$(arch)))))

clean_containers:
	$(COMPOSE) -f docker/chronicle.yaml rm -f || true

clean_docker: stop
	$(COMPOSE) -f docker/chronicle.yaml down -v --rmi all || true

clean_target:
	$(RM) -r target

uname_S := $(shell uname -s)
uname_M := $(shell uname -m)

ifeq ($(uname_S), Linux)
	OS = linux
	OPA_SUFFIX = _static
else ifeq ($(uname_S), Darwin)
	OS = darwin
else
	OS = windows
	ARCH = amd64
endif

ifeq ($(uname_M), x86_64)
	ARCH = amd64
else ifeq ($(uname_M), arm)
	ARCH = arm64
	OPA_SUFFIX = _static
else ifeq ($(uname_M), arm64)
	ARCH = arm64
	OPA_SUFFIX = _static
else ifeq ($(uname_M), aarch64)
	ARCH = arm64
	OPA_SUFFIX = _static
endif

OPA_VERSION=v0.49.2
OPA_DOWNLOAD_URL=https://openpolicyagent.org/downloads/$(OPA_VERSION)/opa_$(OS)_$(ARCH)$(OPA_SUFFIX)

build/opa:
	curl -sSL -o build/opa $(OPA_DOWNLOAD_URL)
	chmod 755 build/opa

build: policies/bundle.tar.gz

policies/bundle.tar.gz: build/opa
	mkdir -p policies
	build/opa build -t wasm -o policies/bundle.tar.gz -b policies -e "allow_transactions" -e "common_rules"

test: opa-test
.PHONY: opa-test
opa-test: build/opa
	build/opa test -b policies

clean: clean-opa
.PHONY: clean-opa
clean-opa:
	$(RM) policies/*.tar.gz
