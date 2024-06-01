provider "vault" {
  address      = "https://localhost:8200"
  token        = var.vault_token
  ca_cert_file = "${path.module}/ca_crt.pem"
}
