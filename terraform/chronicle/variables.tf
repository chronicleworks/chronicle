variable "global_image_tag" {
  description = "Global image tag"
  type        = string
  default     = ""
}

variable "affinity" {
  description = "Custom affinity rules for the chronicle pod"
  type        = map(any)
  default     = {}
}

variable "auth_required" {
  description = "If true, require authentication, rejecting 'anonymous' requests"
  type        = bool
  default     = false
}

variable "auth_id_claims" {
  description = "Chronicle provides default values ['iss', 'sub']"
  type        = list(string)
  default     = []
}

variable "auth_jwks_url" {
  description = "URL for JWKS"
  type        = string
  default     = ""
}

variable "auth_userinfo_url" {
  description = "URL for userinfo"
  type        = string
  default     = ""
}

variable "backtrace_level" {
  description = "Backtrace level for Chronicle"
  type        = string
  default     = "full"
}

variable "dev_id_provider_enabled" {
  description = "Enable the devIdProvider"
  type        = bool
  default     = true
}

variable "dev_id_provider_image_pull_policy" {
  description = "The image pull policy for the id-provider container"
  type        = string
  default     = "IfNotPresent"
}

variable "dev_id_provider_image_repository" {
  description = "The image repository for the id-provider container"
  type        = string
  default     = "blockchaintp/id-provider-amd64"
}

variable "dev_id_provider_image_tag" {
  description = "The image tag for the id-provider container"
  type        = string
  default     = "BTP2.1.0-0.7.4"
}

variable "opa_enabled" {
  description = "Enable OPA"
  type        = bool
  default     = false
}

variable "endpoints_arrow_enabled" {
  description = "Enable arrow endpoint"
  type        = bool
  default     = true
}

variable "endpoints_data_enabled" {
  description = "Enable data endpoint"
  type        = bool
  default     = true
}

variable "endpoints_graphql_enabled" {
  description = "Enable GraphQL endpoint"
  type        = bool
  default     = true
}

variable "extra_volumes" {
  description = "A list of additional volumes to add to chronicle"
  type        = list(any)
  default     = []
}

variable "extra_volume_mounts" {
  description = "A list of additional volume mounts to add to chronicle"
  type        = list(any)
  default     = []
}

variable "health_check_enabled" {
  description = "Enable the liveness depth charge check"
  type        = bool
  default     = false
}

variable "health_check_interval" {
  description = "The interval between health checks"
  type        = string
  default     = "1800"
}

variable "image_repository" {
  description = "The repository of the image"
  type        = string
  default     = "blockchaintp/chronicle-amd64"
}

variable "image_tag" {
  description = "The tag of the image to use"
  type        = string
  default     = "BTP2.1.0-0.7.4"
}

variable "image_pull_policy" {
  description = "The image pull policy to use"
  type        = string
  default     = "IfNotPresent"
}

variable "image_pull_secrets_enabled" {
  description = "Use the list of named imagePullSecrets"
  type        = bool
  default     = false
}

variable "image_pull_secrets_value" {
  description = "A list of named secret references"
  type        = list(any)
  default     = []
}

variable "ingress_api_version" {
  description = "The apiVersion of the ingress"
  type        = string
  default     = ""
}

variable "ingress_enabled" {
  description = "Enable the ingress to the main service rest-api"
  type        = bool
  default     = false
}

variable "ingress_cert_manager" {
  description = "Enable the acme certmanager for this ingress"
  type        = bool
  default     = false
}

variable "ingress_hostname" {
  description = "Primary hostname for the ingress"
  type        = string
  default     = ""
}

variable "ingress_path" {
  description = "Path for the ingress's primary hostname"
  type        = string
  default     = "/"
}

variable "ingress_path_type" {
  description = "PathType for the ingress's primary hostname"
  type        = string
  default     = ""
}

variable "ingress_annotations" {
  description = "Annotations for the ingress"
  type        = map(any)
  default     = {}
}

variable "ingress_tls" {
  description = "Enable TLS on the ingress with a secret at hostname-tls"
  type        = bool
  default     = false
}

variable "ingress_extra_hosts" {
  description = "List of extra hosts to add to the ingress"
  type        = list(any)
  default     = []
}

variable "ingress_extra_paths" {
  description = "List of extra paths to add to the primary host of the ingress"
  type        = list(any)
  default     = []
}

variable "ingress_extra_tls" {
  description = "List of extra TLS entries"
  type        = list(any)
  default     = []
}

variable "ingress_hosts" {
  description = "List of ingress host and path declarations for the chronicle ingress"
  type        = list(any)
  default     = []
}

variable "log_level" {
  description = "Log level for Chronicle"
  type        = string
  default     = "info"
}

variable "port" {
  description = "The port on which the chronicle service listens"
  type        = number
  default     = 9982
}

variable "replicas" {
  description = "Number of Chronicle replicas to run"
  type        = number
  default     = 1
}

