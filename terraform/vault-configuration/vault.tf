resource "vault_policy" "admin_policy" {
  name   = "admin"
  policy = file("policies/admin.hcl")
}

resource "vault_policy" "bootnode_policy" {
  name   = "chronicle-substrate"
  policy = file("policies/chronicle-substrate.hcl")
}

resource "vault_policy" "chronicle_policy" {
  name   = "chronicle"
  policy = file("policies/chronicle.hcl")
}


resource "vault_mount" "chronicle_substrate" {
  path = "kv/chronicle-substrate"
  type = "kv-v2"

  description = "KV2 Secrets Engine for Chronicle Substrate."
}

resource "vault_mount" "chronicle" {
  path = "kv/chronicle"
  type = "kv-v2"

  description = "KV2 Secrets Engine for Chronicle."
}

resource "vault_kv_secret_backend_v2" "chronicle_substrate_backend" {
  mount        = vault_mount.chronicle_substrate.path
  cas_required = false
}

resource "vault_kv_secret_backend_v2" "chronicle_backend" {
  mount        = vault_mount.chronicle.path
  cas_required = false
}

resource "vault_kv_secret_v2" "bootnode_key" {
  name  = "bootnode_key"
  mount = vault_mount.chronicle_substrate.path
  data_json = jsonencode({
    "key" = var.bootnode_key
  })

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}


resource "vault_kv_secret_v2" "bootnode_peer" {
  name  = "bootnode_peer"
  mount = vault_mount.chronicle_substrate.path
  data_json = jsonencode({
    "peer_id" = var.bootnode_peer_id
  })


  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

resource "vault_kv_secret_v2" "aura" {
  name      = "aura"
  mount     = vault_mount.chronicle_substrate.path
  data_json = var.aura_key

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

resource "vault_kv_secret_v2" "grankey" {
  name      = "grankey"
  mount     = vault_mount.chronicle_substrate.path
  data_json = var.grankey_key

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

resource "vault_kv_secret_v2" "rootkey" {
  name      = "root"
  mount     = vault_mount.chronicle_substrate.path
  data_json = var.root_key

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

resource "vault_kv_secret_v2" "onlinekey" {
  name      = "online"
  mount     = vault_mount.chronicle_substrate.path
  data_json = var.online_key

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

resource "vault_kv_secret_v2" "chronicle_account_key" {
  name      = "chronicle-account"
  mount     = vault_mount.chronicle.path
  data_json = var.chronicle_account_key

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

resource "vault_kv_secret_v2" "chronicle_identity_key" {
  name      = "chronicle-identity"
  mount     = vault_mount.chronicle.path
  data_json = var.chronicle_identity_key

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

resource "vault_auth_backend" "kubernetes" {
  type = "kubernetes"
}

resource "vault_kubernetes_auth_backend_config" "kubernetes_config" {
  backend                = vault_auth_backend.kubernetes.id
  kubernetes_host        = var.k8s_host
  kubernetes_ca_cert     = file("${path.module}/ca_crt.pem")
  token_reviewer_jwt     = var.sa_jwt_token

  depends_on = [vault_auth_backend.kubernetes]
}

resource "vault_kubernetes_auth_backend_role" "admin" {

  backend = vault_auth_backend.kubernetes.id

  role_name                        = "admin"
  bound_service_account_names      = ["*"]
  bound_service_account_namespaces = ["vault", "chronicle", "chronicle-substrate"]
  token_ttl                        = 3600
  token_policies                   = ["admin"]

    depends_on = [vault_auth_backend.kubernetes]
}

resource "vault_kubernetes_auth_backend_role" "chronicle-substrate" {
  backend = vault_auth_backend.kubernetes.id

  role_name                        = "chronicle-substrate"
  bound_service_account_names      = ["*"]
  bound_service_account_namespaces = ["vault", "chronicle", "chronicle-substrate"]
  token_ttl                        = 3600
  token_policies                   = []

    depends_on = [vault_auth_backend.kubernetes]
}

resource "vault_kubernetes_auth_backend_role" "chronicle" {
  backend = vault_auth_backend.kubernetes.id

  role_name                        = "chronicle"
  bound_service_account_names      = ["*"]
  bound_service_account_namespaces = ["vault", "chronicle", "chronicle-substrate"]
  token_ttl                        = 3600
  token_policies                   = []

    depends_on = [vault_auth_backend.kubernetes]
}

resource "vault_kubernetes_auth_backend_role" "chronicle-substrate" {
  backend = vault_auth_backend.kubernetes.id

  role_name                        = "chronicle-substrate"
  bound_service_account_names      = ["*"]
  bound_service_account_namespaces = ["vault"]
  token_ttl                        = 3600
  token_policies                   = []

    depends_on = [vault_auth_backend.kubernetes]
}

