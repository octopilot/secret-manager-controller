-- Add environment and location columns to GCP parameters table
-- Location is extracted from the key format: projects/{project}/locations/{location}/parameters/{parameter}
-- Environment is extracted from metadata.labels
ALTER TABLE gcp.parameters 
ADD COLUMN IF NOT EXISTS environment TEXT,
ADD COLUMN IF NOT EXISTS location TEXT;

-- Create indexes for efficient filtering
CREATE INDEX IF NOT EXISTS idx_gcp_parameters_environment ON gcp.parameters(environment);
CREATE INDEX IF NOT EXISTS idx_gcp_parameters_location ON gcp.parameters(location);
CREATE INDEX IF NOT EXISTS idx_gcp_parameters_environment_location ON gcp.parameters(environment, location);

-- Extract location from key format: projects/{project}/locations/{location}/parameters/{parameter}
-- Extract environment from metadata.labels
UPDATE gcp.parameters
SET 
    location = CASE 
        WHEN key ~ '^projects/[^/]+/locations/([^/]+)/parameters/' THEN
            (regexp_match(key, '^projects/[^/]+/locations/([^/]+)/parameters/'))[1]
        ELSE NULL
    END,
    environment = COALESCE(
        metadata->'labels'->>'environment',
        metadata->'labels'->>'Environment',
        metadata->'labels'->>'env',
        metadata->'labels'->>'Env'
    )
WHERE location IS NULL OR environment IS NULL;

