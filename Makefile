MAKEFILE_DIR := $(dir $(lastword $(MAKEFILE_LIST)))
include $(MAKEFILE_DIR)/standard_defs.mk

export OPENSSL_STATIC=1

CLEAN_DIRS := $(CLEAN_DIRS)

clean: clean_containers clean_target

distclean: clean_docker clean_markers

build: $(MARKERS)/build

analyze: analyze_fossa

publish: gh-create-draft-release
	container_id=$$(docker create chronicle:${ISOLATION_ID}); \
	  docker cp $$container_id:/usr/local/bin/chronicle `pwd`/target/ && \
	  docker cp $$container_id:/usr/local/bin/chronicle_sawtooth_tp `pwd`/target/ && \
		target/chronicle --export-schema > `pwd`/target/chronicle.graphql 2>&1 && \
		docker rm $$container_id
	if [ "$(RELEASABLE)" = "yes" ]; then \
	  $(GH_RELEASE) upload $(VERSION) target/* ; \
	fi

run:
	docker-compose -f docker/chronicle.yaml up --force-recreate

.PHONY: stop
stop:
	docker-compose -f docker/chronicle.yaml down || true

$(MARKERS)/build:
	docker-compose -f docker-compose.yaml build
	touch $@

clean_containers:
	docker-compose -f docker/chronicle.yaml rm -f || true
	docker-compose -f docker-compose.yaml rm -f || true

clean_docker: stop
	docker-compose -f docker/chronicle.yaml down -v --rmi all || true
	docker-compose -f docker-compose.yaml down -v --rmi all || true

clean_target:
	rm -rf target
