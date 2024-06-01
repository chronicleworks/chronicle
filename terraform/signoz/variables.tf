variable "signoz_namespace" {
  type        = string
  description = "The Kubernetes namespace to deploy SigNoz into"
  default     = "observabilty"
}

variable "signoz_helm_chart_name" {
  type        = string
  description = "The chart name in the Helm repository for SigNoz"
  default     = "signoz"
}

variable "signoz_helm_repository" {
  type        = string
  description = "The location of the Helm repository for SigNoz"
  default     = "https://charts.signoz.io/"
}

variable "signoz_fe_replicas" {
  type        = number
  description = "The number of SigNoz replicas to deploy"
  default     = 1
}


variable "signoz_query_replicas" {
  type        = number
  description = "The number of SigNoz replicas to deploy"
  default     = 1
}

variable "signoz_storage_size" {
  type        = string
  description = "The size, in Gi, of the storage volume for SigNoz"
  default     = "20"
}

variable "opentelemetry_operator_helm_chart_name" {
  type        = string
  description = "The chart name in the Helm repository for OpenTelemetry Collector"
  default     = "opentelemetry-operator"
}

variable "opentelemetry_operator_helm_repository" {
  type        = string
  description = "The location of the Helm repository for OpenTelemetry Collector"
  default     = "https://open-telemetry.github.io/opentelemetry-helm-charts"
}

