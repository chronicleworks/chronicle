#------------------------------------------------------------------------------
# Certificate Authority
#------------------------------------------------------------------------------
resource "tls_private_key" "ca" {
  count = local.generate_tls_certs ? 1 : 0

  algorithm   = "RSA"
  ecdsa_curve = "P384"
  rsa_bits    = "2048"
}



#------------------------------------------------------------------------------
# Certificate
#------------------------------------------------------------------------------
resource "tls_private_key" "vault_private_key" {
  count = local.generate_tls_certs ? 1 : 0

  algorithm   = "RSA"
  ecdsa_curve = "P384"
  rsa_bits    = "2048"
}

locals {
  dns_names = [for i in range(var.vault_replicas) : format("vault-%s.%s-internal", i, var.helm_chart_name)]
}


resource "tls_cert_request" "vault_cert_request" {
  count = local.generate_tls_certs ? 1 : 0

  private_key_pem = tls_private_key.vault_private_key[0].private_key_pem

  dns_names = concat(local.dns_names, [
    "*.vault.svc.${var.cluster_name}",
    "*.vault.svc",
    "*.vault"
  ])
  ip_addresses = ["127.0.0.1"]

  subject {
    common_name  = "system:node:*.${var.kubernetes_namespace}.svc.${var.cluster_name}"
    organization = "system:nodes"
  }
}

resource "kubernetes_certificate_signing_request_v1" "kubernetes_certificate_signing_request" {
  metadata {
    name = "${var.kubernetes_namespace}.vault.svc"
  }

  spec {
    usages      = ["key encipherment", "digital signature", "server auth"]
    signer_name = "kubernetes.io/kubelet-serving"

    request = tls_cert_request.vault_cert_request[0].cert_request_pem

  }

  auto_approve = false

}
