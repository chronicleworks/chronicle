resource "helm_release" "signoz" {
  name       = "signoz"
  repository = var.signoz_helm_repository
  chart      = var.signoz_helm_chart_name
  namespace  = var.signoz_namespace
  create_namespace = true

  set {
    name  = "frontend.replicaCount"
    value = var.signoz_fe_replicas
  }

  set {
    name  = "queryService.replicaCount"
    value = var.signoz_query_replicas
  }

  set {
    name  = "storage.size"
    value = var.signoz_storage_size
  }
}

resource "helm_release" "opentelemetry_operator" {
  name       = "opentelemetry-operator"
  repository = var.opentelemetry_operator_helm_repository
  chart      = var.opentelemetry_operator_helm_chart_name
  namespace  = var.signoz_namespace
  create_namespace = true


  set {
    name = "manager.collectorImage.repository"
    value = "ghcr.io/open-telemetry/opentelemetry-collector-releases/opentelemetry-collector-contrib"
  }
}

resource "helm_release" "cert_manager" {
  name       = "cert-manager"
  repository = "https://charts.jetstack.io"
  chart      = "cert-manager"
  namespace  = "cert-manager"
  create_namespace = true
  version    = "v1.14.5"

  set {
    name  = "installCRDs"
    value = "true"
  }
}



variable "namespaces" {
  type        = list(string)
  description = "List of Kubernetes namespaces to deploy OpenTelemetry Collector into"
  default     = ["chronicle-substrate","chronicle","vault"]
}

