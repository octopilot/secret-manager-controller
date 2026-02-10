-- Insert test data for GCP Secret Manager and Parameter Manager
-- Using environment 'mig' to differentiate from controller-inserted data

-- Insert GCP secret
INSERT INTO gcp.secrets (key, disabled, metadata, environment, location, created_at, updated_at)
VALUES (
    'projects/test-project/secrets/test-secret-mig',
    false,
    '{"labels": {"environment": "mig", "location": "us-central1"}}'::jsonb,
    'mig',
    'us-central1',
    EXTRACT(EPOCH FROM NOW())::BIGINT,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (key) DO NOTHING;

-- Insert GCP secret version
-- Base64-encoded "test-secret-value" = dGVzdC1zZWNyZXQtdmFsdWU=
INSERT INTO gcp.versions (secret_key, version_id, data, enabled, created_at)
VALUES (
    'projects/test-project/secrets/test-secret-mig',
    '1',
    '{"payload": {"data": "dGVzdC1zZWNyZXQtdmFsdWU="}}'::jsonb,
    true,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (secret_key, version_id) DO NOTHING;

-- Insert GCP parameter
INSERT INTO gcp.parameters (key, metadata, environment, location, created_at, updated_at)
VALUES (
    'projects/test-project/locations/us-central1/parameters/test-param-mig',
    '{"labels": {"environment": "mig", "location": "us-central1"}}'::jsonb,
    'mig',
    'us-central1',
    EXTRACT(EPOCH FROM NOW())::BIGINT,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (key) DO NOTHING;

-- Insert GCP parameter version
-- Base64-encoded JSON: {"value": "test-param-value"}
-- The JSON string "{\"value\": \"test-param-value\"}" base64-encoded = eyJ2YWx1ZSI6ICJ0ZXN0LXBhcmFtLXZhbHVlIn0=
INSERT INTO gcp.parameter_versions (parameter_key, version_id, data, created_at)
VALUES (
    'projects/test-project/locations/us-central1/parameters/test-param-mig',
    '1',
    '{"data": "eyJ2YWx1ZSI6ICJ0ZXN0LXBhcmFtLXZhbHVlIn0="}'::jsonb,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (parameter_key, version_id) DO NOTHING;

