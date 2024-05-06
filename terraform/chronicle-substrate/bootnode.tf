#------------------------------------------------------------------------------
# Bootnode  deployment
#------------------------------------------------------------------------------
resource "helm_release" "bootnode" {
  name       = var.helm_release_name
  repository = "https://paritytech.github.io/helm-charts/"
  chart      = "node"
  namespace  = var.kubernetes_namespace

  values = [
    templatefile("templates/bootnode.tmpl", {
      helm_release_name = var.helm_release_name

      kubernetes_namespace                          = var.kubernetes_namespace
      kubernetes_image_pull_secrets                 = var.kubernetes_image_pull_secrets

      chronicle_node_repository = var.chronicle-node-repository
      bootnode_key = var.bootnode-key

      node = {
        command = "node-chronicle"
      }
    })
  ]
}
