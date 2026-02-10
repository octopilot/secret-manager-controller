-- Add environment and location columns to AWS parameters table
-- Location is extracted from the name format: /{prefix}/{environment}/{key} or /{prefix}/{location}/{key}
-- Environment is extracted from metadata.Tags or name format
ALTER TABLE aws.parameters 
ADD COLUMN IF NOT EXISTS environment TEXT,
ADD COLUMN IF NOT EXISTS location TEXT;

-- Create indexes for efficient filtering
CREATE INDEX IF NOT EXISTS idx_aws_parameters_environment ON aws.parameters(environment);
CREATE INDEX IF NOT EXISTS idx_aws_parameters_location ON aws.parameters(location);
CREATE INDEX IF NOT EXISTS idx_aws_parameters_environment_location ON aws.parameters(environment, location);

-- Extract environment and location from name format: /{prefix}/{environment}/{key} or /{prefix}/{location}/{key}
-- Also extract from metadata.Tags
UPDATE aws.parameters
SET 
    environment = COALESCE(
        (SELECT tag->>'Value' FROM jsonb_array_elements(metadata->'Tags') AS tag WHERE tag->>'Key' IN ('Environment', 'environment', 'Env', 'env') LIMIT 1),
        -- Fallback: extract from name format /{prefix}/{environment}/{key}
        CASE 
            WHEN name ~ '^/[^/]+/([^/]+)/' THEN
                (regexp_match(name, '^/[^/]+/([^/]+)/'))[1]
            ELSE NULL
        END
    ),
    location = COALESCE(
        (SELECT tag->>'Value' FROM jsonb_array_elements(metadata->'Tags') AS tag WHERE tag->>'Key' IN ('Location', 'location', 'Region', 'region') LIMIT 1),
        NULL
    )
WHERE environment IS NULL OR location IS NULL;

