# Kubernetes variables
variable "cluster_name" {
  type = string
  description = "The kubernetes cluster name"
  default = "cluster.local"
}

variable "kubernetes_namespace" {
  type        = string
  description = "The Kubernetes namespace to deploy Vault into"
  default     = "vault"
}

variable "kubernetes_sa_name" {
  type        = string
  description = "The Kubernetes Service Account that Vault will use"
  default     = "vault-sa"
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

variable "kubernetes_vault_server_service_type" {
  type        = string
  description = "The kubernetes service type for the Vault service"
  default     = "ClusterIP"
}

variable "kubernetes_vault_ui_service_type" {
  type        = string
  description = "The kubernetes service type for the Vault UI"
  default     = "ClusterIP"
}

variable "helm_release_name" {
  type        = string
  description = "The name of the Helm release"
  default     = "vault"
}

variable "helm_chart_name" {
  type        = string
  description = "The chart name in the Helm repository"
  default     = "vault"
}

variable "helm_repository" {
  type        = string
  description = "The location of the Helm repository"
  default     = "https://helm.releases.hashicorp.com/"
}

variable "vault_replicas" {
  type        = number
  description = "The number of Vault replicas to deploy"
  default     = 1
}

variable "single_node_deployment" {
  type        = bool
  description = "Set this if running in a single node environment like minikube or docker desktop"
  default     = true
}

variable "vault_injector_enable" {
  type        = bool
  description = "Whether or not to enable the Vault agent injector"
  default     = true
}

variable "vault_injector_image_repository" {
  type        = string
  description = "The repository to pull the Vault injector image from"
  default     = "hashicorp/vault-k8s"
}

variable "vault_injector_image_tag" {
  type        = string
  description = "The image tag to use when pulling the Vault injector image"
  default     = "1.4"
}

variable "vault_image_repository" {
  type        = string
  description = "The repository to pull the Vault image from"
  default     = "hashicorp/vault"
}

variable "vault_image_tag" {
  type        = string
  description = "The image tag to use when pulling the Vault image"
  default     = "1.16"
}

variable "vault_data_storage_size" {
  type        = string
  description = "The size, in Gi, of the data storage volume"
  default     = "10"
}

variable "vault_ui" {
  type        = bool
  description = "Enable the Vault UI"
  default     = true
}

variable "vault_seal_method" {
  type        = string
  description = "The Vault seal method to use"
  default     = "shamir"
}

variable "vault_enable_audit" {
  type        = bool
  description = "Enables Vault audit storage"
  default     = true
}
