-- Add environment and location columns to Azure app_config table
-- Location is extracted from tags
-- Environment is extracted from key format: {prefix}:{environment}:{key} or tags
ALTER TABLE azure.app_config 
ADD COLUMN IF NOT EXISTS environment TEXT,
ADD COLUMN IF NOT EXISTS location TEXT;

-- Create indexes for efficient filtering
CREATE INDEX IF NOT EXISTS idx_azure_app_config_environment ON azure.app_config(environment);
CREATE INDEX IF NOT EXISTS idx_azure_app_config_location ON azure.app_config(location);
CREATE INDEX IF NOT EXISTS idx_azure_app_config_environment_location ON azure.app_config(environment, location);

-- Extract environment from key format: {prefix}:{environment}:{key}
-- Extract location from tags
UPDATE azure.app_config
SET 
    environment = COALESCE(
        tags->>'Environment',
        tags->>'environment',
        tags->>'Env',
        tags->>'env',
        -- Fallback: extract from key format {prefix}:{environment}:{key}
        CASE 
            WHEN key ~ '^[^:]+:([^:]+):' THEN
                (regexp_match(key, '^[^:]+:([^:]+):'))[1]
            ELSE NULL
        END
    ),
    location = COALESCE(
        tags->>'Location',
        tags->>'location',
        tags->>'Region',
        tags->>'region'
    )
WHERE environment IS NULL OR location IS NULL;

