KEYGEN_IMAGE := node-chronicle-arm64
KEYGEN_TAG := local

TF_VAR_aura_key := $(shell docker run $(KEYGEN_IMAGE):$(KEYGEN_TAG) node-chronicle key generate --network substrate --output-type json)
export TF_VAR_aura_key

TF_VAR_grankey_key := $(shell docker run $(KEYGEN_IMAGE):$(KEYGEN_TAG) node-chronicle key generate --network substrate --output-type json)
export TF_VAR_grankey_key

TF_VAR_bootnode_key := $(shell docker run $(KEYGEN_IMAGE):$(KEYGEN_TAG) node-chronicle key generate-node-key)
export TF_VAR_bootnode_key

TF_VAR_bootnode_peer_id := $(shell echo $(TF_VAR_bootnode_key) | docker run -i $(KEYGEN_IMAGE):$(KEYGEN_TAG) node-chronicle key inspect-node-key)
export TF_VAR_bootnode_peer_id

TF_VAR_sa_jwt_token := $(shell kubectl get secret vault-auth-secret --namespace vault --output 'go-template={{ .data.token }}' | base64 --decode)
export TF_VAR_sa_jwt_token

TF_VAR_k8s_host := https://kubernetes.default.svc
export TF_VAR_k8s_host

ca:
	kubectl config view --raw --minify --flatten --output 'jsonpath={.clusters[].cluster.certificate-authority-data}' | base64 --decode > ca_crt.pem

format:
	@terraform fmt \
		-write=true \
		-recursive

# https://github.com/terraform-linters/tflint
lint: format ## Check for possible errors, best practices, etc in current directory!
	@tflint

# https://github.com/liamg/tfsec
check-security: format ## Static analysis of your terraform templates to spot potential security issues.
	@tfsec .

documentation: lint check-security ## Generate README.md for a module
	@terraform-docs \
		markdown table \
		--sort-by-required  . > README.md

apply: ca
	kubectl port-forward service/vault-ui 8200:8200 --namespace vault &> /dev/null & \
	VAULT_TLS_SERVER_NAME=vault-0.vault-internal terraform apply;
	@kill $$! # Stop the port-forwarding process

destroy:
	kubectl port-forward service/vault-ui 8200:8200 --namespace vault &> /dev/null & \
	VAULT_TLS_SERVER_NAME=vault-0.vault-internal terraform destroy;
	@kill $$! # Stop the port-forwarding proces

