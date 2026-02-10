-- Create functions to get unique filter values for AWS secrets and parameters

-- Get unique environments from AWS secrets
CREATE OR REPLACE FUNCTION aws.get_secret_environments()
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.environment
    FROM aws.secrets s
    WHERE s.environment IS NOT NULL
    ORDER BY s.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from AWS secrets
CREATE OR REPLACE FUNCTION aws.get_secret_locations()
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT s.location
    FROM aws.secrets s
    WHERE s.location IS NOT NULL
    ORDER BY s.location;
END;
$$ LANGUAGE plpgsql;

-- Get unique environments from AWS parameters
CREATE OR REPLACE FUNCTION aws.get_parameter_environments()
RETURNS TABLE(environment TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT p.environment
    FROM aws.parameters p
    WHERE p.environment IS NOT NULL
    ORDER BY p.environment;
END;
$$ LANGUAGE plpgsql;

-- Get unique locations from AWS parameters
CREATE OR REPLACE FUNCTION aws.get_parameter_locations()
RETURNS TABLE(location TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT p.location
    FROM aws.parameters p
    WHERE p.location IS NOT NULL
    ORDER BY p.location;
END;
$$ LANGUAGE plpgsql;

