-- Enforce NOT NULL constraints on location and environment columns for AWS secrets
-- This ensures data integrity at the database level

-- First, update any NULL values to defaults
-- For environment, we'll use 'unknown' as a fallback
-- For location, we'll use the region from ARN if available, otherwise 'unknown'
UPDATE aws.secrets
SET 
    environment = COALESCE(environment, 'unknown'),
    location = COALESCE(
        location,
        -- Try to extract from ARN if present
        CASE 
            WHEN metadata->>'ARN' IS NOT NULL AND metadata->>'ARN' ~ 'arn:aws:[^:]+:([^:]+):' THEN
                (regexp_match(metadata->>'ARN', 'arn:aws:[^:]+:([^:]+):'))[1]
            ELSE 'unknown'
        END
    )
WHERE environment IS NULL OR location IS NULL;

-- Add NOT NULL constraints
ALTER TABLE aws.secrets
ALTER COLUMN environment SET NOT NULL,
ALTER COLUMN location SET NOT NULL;

-- Add check constraints to ensure values are not empty
-- Drop constraints if they exist first (idempotent)
ALTER TABLE aws.secrets
DROP CONSTRAINT IF EXISTS chk_aws_secrets_environment_not_empty;

ALTER TABLE aws.secrets
ADD CONSTRAINT chk_aws_secrets_environment_not_empty
CHECK (environment != '');

ALTER TABLE aws.secrets
DROP CONSTRAINT IF EXISTS chk_aws_secrets_location_not_empty;

ALTER TABLE aws.secrets
ADD CONSTRAINT chk_aws_secrets_location_not_empty
CHECK (location != '');

-- Create indexes if not exists (should already exist from migration 007)
CREATE INDEX IF NOT EXISTS idx_aws_secrets_environment ON aws.secrets(environment);
CREATE INDEX IF NOT EXISTS idx_aws_secrets_location ON aws.secrets(location);

