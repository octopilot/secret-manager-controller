-- Insert test data for Azure Key Vault and App Configuration
-- Using environment 'mig' to differentiate from controller-inserted data

-- Insert Azure secret
INSERT INTO azure.secrets (name, disabled, metadata, environment, location, created_at, updated_at)
VALUES (
    'test-secret-mig',
    false,
    '{"tags": {"Environment": "mig", "Location": "eastus"}}'::jsonb,
    'mig',
    'eastus',
    EXTRACT(EPOCH FROM NOW())::BIGINT,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (name) DO NOTHING;

-- Insert Azure secret version
-- Azure stores secret data as JSON with value field (base64-encoded in API, but stored as plain string in JSON)
INSERT INTO azure.versions (secret_name, version_id, data, enabled, created_at)
VALUES (
    'test-secret-mig',
    'a1b2c3d4e5f6g7h8',
    '{"value": "dGVzdC1zZWNyZXQtdmFsdWU="}'::jsonb,
    true,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (secret_name, version_id) DO NOTHING;

-- Insert Azure app config
INSERT INTO azure.app_config (key, value, content_type, label, tags, environment, location, created_at, updated_at)
VALUES (
    'test-app-config-mig',
    'test-app-config-value',
    'text/plain',
    NULL,
    '{"Environment": "mig", "Location": "eastus"}'::jsonb,
    'mig',
    'eastus',
    EXTRACT(EPOCH FROM NOW())::BIGINT,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (key) DO NOTHING;

-- Insert Azure app config version
-- Azure app_config_versions stores value as TEXT, tags as JSONB
INSERT INTO azure.app_config_versions (config_key, version_id, value, content_type, label, tags, created_at)
VALUES (
    'test-app-config-mig',
    'v1234567890',
    'test-app-config-value',
    'text/plain',
    NULL,
    '{"Environment": "mig", "Location": "eastus"}'::jsonb,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (config_key, version_id) DO NOTHING;

