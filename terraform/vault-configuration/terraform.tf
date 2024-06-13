terraform {
  backend "kubernetes" {
    secret_suffix = "vault-config"
    config_path   = "~/.kube/config"
  }
}
