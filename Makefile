MAKEFILE_DIR := $(dir $(lastword $(MAKEFILE_LIST)))
include $(MAKEFILE_DIR)/standard_defs.mk

export OPENSSL_STATIC=1
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

IMAGES := chronicle chronicle-tp chronicle-builder
ARCHS := amd64 arm64
HOST_ARCHITECTURE ?= $(shell uname -m | sed -e 's/x86_64/amd64/' -e 's/aarch64/arm64/')

# Don't use fancy output if we are running in Jenkins
ifneq ($(JENKINS_URL),)
DOCKER_PROGRESS := --progress=plain
endif

CLEAN_DIRS := $(CLEAN_DIRS)

clean: clean_containers clean_target

distclean: clean_docker clean_markers

analyze: analyze_fossa

publish: gh-create-draft-release
	container_id=$$(docker create chronicle-tp:${ISOLATION_ID}); \
		docker cp $$container_id:/usr/local/bin/chronicle_sawtooth_tp `pwd`/target/;  \
		docker rm $$container_id;
	container_id=$$(docker create chronicle:${ISOLATION_ID}); \
		docker cp $$container_id:/usr/local/bin/chronicle `pwd`/target/; \
		docker rm $$container_id;
	if [ "$(RELEASABLE)" = "yes" ]; then \
		$(GH_RELEASE) upload $(VERSION) target/* ; \
	fi

run:
	docker-compose -f docker/chronicle.yaml up --force-recreate

.PHONY: stop
stop:
	$(COMPOSE) -f docker/chronicle.yaml down || true

$(MARKERS)/binfmt:
	mkdir -p $(MARKERS)
	if [ `uname -m` = "x86_64" ]; then \
		docker run --rm --privileged multiarch/qemu-user-static --reset -p yes; \
	fi
	touch $@

define multi-arch-docker =

.PHONY: ensure-context-$(1)
$(1)-$(2)-ensure-context: $(MARKERS)/binfmt
	docker buildx create --name ctx-$(ISOLATION_ID) \
		--driver docker-container \
		--bootstrap || true
	docker buildx use ctx-$(ISOLATION_ID)

.PHONY: $(1)-$(2)-build
$(1)-$(2)-build: $(1)-$(2)-ensure-context
	docker buildx build $(DOCKER_PROGRESS)  \
		-f./docker/unified-builder \
		-t $(1)-$(2):$(ISOLATION_ID) . \
		--platform linux/$(HOST_ARCHITECTURE) \
		--load

$(1)-manifest: $(1)-$(2)-build
	docker manifest create $(1):$(ISOLATION_ID) \
		-a $(1)-$(2):$(ISOLATION_ID)

$(1): $(1)-$(2)-build

build: $(1)

build-native: $(1)-$(HOST_ARCHITECTURE)-build
endef

$(foreach image,$(IMAGES),$(foreach arch,$(ARCHS),$(eval $(call multi-arch-docker,$(image),$(arch)))))

chronicle-builder-ensure-context:
	docker buildx create --name ctx-$(ISOLATION_ID) \
		--driver docker-container \
		--bootstrap || true
	docker buildx use ctx-$(ISOLATION_ID)

clean_containers:
	docker-compose -f docker/chronicle.yaml rm -f || true

clean_docker: stop
	docker-compose -f docker/chronicle.yaml down -v --rmi all || true

clean_target:
	$(RM) -r target
