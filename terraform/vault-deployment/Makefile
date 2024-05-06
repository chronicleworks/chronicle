
get-ca:
	kubectl config view \
		--raw \
		--minify \
		--flatten \
		-o jsonpath='{.clusters[].cluster.certificate-authority-data}' \
		| base64 -d > ca.crt

apply: get-ca
	terraform apply

unseal: apply
	kubectl exec --stdin=true --tty=true vault-0 --namespace vault  -- vault operator init -key-shares=1 -key-threshold=1

