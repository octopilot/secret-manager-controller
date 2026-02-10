-- Add environment and location columns to Azure secrets table
-- These columns are extracted from metadata.tags for efficient filtering
ALTER TABLE azure.secrets 
ADD COLUMN IF NOT EXISTS environment TEXT,
ADD COLUMN IF NOT EXISTS location TEXT;

-- Create indexes for efficient filtering
CREATE INDEX IF NOT EXISTS idx_azure_secrets_environment ON azure.secrets(environment);
CREATE INDEX IF NOT EXISTS idx_azure_secrets_location ON azure.secrets(location);
CREATE INDEX IF NOT EXISTS idx_azure_secrets_environment_location ON azure.secrets(environment, location);

-- Extract environment and location from metadata.tags for existing records
-- Azure tags format: { "key": "value", ... }
-- Environment: tags.Environment, tags.environment, tags.Env, or tags.env
-- Location: tags.Location, tags.location, tags.Region, or tags.region
UPDATE azure.secrets
SET 
    environment = COALESCE(
        metadata->'tags'->>'Environment',
        metadata->'tags'->>'environment',
        metadata->'tags'->>'Env',
        metadata->'tags'->>'env'
    ),
    location = COALESCE(
        metadata->'tags'->>'Location',
        metadata->'tags'->>'location',
        metadata->'tags'->>'Region',
        metadata->'tags'->>'region'
    )
WHERE environment IS NULL OR location IS NULL;

