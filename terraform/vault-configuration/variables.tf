variable "node_service_account" {
  description = "The service account for the Chronicle Substrate node."
  type        = string
  sensitive   = false
  default     = "vault-sa"
}

variable "vault_token" {
  description = "A vault access token with root permission"
  type        = string
  sensitive   = true
}

variable "bootnode_key" {
  description = "The bootnode key for the Chronicle Substrate node."
  type        = string
  sensitive   = true
}

variable "aura_key" {
  description = "The aura key for the Chronicle Substrate node."
  type        = string
  sensitive   = true
}

variable "grankey_key" {
  description = "The grankey key for the Chronicle Substrate node."
  type        = string
  sensitive   = true
}

variable "bootnode_peer_id" {
  description = "The bootnode peer ID for the Chronicle Substrate node."
  type        = string
  sensitive   = true
}

variable "chronicle_repository" {
  description = "The repository for the Chronicle container image."
  type        = string
  default     = "node-chronicle-arm64"
}

variable "chronicle_tag" {
  description = "The tag for the Chronicle container image."
  type        = string
  default     = "local"
}
variable "sa_jwt_token" {
  description = "The JWT token for the service account."
  type        = string
  sensitive   = false
}

variable "k8s_host" {
  description = "The Kubernetes host URL."
  type        = string
  sensitive   = false
}

