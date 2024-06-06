# Substrate needs access to its own account keys and chronicle's account keys
# Access could be reduced to only chronicle's public keys, but this involves additional configuration complexity
# as that must then be set via RPC rather than relying on the agent injector

path "kv/chronicle-substrate/*"
{
  capabilities = ["create", "read", "update", "delete", "list"]
}

path "kv/chronicle/*"
{
  capabilities = ["create", "read", "update", "delete", "list"]
}
