-- Add environment and location columns to GCP secrets table
-- These columns are extracted from metadata.labels for efficient filtering
ALTER TABLE gcp.secrets 
ADD COLUMN IF NOT EXISTS environment TEXT,
ADD COLUMN IF NOT EXISTS location TEXT;

-- Create indexes for efficient filtering
CREATE INDEX IF NOT EXISTS idx_gcp_secrets_environment ON gcp.secrets(environment);
CREATE INDEX IF NOT EXISTS idx_gcp_secrets_location ON gcp.secrets(location);
CREATE INDEX IF NOT EXISTS idx_gcp_secrets_environment_location ON gcp.secrets(environment, location);

-- Extract environment and location from metadata.labels for existing records
-- Environment: metadata->'labels'->>'environment' or metadata->'labels'->>'Environment' or metadata->'labels'->>'env'
-- Location: metadata->'labels'->>'location' or metadata->'labels'->>'Location' or metadata->'labels'->>'region'
UPDATE gcp.secrets
SET 
    environment = COALESCE(
        metadata->'labels'->>'environment',
        metadata->'labels'->>'Environment',
        metadata->'labels'->>'env',
        metadata->'labels'->>'Env'
    ),
    location = COALESCE(
        metadata->'labels'->>'location',
        metadata->'labels'->>'Location',
        metadata->'labels'->>'region',
        metadata->'labels'->>'Region'
    )
WHERE environment IS NULL OR location IS NULL;

