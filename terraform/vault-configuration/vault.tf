resource "vault_policy" "admin_policy" {
  name   = "admins"
  policy = file("policies/admin.hcl")
}

resource "vault_policy" "bootnode_policy" {
  name   = "bootnode"
  policy = file("policies/bootnode.hcl")
}

resource "vault_policy" "chronicle_policy" {
  name   = "chronicle"
  policy = file("policies/chronicle.hcl")
}

resource "vault_mount" "chronicle_substrate" {
  path        = "kv/chronicle_substrate"
  type        = "kv-v2"

  description = "KV2 Secrets Engine for Chronicle Substrate."
}

resource "vault_mount" "chronicle" {
  path        = "kv/chronicle"
  type        = "kv-v2"

  description = "KV2 Secrets Engine for Chronicle."
}

resource "vault_kv_secret_backend_v2" "chronicle_substrate_backend" {
  mount                = vault_mount.chronicle_substrate.path
  cas_required         = false
}

resource "vault_kv_secret_backend_v2" "chronicle_backend" {
  mount                = vault_mount.chronicle.path
  cas_required         = false
}

resource "vault_kv_secret_v2" "bootnode_key" {
  name = "bootnode_key"
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
  name = "bootnode_peer"
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
  name = "aura"
  mount = vault_mount.chronicle_substrate.path
  data_json = var.aura_key

  #Once secrets are set, do not update
  lifecycle {
    ignore_changes = [data_json]
  }
}

