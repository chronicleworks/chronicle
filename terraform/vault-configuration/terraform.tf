terraform {
  backend "kubernetes" {
    secret_suffix = "vault-config-state"
    config_path   = "~/.kube/config"
  }
}
