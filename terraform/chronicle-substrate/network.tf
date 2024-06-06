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
      }
    })
  ]

  depends_on = [helm_release.bootnode]
}


resource "helm_release" "rpcnodes" {
  name       = "${var.helm_release_name}-rpc"
  repository = "https://paritytech.github.io/helm-charts/"
  chart      = "node"
  namespace  = var.kubernetes_namespace

  values = [
    templatefile("templates/rpcnode.tmpl", {
      helm_release_name = var.helm_release_name

      kubernetes_namespace                          = var.kubernetes_namespace
      kubernetes_image_pull_secrets                 = var.kubernetes_image_pull_secrets

      chronicle_node_repository = var.chronicle-node-repository

      node = {
        command = "node-chronicle"
      }
    })
  ]

  depends_on = [helm_release.validators]
}


