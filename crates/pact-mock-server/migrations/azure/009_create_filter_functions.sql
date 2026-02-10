-- Create functions to get unique filter values for Azure secrets and app config

-- Get unique environments from Azure secrets
CREATE OR REPLACE FUNCTION azure.get_secret_environments()
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.environment
    FROM azure.secrets s
    WHERE s.environment IS NOT NULL
    ORDER BY s.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from Azure secrets
CREATE OR REPLACE FUNCTION azure.get_secret_locations()
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.location
    FROM azure.secrets s
    WHERE s.location IS NOT NULL
    ORDER BY s.location;
END;
$$ LANGUAGE plpgsql;

-- Get unique environments from Azure app config
CREATE OR REPLACE FUNCTION azure.get_app_config_environments()
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT a.environment
    FROM azure.app_config a
    WHERE a.environment IS NOT NULL
    ORDER BY a.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from Azure app config
CREATE OR REPLACE FUNCTION azure.get_app_config_locations()
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT a.location
    FROM azure.app_config a
    WHERE a.location IS NOT NULL
    ORDER BY a.location;
END;
$$ LANGUAGE plpgsql;

