-- Enforce NOT NULL constraints on location and environment columns for Azure secrets
-- This ensures data integrity at the database level

-- First, update any NULL values to defaults
-- For environment, we'll use 'unknown' as a fallback
-- For location, we'll use 'unknown' as a fallback
UPDATE azure.secrets
SET 
    environment = COALESCE(environment, 'unknown'),
    location = COALESCE(location, 'unknown')
WHERE environment IS NULL OR location IS NULL;

-- Add NOT NULL constraints
ALTER TABLE azure.secrets
ALTER COLUMN environment SET NOT NULL,
ALTER COLUMN location SET NOT NULL;

-- Add check constraints to ensure values are not empty
-- Drop constraints if they exist first (idempotent)
ALTER TABLE azure.secrets
DROP CONSTRAINT IF EXISTS chk_azure_secrets_environment_not_empty;

ALTER TABLE azure.secrets
ADD CONSTRAINT chk_azure_secrets_environment_not_empty
CHECK (environment != '');

ALTER TABLE azure.secrets
DROP CONSTRAINT IF EXISTS chk_azure_secrets_location_not_empty;

ALTER TABLE azure.secrets
ADD CONSTRAINT chk_azure_secrets_location_not_empty
CHECK (location != '');

-- Create indexes if not exists (should already exist from migration 007)
CREATE INDEX IF NOT EXISTS idx_azure_secrets_environment ON azure.secrets(environment);
CREATE INDEX IF NOT EXISTS idx_azure_secrets_location ON azure.secrets(location);

