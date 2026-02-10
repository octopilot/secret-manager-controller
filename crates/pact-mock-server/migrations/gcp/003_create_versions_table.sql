-- Create GCP versions table
CREATE TABLE IF NOT EXISTS gcp.versions (
    secret_key TEXT NOT NULL,
    version_id TEXT NOT NULL,
    data JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    PRIMARY KEY (secret_key, version_id),
    FOREIGN KEY (secret_key) REFERENCES gcp.secrets(key) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gcp_versions_secret_key ON gcp.versions(secret_key);
CREATE INDEX IF NOT EXISTS idx_gcp_versions_created_at ON gcp.versions(secret_key, created_at);

