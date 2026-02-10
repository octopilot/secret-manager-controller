-- Create Azure app_config table
CREATE TABLE IF NOT EXISTS azure.app_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    content_type TEXT,
    label TEXT,
    tags JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
);

