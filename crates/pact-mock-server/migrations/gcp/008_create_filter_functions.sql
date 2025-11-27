-- Create functions to get unique filter values for GCP secrets and parameters

-- Get unique environments from GCP secrets for a specific project and location
-- CRITICAL: Different locations can have different environments
CREATE OR REPLACE FUNCTION gcp.get_secret_environments(project_filter TEXT, location_filter TEXT)
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.environment
    FROM gcp.secrets s
    WHERE s.environment IS NOT NULL
      AND s.key LIKE 'projects/' || project_filter || '/secrets/%'
      AND (s.location = location_filter OR (s.location IS NULL AND location_filter IS NULL))
    ORDER BY s.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from GCP secrets for a specific project
CREATE OR REPLACE FUNCTION gcp.get_secret_locations(project_filter TEXT)
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.location
    FROM gcp.secrets s
    WHERE s.location IS NOT NULL
      AND s.key LIKE 'projects/' || project_filter || '/secrets/%'
    ORDER BY s.location;
END;
$$ LANGUAGE plpgsql;

-- Get unique project IDs from GCP secrets (extracted from key format)
CREATE OR REPLACE FUNCTION gcp.get_secret_projects()
RETURNS TABLE(project_id TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT 
        (regexp_match(s.key, '^projects/([^/]+)/secrets/'))[1] AS project_id
    FROM gcp.secrets s
    WHERE s.key ~ '^projects/[^/]+/secrets/'
    ORDER BY project_id;
END;
$$ LANGUAGE plpgsql;

-- Get unique environments from GCP parameters for a specific project
CREATE OR REPLACE FUNCTION gcp.get_parameter_environments(project_filter TEXT, location_filter TEXT)
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT p.environment
    FROM gcp.parameters p
    WHERE p.environment IS NOT NULL
      AND p.key LIKE 'projects/' || project_filter || '/locations/' || location_filter || '/parameters/%'
    ORDER BY p.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from GCP parameters for a specific project
CREATE OR REPLACE FUNCTION gcp.get_parameter_locations(project_filter TEXT)
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT p.location
    FROM gcp.parameters p
    WHERE p.location IS NOT NULL
      AND p.key LIKE 'projects/' || project_filter || '/locations/%/parameters/%'
    ORDER BY p.location;
END;
$$ LANGUAGE plpgsql;

