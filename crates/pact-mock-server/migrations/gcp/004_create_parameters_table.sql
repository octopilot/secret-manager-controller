-- Create GCP parameters table
CREATE TABLE IF NOT EXISTS gcp.parameters (
    key TEXT PRIMARY KEY,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
);

