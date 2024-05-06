variable "node_service_account" {
  description = "The service account for the Chronicle Substrate node."
  type        = string
  sensitive = false
  default = "bootnode"
}

variable "vault_token" {
  description = "A vault access token with root permission"
  type        = string
  sensitive = true
  default = "s.VL1OYyHeeQ1Qxyg9FqSyKjBe"
}

variable "bootnode_key" {
  description = "The bootnode key for the Chronicle Substrate node."
  type        = string
  sensitive = true
}

variable "aura_key" {
  description = "The aura key for the Chronicle Substrate node."
  type        = string
  default = <<EOF
{
  "accountId": "0x68c802830f90c7066f18f68d3c0e28357d8c9573bee80284a6ca3daf056dc816",
  "networkId": "substrate",
  "publicKey": "0x68c802830f90c7066f18f68d3c0e28357d8c9573bee80284a6ca3daf056dc816",
  "secretPhrase": "menu tenant guard pioneer affair pumpkin snake snake capable hand token crisp",
  "secretSeed": "0xb8c1b2d5f6fa96e05da9dd4d92139162fcee4ae267f5d1c04442b517e71a5049",
  "ss58Address": "5ES6FbeMbeu6vjdPzj9NZHUVAPbSRY9TWbam3VY1b5jJjVNM",
  "ss58PublicKey": "5ES6FbeMbeu6vjdPzj9NZHUVAPbSRY9TWbam3VY1b5jJjVNM"
}
EOF
}

variable bootnode_peer_id {
  description = "The bootnode peer ID for the Chronicle Substrate node."
  type        = string
  sensitive = false
}

variable "vault_service_ca" {
  description = "A path to the CA certificate for the Vault service."
  type        = string
  default = "/Users/ryan/code/chronicle/terraform/vault-deployment/ca.crt"
}

