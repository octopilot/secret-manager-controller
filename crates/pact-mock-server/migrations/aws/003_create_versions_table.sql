-- Create AWS versions table
CREATE TABLE IF NOT EXISTS aws.versions (
    secret_name TEXT NOT NULL,
    version_id TEXT NOT NULL,
    data JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    PRIMARY KEY (secret_name, version_id),
    FOREIGN KEY (secret_name) REFERENCES aws.secrets(name) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_aws_versions_secret_name ON aws.versions(secret_name);

