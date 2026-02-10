-- Create Azure app_config_versions table
CREATE TABLE IF NOT EXISTS azure.app_config_versions (
    config_key TEXT NOT NULL,
    version_id TEXT NOT NULL,
    value TEXT NOT NULL,
    content_type TEXT,
    label TEXT,
    tags JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    PRIMARY KEY (config_key, version_id),
    FOREIGN KEY (config_key) REFERENCES azure.app_config(key) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_azure_app_config_versions_config_key ON azure.app_config_versions(config_key);

