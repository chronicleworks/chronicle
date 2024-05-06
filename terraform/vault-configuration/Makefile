TF_VAR_bootnode_key := $(shell docker run --platform=linux/amd64 parity/polkadot key generate-node-key 2>/dev/null)
export TF_VAR_bootnode_key

TF_VAR_bootnode_peer_id := $(shell echo $(TF_VAR_bootnode_key) | docker run --platform=linux/amd64 -i parity/polkadot key inspect-node-key)
export TF_VAR_bootnode_peer_id

TF_VAR_aura_key := $(shell docker run --platform=linux/amd64 parity/polkadot key generate --network substrate --output-type json)
export TF_VAR_aura_key

apply:
	VAULT_TLS_SERVER_NAME=vault-0.vault-internal terraform apply
