-- Add environment and location columns to AWS secrets table
-- These columns are extracted from metadata.Tags for efficient filtering
ALTER TABLE aws.secrets 
ADD COLUMN IF NOT EXISTS environment TEXT,
ADD COLUMN IF NOT EXISTS location TEXT;

-- Create indexes for efficient filtering
CREATE INDEX IF NOT EXISTS idx_aws_secrets_environment ON aws.secrets(environment);
CREATE INDEX IF NOT EXISTS idx_aws_secrets_location ON aws.secrets(location);
CREATE INDEX IF NOT EXISTS idx_aws_secrets_environment_location ON aws.secrets(environment, location);

-- Extract environment and location from metadata.Tags for existing records
-- AWS Tags format: [{"Key": "key", "Value": "value"}, ...]
-- Environment: Tag with Key "Environment", "environment", "Env", or "env"
-- Location: Tag with Key "Location", "location", "Region", or "region" (or from ARN)
UPDATE aws.secrets
SET 
    environment = COALESCE(
        (SELECT tag->>'Value' FROM jsonb_array_elements(metadata->'Tags') AS tag WHERE tag->>'Key' IN ('Environment', 'environment', 'Env', 'env') LIMIT 1),
        NULL
    ),
    location = COALESCE(
        (SELECT tag->>'Value' FROM jsonb_array_elements(metadata->'Tags') AS tag WHERE tag->>'Key' IN ('Location', 'location', 'Region', 'region') LIMIT 1),
        -- Fallback: extract region from ARN if present in metadata
        CASE 
            WHEN metadata->>'ARN' IS NOT NULL AND metadata->>'ARN' ~ 'arn:aws:[^:]+:([^:]+):' THEN
                (regexp_match(metadata->>'ARN', 'arn:aws:[^:]+:([^:]+):'))[1]
            ELSE NULL
        END
    )
WHERE environment IS NULL OR location IS NULL;

