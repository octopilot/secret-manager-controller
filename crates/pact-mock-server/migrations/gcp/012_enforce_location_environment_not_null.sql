-- Enforce NOT NULL constraints on location and environment columns for GCP secrets
-- This ensures data integrity at the database level

-- First, update any NULL values to a default (if any exist)
-- For environment, we'll use 'unknown' as a fallback
-- For location, we'll use NULL for automatic replication (which is valid for GCP)
UPDATE gcp.secrets
SET environment = 'unknown'
WHERE environment IS NULL;

-- Add NOT NULL constraint to environment
ALTER TABLE gcp.secrets
ALTER COLUMN environment SET NOT NULL;

-- Note: location can be NULL for GCP automatic replication
-- We do NOT add NOT NULL constraint to location for GCP
-- Automatic replication means no specific location, which is represented as NULL

-- Add check constraint to ensure environment is not empty
-- Drop constraint if it exists first (idempotent)
ALTER TABLE gcp.secrets
DROP CONSTRAINT IF EXISTS chk_gcp_secrets_environment_not_empty;

ALTER TABLE gcp.secrets
ADD CONSTRAINT chk_gcp_secrets_environment_not_empty
CHECK (environment != '');

-- Create index if not exists (should already exist from migration 006)
CREATE INDEX IF NOT EXISTS idx_gcp_secrets_environment ON gcp.secrets(environment);

