variable "chronicle-node-repository" {
    type = string
    description = "The repository to pull the chronicle node image from"
    default = "node-chronicle-arm64"
}

variable "kubernetes_namespace" {
  type        = string
  description = "The Kubernetes namespace to deploy Vault into"
  default     = "chronicle-substrate"
}

variable "kubernetes_image_pull_secrets" {
  type        = list(string)
  description = "A list of Kubernetes secrets that hold any required image registry credentials"
  default     = null
}

variable "kubernetes_extra_secret_environment_variables" {
  type        = list(map(string))
  description = "A list of maps referencing Kubernetes secrets and their environment variable to mount to the Vault pods"
  default     = null
}

variable "helm_release_name" {
  type        = string
  description = "The name of the Helm release"
  default     = "chronicle-substrate"
}

variable "helm_chart_name" {
  type        = string
  description = "The chart name in the Helm repository"
  default     = "node"
}

variable "helm_repository" {
  type        = string
  description = "The location of the Helm repository"
  default     = "https://paritytech.github.io/helm-charts/"
}

variable "vault_replicas" {
  type        = number
  description = "The number of Vault replicas to deploy"
  default     = 3
}
