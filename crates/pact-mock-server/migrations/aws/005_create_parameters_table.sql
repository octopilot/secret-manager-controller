-- Create AWS parameters table
CREATE TABLE IF NOT EXISTS aws.parameters (
    name TEXT PRIMARY KEY,
    parameter_type TEXT NOT NULL DEFAULT 'String',
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
);

