SHELL := /bin/bash

KUBERNETES_SERVICE_HOST := kubernetes.default.svc
KUBERNETES_SERVICE_PORT := "$(kubectl get svc kubernetes -n default -o jsonpath='{.spec.ports[0].port}')"

ca:
	kubectl config view --raw --minify --flatten --output 'jsonpath={.clusters[].cluster.certificate-authority-data}' | base64 --decode > ca_crt.pem

apply: ca
	terraform apply

.ONESHELL:
unseal: apply
	@retry=0; \
	while [[ $$retry -lt 5 ]]; do \
		output=$$(kubectl exec vault-0 --namespace vault -- vault operator init -key-shares=1 -key-threshold=1); \
		if [[ $$? -eq 0 ]]; then \
			unseal_keys=$$(echo "$$output" | awk '/Unseal Key/ {print $$NF}'); \
			root_token=$$(echo "$$output" | awk '/Initial Root Token/ {print $$NF}'); \
			break; \
		fi; \
		((retry++)); \
		echo "Retrying in 3 seconds..."; \
		sleep 3; \
	done; \
	num_pods=$$(kubectl get pods --namespace vault -l app.kubernetes.io/name=vault -o jsonpath='{.items[*].metadata.name}' | wc -w); \
	for ((i=0; i<$$num_pods; i++)); do \
		pod_name="vault-$$i"; \
		for key in $$unseal_keys; do \
			retry_key=0; \
			while [[ $$retry_key -lt 5 ]]; do \
				kubectl exec $$pod_name --namespace vault -- vault operator unseal $$key; \
				if [[ $$? -eq 0 ]]; then \
					break; \
				fi; \
				((retry_key++)); \
				echo "Retrying unseal for $$pod_name in 3 seconds..."; \
				sleep 3; \
			done; \
		done; \
	done; \
	echo "Unseal Keys: $$unseal_keys"; \
	echo "Root Token: $$root_token"

