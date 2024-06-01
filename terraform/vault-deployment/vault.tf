#------------------------------------------------------------------------------
# Kubernetes resources
#------------------------------------------------------------------------------
resource "kubernetes_namespace" "vault" {
  metadata {
    name = var.kubernetes_namespace
  }
}


resource "kubernetes_secret" "tls" {
  metadata {
    name      = "tls"
    namespace = kubernetes_namespace.vault.metadata.0.name
  }

  data = {
    "tls.crt" =  kubernetes_certificate_signing_request_v1.kubernetes_certificate_signing_request.certificate
    "tls.key" =  tls_private_key.vault_private_key[0].private_key_pem
  }

  type = "kubernetes.io/tls"
}

resource "kubernetes_secret" "tls_ca" {
  metadata {
    name      = "tls-ca"
    namespace = kubernetes_namespace.vault.metadata.0.name
  }

  data = {
    "ca.crt"  = file("ca_crt.pem")
  }
}

resource "kubernetes_service_account" "vault" {
  metadata {
    name        = var.kubernetes_sa_name
    namespace   = var.kubernetes_namespace
  }
}


#------------------------------------------------------------------------------
# We need to be able to get API tokens for the vault service account,
# Long lived tokens will cause problems with the vault agent
#------------------------------------------------------------------------------

resource "kubernetes_secret" "vault_auth_secret" {
  metadata {
    name = "vault-auth-secret"
    namespace = var.kubernetes_namespace
    annotations = {
      "kubernetes.io/service-account.name" = kubernetes_service_account.vault.metadata.0.name
    }
  }

  type = "kubernetes.io/service-account-token"
}



#------------------------------------------------------------------------------
# Vault deployment
#------------------------------------------------------------------------------
resource "helm_release" "vault" {
  name       = var.helm_release_name
  repository = var.helm_repository
  chart      = var.helm_chart_name
  namespace  = var.kubernetes_namespace

  values = [
    templatefile("templates/values.tmpl", {
      helm_release_name = var.helm_release_name

      kubernetes_namespace                          = var.kubernetes_namespace
      kubernetes_vault_service_account              = kubernetes_service_account.vault.metadata.0.name
      kubernetes_secret_name_tls_cert               = kubernetes_secret.tls.metadata.0.name
      kubernetes_secret_name_tls_ca                 = kubernetes_secret.tls_ca.metadata.0.name
      kubernetes_image_pull_secrets                 = var.kubernetes_image_pull_secrets
      kubernetes_extra_secret_environment_variables = var.kubernetes_extra_secret_environment_variables
      kubernetes_vault_server_service_type          = var.kubernetes_vault_server_service_type
      kubernetes_vault_ui_service_type              = var.kubernetes_vault_ui_service_type

      vault_replica_count             = var.vault_replicas
      vault_injector_enable           = var.vault_injector_enable
      vault_injector_image_repository = var.vault_injector_image_repository
      vault_injector_image_tag        = "${var.vault_injector_image_tag}"
      vault_image_repository          = var.vault_image_repository
      vault_image_tag                 = "${var.vault_image_tag}"
      vault_data_storage_size         = var.vault_data_storage_size
      vault_leader_tls_servername     = "vault.svc"
      vault_seal_method               = var.vault_seal_method
      single_node_deployment          = var.single_node_deployment
    })
  ]
}
