-- Fix "automatic" location values to NULL
-- "automatic" is a replication mode, not a location
-- For automatic replication, location should be NULL (no specific location)
-- GCP Secret Manager uses replication: { automatic: {} } which is valid,
-- but location should be NULL, not "automatic"

UPDATE gcp.secrets
SET location = NULL
WHERE location = 'automatic';

-- Also fix in parameters table if needed
UPDATE gcp.parameters
SET location = NULL
WHERE location = 'automatic';

