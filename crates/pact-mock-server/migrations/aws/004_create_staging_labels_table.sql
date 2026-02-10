-- Create AWS staging_labels table
CREATE TABLE IF NOT EXISTS aws.staging_labels (
    secret_name TEXT NOT NULL,
    label TEXT NOT NULL,
    version_id TEXT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    PRIMARY KEY (secret_name, label),
    FOREIGN KEY (secret_name) REFERENCES aws.secrets(name) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_aws_staging_labels_secret_name ON aws.staging_labels(secret_name);

