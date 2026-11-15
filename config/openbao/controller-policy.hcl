# Least-privilege OpenBao policy for the Secret Manager Controller.
# Grants only the KV v2 paths the provider needs on the `secret` mount.
# Adjust the mount name if you use a different KV v2 mount.
path "secret/data/*" {
  capabilities = ["create", "update", "read"]
}

# KV v2 soft-delete (disable_secret) and undelete (enable_secret).
path "secret/delete/*" {
  capabilities = ["update"]
}
path "secret/undelete/*" {
  capabilities = ["update"]
}

# Version metadata: read latest version for enable_secret; delete removes all
# versions (delete_secret); list for diff discovery.
path "secret/metadata/*" {
  capabilities = ["read", "list", "delete"]
}