variable "service_account_create" {
  description = "Create a service account"
  type        = bool
  default     = true
}

variable "service_account_name" {
  description = "Name of the service account"
  type        = string
  default     = ""
}

variable "test_api_enabled" {
  description = "Enable api-test Jobs and Services"
  type        = bool
  default     = true
}

variable "test_api_image_pull_policy" {
  description = "The image pull policy for the api-test container"
  type        = string
  default     = "IfNotPresent"
}

variable "test_api_image_repository" {
  description = "The image repository for the api-test container"
  type        = string
  default     = "blockchaintp/chronicle-helm-api-test-amd64"
}

variable "test_api_image_tag" {
  description = "The image tag for the api-test container"
  type        = string
  default     = "BTP2.1.0-0.7.4"
}

variable "test_auth_enabled" {
  description = "Enable auth-related testing"
  type        = bool
  default     = true
}

variable "test_auth_token" {
  description = "Provide a token for auth-related testing"
  type        = string
  default     = ""
}

variable "postgres_enabled" {
  description = "Create an internal postgres instance"
  type        = bool
  default     = true
}

variable "postgres_env" {
  description = "Postgres environment variables"
  type        = map(any)
  default     = {}
}

variable "postgres_image_registry" {
  description = "Postgres image registry"
  type        = string
  default     = ""
}

variable "postgres_image_repository" {
  description = "Postgres image repository"
  type        = string
  default     = "postgres"
}

variable "postgres_image_tag" {
  description = "Postgres image tag"
  type        = string
  default     = "11"
}

variable "postgres_user" {
  description = "User for the postgres database"
  type        = string
  default     = "postgres"
}

variable "postgres_host" {
  description = "Host for the postgres database"
  type        = string
  default     = "localhost"
}

variable "postgres_database" {
  description = "Database for the postgres database"
  type        = string
  default     = "postgres"
}

variable "postgres_port" {
  description = "Port for the postgres database"
  type        = number
  default     = 5432
}

variable "postgres_password" {
  description = "Password for the postgres database"
  type        = string
  default     = "postgres"
}

variable "postgres_existing_password_secret" {
  description = "Name of a secret containing the postgres password"
  type        = string
  default     = ""
}

variable "postgres_existing_password_secret_key" {
  description = "Name of the key in a secret containing the postgres password"
  type        = string
  default     = ""
}

variable "postgres_tls" {
  description = "Postgres TLS configuration"
  type        = string
  default     = ""
}

variable "postgres_persistence_enabled" {
  description = "Allocate a PVC for the postgres instance"
  type        = bool
  default     = false
}

variable "postgres_persistence_annotations" {
  description = "Custom annotations to the postgres PVC's"
  type        = map(any)
  default     = {}
}

variable "postgres_persistence_access_modes" {
  description = "Postgres PVC access modes"
  type        = list(string)
  default     = ["ReadWriteOnce"]
}

variable "postgres_persistence_storage_class" {
  description = "Postgres PVC storageClass"
  type        = string
  default     = ""
}

variable "postgres_persistence_size" {
  description = "Postgres PVC volume size"
  type        = string
  default     = "40Gi"
}

variable "postgres_resources" {
  description = "Resources for postgres"
  type        = map(any)
  default     = {}
}

variable "resources" {
  description = "Resources for chronicle"
  type        = map(any)
  default     = {}
}

variable "volumes" {
  description = "Volumes for chronicle"
  type        = map(any)
  default     = {}
}

variable "node_vault_keys" {
  description = "Vault keys for the node"
  type        = list(map(string))
  default     = [
    {
      name           = "aura",
      type           = "aura",
      scheme         = "secp256k1",
      vaultPath      = "kv/secret/chronicle-node",
      vaultKey       = "aura",
      extraDerivation = "//${HOSTNAME}//aura"
    }
  ]
}

variable "node_vault_node_key" {
  description = "Node key for the vault"
  type        = map(string)
  default     = {
    name      = "bootnode-key",
    vaultPath = "kv/secret/chronicle-node",
    vaultKey  = "bootnode-key"
  }
}

variable "node_image_pull_policy" {
  description = "The image pull policy for the node"
  type        = string
  default     = "IfNotPresent"
}

variable "node_image_repository" {
  description = "The image repository for the node"
  type        = string
  default     = "blockchaintp/chronicle-node-amd64"
}

variable "node_image_tag" {
  description = "The image tag for the node"
  type        = string
  default     = "BTP2.1.0-0.8.0"
}

variable "node_chain" {
  description = "Chain for the node"
  type        = string
  default     = "chronicle"
}

variable "node_command" {
  description = "Command for the node"
  type        = string
  default     = "node-chronicle"
}


variable "helm_release_name" {
  description = "Name of the helm release"
  type        = string
  default     = "chronicle"
}


variable "kubernetes_namespace" {
  description = "Namespace for the helm release"
  type        = string
  default     = "chronicle-substrate"
}

