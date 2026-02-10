-- Insert test data for AWS Secrets Manager and Parameter Store
-- Using environment 'mig' to differentiate from controller-inserted data

-- Insert AWS secret
INSERT INTO aws.secrets (name, disabled, metadata, environment, location, created_at, updated_at)
VALUES (
    'test-secret-mig',
    false,
    '{"Tags": [{"Key": "Environment", "Value": "mig"}, {"Key": "Location", "Value": "us-east-1"}], "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-mig"}'::jsonb,
    'mig',
    'us-east-1',
    EXTRACT(EPOCH FROM NOW())::BIGINT,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (name) DO NOTHING;

-- Insert AWS secret version
-- AWS stores secret data as JSON with SecretString or SecretBinary
INSERT INTO aws.versions (secret_name, version_id, data, enabled, created_at)
VALUES (
    'test-secret-mig',
    'a1b2c3d4e5f6g7h8',
    '{"SecretString": "test-secret-value", "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-mig", "VersionId": "a1b2c3d4e5f6g7h8", "VersionStages": ["AWSCURRENT"]}'::jsonb,
    true,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (secret_name, version_id) DO NOTHING;

-- Insert AWS parameter
INSERT INTO aws.parameters (name, parameter_type, description, metadata, environment, location, created_at, updated_at)
VALUES (
    '/test-app/mig/test-param',
    'String',
    'Test parameter for migration',
    '{"Tags": [{"Key": "Environment", "Value": "mig"}, {"Key": "Location", "Value": "us-east-1"}]}'::jsonb,
    'mig',
    'us-east-1',
    EXTRACT(EPOCH FROM NOW())::BIGINT,
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (name) DO NOTHING;

-- Insert AWS parameter version
-- AWS parameter_versions stores value as TEXT (not JSONB)
INSERT INTO aws.parameter_versions (parameter_name, version_id, value, created_at)
VALUES (
    '/test-app/mig/test-param',
    '1',
    'test-param-value',
    EXTRACT(EPOCH FROM NOW())::BIGINT
)
ON CONFLICT (parameter_name, version_id) DO NOTHING;

