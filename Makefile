MAKEFILE_DIR := $(dir $(lastword $(MAKEFILE_LIST)))
include $(MAKEFILE_DIR)/standard_defs.mk

export OPENSSL_STATIC=1
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

IMAGES := chronicle chronicle-tp chronicle-builder
ARCHS := amd64 arm64

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
	docker-compose -f docker/chronicle.yaml down || true

$(MARKERS)/binfmt:
	mkdir -p $(MARKERS)
	docker run --rm --privileged multiarch/qemu-user-static --reset -p yes
	touch $@

define multi-arch-docker =

.PHONY: ensure-context-$(1)
$(1)-$(2)-ensure-context: $(MARKERS)/binfmt
	docker buildx create --name ctx-$(ISOLATION_ID) \
		--driver docker-container \
		--bootstrap || true
	docker buildx use ctx-$(ISOLATION_ID)

$(1)-$(2)-build: $(1)-$(2)-ensure-context
	docker buildx build -f ./docker/$(1).dockerfile -t $(1)-$(2):$(ISOLATION_ID) . \
		--platform linux/$(2) \
		--load

$(1)-$(2)-build-native: $(1)-$(2)-ensure-context
	if [ "$(2)" = "amd64" ]; then \
		docker buildx build -f ./docker/$(1).dockerfile -t $(1):$(ISOLATION_ID) . \
			--platform linux/$(2) \
			--load ; \
	fi

$(1)-manifest: $(1)-$(2)-build
	docker manifest create $(1):$(ISOLATION_ID) \
		-a $(1)-$(2):$(ISOLATION_ID)

$(1): $(1)-$(2)-build $(1)-$(2)-build-native

build: $(1)
endef

$(foreach image,$(IMAGES),$(foreach arch,$(ARCHS),$(eval $(call multi-arch-docker,$(image),$(arch)))))

chronicle-builder-ensure-context:
	docker buildx create --name $(ISOLATION_ID) \
		--driver docker-container \
		--bootstrap || true
	docker buildx use $(ISOLATION_ID)

chronicle-builder-build: chronicle-builder-ensure-context
	docker buildx build -f ./docker/chronicle-builder.dockerfile \
		-t chronicle-builder:$(ISOLATION_ID) . \
		--load

build: chronicle-builder-build

clean_containers:
	docker-compose -f docker/chronicle.yaml rm -f || true
	docker-compose -f docker/docker-compose.yaml rm -f || true

clean_docker: stop
	docker-compose -f docker/chronicle.yaml down -v --rmi all || true
	docker-compose -f docker/docker-compose.yaml down -v --rmi all || true

clean_target:
	$(RM) -r target
