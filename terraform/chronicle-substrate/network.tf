#------------------------------------------------------------------------------
# Bootnode  deployment
#------------------------------------------------------------------------------
resource "helm_release" "bootnode" {
  name       = "${var.helm_release_name}-bootnode"
  repository = "https://paritytech.github.io/helm-charts/"
  chart      = "node"
  namespace  = var.kubernetes_namespace

  values = [
    templatefile("templates/bootnode.tmpl", {
      helm_release_name = var.helm_release_name

      kubernetes_namespace                          = var.kubernetes_namespace
      kubernetes_image_pull_secrets                 = var.kubernetes_image_pull_secrets

      chronicle_node_repository = var.chronicle-node-repository

      node = {
        command = "node-chronicle"
        tracing =  {
          enabled = true
        }
      }
    })
  ]
}

resource "helm_release" "validators" {
  name       = "${var.helm_release_name}-validators"
  repository = "https://paritytech.github.io/helm-charts/"
  chart      = "node"
  namespace  = var.kubernetes_namespace

  values = [
    templatefile("templates/validators.tmpl", {
      helm_release_name = var.helm_release_name

      kubernetes_namespace                          = var.kubernetes_namespace
      kubernetes_image_pull_secrets                 = var.kubernetes_image_pull_secrets

      chronicle_node_repository = var.chronicle-node-repository

      node = {
        command = "node-chronicle"
        tracing =  {
          enabled = true
        }
      }
    })
  ]

  depends_on = [helm_release.bootnode]
}


resource "kubernetes_manifest" "bootnode_service_monitor" {
  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "ServiceMonitor"
    metadata = {
      name      = "bootnode-service-monitor"
      namespace = var.kubernetes_namespace
      labels = {
        release = "prometheus-operator"
      }
    }
    spec = {
      selector = {
        matchLabels = {
          component = "substrate-node"
        }
      }
      namespaceSelector = {
        matchNames = [var.kubernetes_namespace]
      }
      endpoints = [
        {
          port     = 9625
          interval = "30s"
        }
      ]
    }
  }
}

