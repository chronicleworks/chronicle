

resource "helm_release" "chronicle" {
  name       = "${var.helm_release_name}-chronicle"
  repository = "~/code/chronicle/charts/chronicle"
  chart      = "chronicle"
  namespace  = var.kubernetes_namespace

  
}

