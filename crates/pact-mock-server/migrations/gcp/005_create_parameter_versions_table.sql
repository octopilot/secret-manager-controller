-- Create GCP parameter_versions table
CREATE TABLE IF NOT EXISTS gcp.parameter_versions (
    parameter_key TEXT NOT NULL,
    version_id TEXT NOT NULL,
    data JSONB NOT NULL,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    PRIMARY KEY (parameter_key, version_id),
    FOREIGN KEY (parameter_key) REFERENCES gcp.parameters(key) ON DELETE CASCADE
);

