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
