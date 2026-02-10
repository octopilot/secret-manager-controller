-- Create AWS parameter_versions table
CREATE TABLE IF NOT EXISTS aws.parameter_versions (
    parameter_name TEXT NOT NULL,
    version_id TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    PRIMARY KEY (parameter_name, version_id),
    FOREIGN KEY (parameter_name) REFERENCES aws.parameters(name) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_aws_parameter_versions_parameter_name ON aws.parameter_versions(parameter_name);

