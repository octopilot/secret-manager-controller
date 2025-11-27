import { Component, createSignal, createEffect, onMount, Show, For } from 'solid-js';

interface Secret {
  name: string;
  provider: 'gcp' | 'aws' | 'azure';
  project?: string;
  location?: string;
  environment?: string;
  value?: string;
  created?: string;
  enabled?: boolean;
  versions?: number;
}

interface SecretsViewerProps {
  onNavigate?: (category: string, section: string | null, page: string | null) => void;
}

const SecretsViewer: Component<SecretsViewerProps> = (_props) => {
  const [secrets, setSecrets] = createSignal<Secret[]>([]);
  const [loading, setLoading] = createSignal<boolean>(false);
  const [error, setError] = createSignal<string | null>(null);
  const [selectedProvider, setSelectedProvider] = createSignal<'gcp' | 'aws' | 'azure'>('gcp');
  const [project, setProject] = createSignal<string>('');
  const [projects, setProjects] = createSignal<string[]>([]);
  const [loadingProjects, setLoadingProjects] = createSignal<boolean>(false);
  const [expandedSecrets, setExpandedSecrets] = createSignal<Set<string>>(new Set());
  // Filter states
  const [selectedEnvironment, setSelectedEnvironment] = createSignal<string>('');
  const [selectedLocation, setSelectedLocation] = createSignal<string>('');
  const [viewType, setViewType] = createSignal<'secrets' | 'parameters'>('secrets');
  const [environments, setEnvironments] = createSignal<string[]>([]);
  const [locations, setLocations] = createSignal<string[]>([]);

  // Use nginx reverse proxy endpoints (cleanest solution)
  // The nginx server proxies requests to the internal Kubernetes services
  // This works whether accessing via localhost (port-forward) or from within cluster
  const getEndpoint = (provider: 'gcp' | 'aws' | 'azure'): string => {
    // Use relative URLs that go through the nginx proxy
    // The proxy routes are:
    // - /api/mock-servers/gcp/* -> gcp-mock-server:1234/*
    // - /api/mock-servers/aws/* -> aws-mock-server:1234/*
    // - /api/mock-servers/azure/* -> azure-mock-server:1234/*
    return `/api/mock-servers/${provider}`;
  };

  const fetchGCPProjects = async () => {
    try {
      const endpoint = getEndpoint('gcp');
      const response = await fetch(`${endpoint}/v1/projects`);
      
      if (!response.ok) {
        // If endpoint doesn't exist or returns error, return empty list
        if (response.status === 404) {
          return [];
        }
        const errorText = await response.text().catch(() => response.statusText);
        throw new Error(`Failed to fetch GCP projects: ${response.status} ${response.statusText} - ${errorText}`);
      }

      const data = await response.json();
      return (data.projects || []) as string[];
    } catch (err) {
      // If error fetching projects, return empty list (projects may not exist yet)
      console.warn('Failed to fetch GCP projects:', err);
      return [];
    }
  };

  const fetchGCPSecrets = async (projectId: string) => {
    try {
      const endpoint = getEndpoint('gcp');
      // Build query parameters for filtering
      // Only add filter if a value is selected (empty string means "all")
      const queryParams = new URLSearchParams();
      if (selectedEnvironment() && selectedEnvironment() !== '') {
        queryParams.append('environment', selectedEnvironment());
      }
      if (selectedLocation() && selectedLocation() !== '') {
        queryParams.append('location', selectedLocation());
      }
      const queryString = queryParams.toString();
      const url = `${endpoint}/v1/projects/${projectId}/secrets${queryString ? `?${queryString}` : ''}`;
      const response = await fetch(url);
      
      if (!response.ok) {
        const errorText = await response.text().catch(() => response.statusText);
        throw new Error(`Failed to fetch GCP secrets: ${response.status} ${response.statusText} - ${errorText}`);
      }

      const data = await response.json();
      const secretList: Secret[] = (data.secrets || []).map((secret: any) => {
        const secretName = secret.name.split('/').pop() || secret.name;
        
        // Extract metadata from labels (if controller stored them)
        // Labels are stored as object: { "key": "value", ... }
        const labels = secret.labels || {};
        const location = labels.location || labels.region || labels.Location || labels.Region;
        const environment = labels.environment || labels.Environment || labels.env || labels.Env;
        // Project is already known from the API call
        
        return {
          name: secretName,
          provider: 'gcp' as const,
          project: projectId,
          location,
          environment,
          created: secret.create_time,
          enabled: true,
        };
      });

      // Fetch values for each secret
      const secretsWithValues = await Promise.all(
        secretList.map(async (secret) => {
          try {
            const valueResponse = await fetch(
              `${endpoint}/v1/projects/${projectId}/secrets/${secret.name}/versions/latest:access`
            );
            if (valueResponse.ok) {
              const valueData = await valueResponse.json();
              // Decode base64
              if (valueData.payload?.data) {
                try {
                  const decoded = atob(valueData.payload.data);
                  return { ...secret, value: decoded };
                } catch {
                  return { ...secret, value: valueData.payload.data };
                }
              }
            }
          } catch {
            // Ignore errors fetching individual values
          }
          return secret;
        })
      );

      return secretsWithValues;
    } catch (err) {
      throw new Error(`Failed to fetch GCP secrets: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const fetchAWSSecrets = async () => {
    try {
      const endpoint = getEndpoint('aws');
      // AWS uses POST with JSON body to root path
      // Ensure trailing slash for nginx rewrite to work correctly
      const awsEndpoint = endpoint.endsWith('/') ? endpoint : `${endpoint}/`;
      const response = await fetch(awsEndpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/x-amz-json-1.1',
          'X-Amz-Target': 'secretsmanager.ListSecrets',
        },
        body: JSON.stringify({}),
      });

      if (!response.ok) {
        const errorText = await response.text().catch(() => response.statusText);
        throw new Error(`Failed to fetch AWS secrets: ${response.status} ${response.statusText} - ${errorText}`);
      }

      const data = await response.json();
      const secretList: Secret[] = (data.SecretList || []).map((secret: any) => {
        const secretName = secret.Name;
        
        // Extract metadata from Tags array (if controller stored them)
        // Tags format: [{ Key: "key", Value: "value" }, ...]
        const tags = secret.Tags || [];
        const tagMap = new Map(tags.map((tag: any) => [tag.Key, tag.Value]));
        
        const location = tagMap.get('Location') || tagMap.get('location') || tagMap.get('Region') || tagMap.get('region') || 
                        secret.ARN?.split(':')[3]; // Fallback to region from ARN
        const environment = tagMap.get('Environment') || tagMap.get('environment') || tagMap.get('Env') || tagMap.get('env');
        const project = tagMap.get('Project') || tagMap.get('project') || tagMap.get('ProjectId');
        
        return {
          name: secretName,
          provider: 'aws' as const,
          project,
          location,
          environment,
          created: secret.LastChangedDate,
          enabled: true,
        };
      });

      // Fetch values for each secret
      const secretsWithValues = await Promise.all(
        secretList.map(async (secret) => {
          try {
            const valueResponse = await fetch(awsEndpoint, {
              method: 'POST',
              headers: {
                'Content-Type': 'application/x-amz-json-1.1',
                'X-Amz-Target': 'secretsmanager.GetSecretValue',
              },
              body: JSON.stringify({ SecretId: secret.name }),
            });
            if (valueResponse.ok) {
              const valueData = await valueResponse.json();
              return { ...secret, value: valueData.SecretString || valueData.SecretBinary };
            }
          } catch {
            // Ignore errors fetching individual values
          }
          return secret;
        })
      );

      return secretsWithValues;
    } catch (err) {
      throw new Error(`Failed to fetch AWS secrets: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const fetchAzureSecrets = async () => {
    try {
      const endpoint = getEndpoint('azure');
      const apiVersion = 'api-version=2025-07-01';
      const response = await fetch(`${endpoint}/secrets?${apiVersion}`);

      if (!response.ok) {
        // If 400 Bad Request, some secrets might have invalid names
        // Try to continue with empty list rather than failing completely
        if (response.status === 400) {
          console.warn('Azure Key Vault returned 400 Bad Request - some secret names may be invalid. Returning empty list.');
          return [];
        }
        const errorText = await response.text().catch(() => response.statusText);
        throw new Error(`Failed to fetch Azure secrets: ${response.status} ${response.statusText} - ${errorText}`);
      }

      const data = await response.json();
      const secretList: Secret[] = (data.value || []).map((secret: any) => {
        const secretName = secret.id.split('/').pop() || secret.id;
        
        // Extract metadata from tags object (if controller stored them)
        // Tags format: { "key": "value", ... }
        const tags = secret.tags || {};
        const location = tags.Location || tags.location || tags.Region || tags.region;
        const environment = tags.Environment || tags.environment || tags.Env || tags.env;
        const project = tags.Project || tags.project || tags.ProjectId;
        
        return {
          name: secretName,
          provider: 'azure' as const,
          project,
          location,
          environment,
          created: secret.attributes?.created,
          enabled: secret.attributes?.enabled !== false,
        };
      });

      // Fetch values for each secret
      const secretsWithValues = await Promise.all(
        secretList.map(async (secret) => {
          try {
            // URL encode secret name to handle special characters
            const encodedName = encodeURIComponent(secret.name);
            const valueResponse = await fetch(
              `${endpoint}/secrets/${encodedName}?${apiVersion}`
            );
            if (valueResponse.ok) {
              const valueData = await valueResponse.json();
              return { ...secret, value: valueData.value };
            } else if (valueResponse.status === 400) {
              // Azure Key Vault returns 400 for invalid secret names
              // Log but continue - secret might have been created with invalid name
              console.warn(`Azure secret name validation error for ${secret.name}: ${valueResponse.statusText}`);
            }
          } catch (err) {
            // Ignore errors fetching individual values
            console.warn(`Failed to fetch value for secret ${secret.name}:`, err);
          }
          return secret;
        })
      );

      return secretsWithValues;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error';
      console.error('Azure fetch error:', err);
      throw new Error(`Failed to fetch Azure secrets: ${errorMessage}`);
    }
  };

  // Load locations for the selected project (GCP only)
  const loadLocations = async () => {
    const provider = selectedProvider();
    const currentProject = project();
    const currentViewType = viewType();
    
    if (provider !== 'gcp' || !currentProject) {
      setLocations([]);
      return;
    }

    try {
      const endpoint = getEndpoint('gcp');
      let url: string;
      
      if (currentViewType === 'secrets') {
        url = `${endpoint}/v1/projects/${currentProject}/secrets/locations`;
      } else {
        url = `${endpoint}/v1/projects/${currentProject}/parameters/locations`;
      }
      
      console.log(`[SecretsViewer] Loading locations for project: ${currentProject}, viewType: ${currentViewType}, url: ${url}`);
      const response = await fetch(url);
      if (response.ok) {
        const data = await response.json();
        const locationsList = (data.locations || []) as string[];
        console.log(`[SecretsViewer] Loaded ${locationsList.length} locations:`, locationsList);
        setLocations(locationsList);
      } else {
        console.warn(`[SecretsViewer] Failed to load locations: ${response.status} ${response.statusText}`);
        setLocations([]);
      }
    } catch (err) {
      console.error('[SecretsViewer] Error loading locations:', err);
      setLocations([]);
    }
  };

  // Load environments for the selected project+location combination (GCP only)
  const loadEnvironments = async () => {
    const provider = selectedProvider();
    const currentProject = project();
    const currentLocation = selectedLocation();
    const currentViewType = viewType();
    
    if (provider !== 'gcp' || !currentProject) {
      setEnvironments([]);
      return;
    }

    // For secrets, we need project only (locations are independent)
    // For parameters, we need project + location
    if (currentViewType === 'secrets') {
      // Load environments for all secrets in the project
      try {
        const endpoint = getEndpoint('gcp');
        const url = `${endpoint}/v1/projects/${currentProject}/secrets/environments`;
        console.log(`[SecretsViewer] Loading environments for project: ${currentProject}, viewType: secrets, url: ${url}`);
        const response = await fetch(url);
        if (response.ok) {
          const data = await response.json();
          const envs = (data.environments || []) as string[];
          const allEnvs = new Set(envs);
          if (envs.length === 0 || envs.includes('pact')) {
            allEnvs.add('pact');
          }
          const sortedEnvs = Array.from(allEnvs).sort();
          console.log(`[SecretsViewer] Loaded ${sortedEnvs.length} environments:`, sortedEnvs);
          setEnvironments(sortedEnvs);
        } else {
          console.warn(`[SecretsViewer] Failed to load environments: ${response.status} ${response.statusText}`);
          setEnvironments([]);
        }
      } catch (err) {
        console.error('[SecretsViewer] Error loading environments:', err);
        setEnvironments([]);
      }
    } else {
      // For parameters, we need location to be selected
      if (!currentLocation) {
        setEnvironments([]);
        return;
      }
      
      try {
        const endpoint = getEndpoint('gcp');
        const url = `${endpoint}/v1/projects/${currentProject}/locations/${currentLocation}/parameters/environments`;
        console.log(`[SecretsViewer] Loading environments for project: ${currentProject}, location: ${currentLocation}, viewType: parameters, url: ${url}`);
        const response = await fetch(url);
        if (response.ok) {
          const data = await response.json();
          const envs = (data.environments || []) as string[];
          const allEnvs = new Set(envs);
          if (envs.length === 0 || envs.includes('pact')) {
            allEnvs.add('pact');
          }
          const sortedEnvs = Array.from(allEnvs).sort();
          console.log(`[SecretsViewer] Loaded ${sortedEnvs.length} environments:`, sortedEnvs);
          setEnvironments(sortedEnvs);
        } else {
          console.warn(`[SecretsViewer] Failed to load environments: ${response.status} ${response.statusText}`);
          setEnvironments([]);
        }
      } catch (err) {
        console.error('[SecretsViewer] Error loading environments:', err);
        setEnvironments([]);
      }
    }
  };

  // Load filter values for AWS/Azure (non-GCP providers)
  const loadFilterValues = async () => {
    if (selectedProvider() === 'gcp') {
      // GCP uses cascading dropdowns, handled separately
      return;
    }

    try {
      if (selectedProvider() === 'azure') {
        const endpoint = getEndpoint('azure');
        
        const [envResponse, locResponse] = await Promise.all([
          fetch(`${endpoint}/secrets/environments`),
          fetch(`${endpoint}/secrets/locations`),
        ]);
        
        if (envResponse.ok) {
          const envData = await envResponse.json();
          const envs = (envData.environments || []) as string[];
          setEnvironments(envs.sort());
        }
        
        if (locResponse.ok) {
          const locData = await locResponse.json();
          setLocations((locData.locations || []) as string[]);
        }
      } else if (selectedProvider() === 'aws') {
        const endpoint = getEndpoint('aws');
        
        const [envResponse, locResponse, projResponse] = await Promise.all([
          fetch(`${endpoint}/environments`),
          fetch(`${endpoint}/locations`),
          fetch(`${endpoint}/projects`),
        ]);
        
        if (envResponse.ok) {
          const envData = await envResponse.json();
          const envs = (envData.environments || []) as string[];
          setEnvironments(envs.sort());
        }
        
        if (locResponse.ok) {
          const locData = await locResponse.json();
          setLocations((locData.locations || []) as string[]);
        }
        
        if (projResponse.ok) {
          const projData = await projResponse.json();
          setProjects((projData.projects || []) as string[]);
        }
      }
    } catch (err) {
      console.error('Error loading filter values:', err);
    }
  };

  // Reactive effect: Load locations when project or viewType changes (GCP only)
  createEffect(() => {
    // Read signals to track dependencies - SolidJS tracks these automatically
    const provider = selectedProvider();
    const currentProject = project();
    const currentViewType = viewType();
    
    if (provider === 'gcp' && currentProject) {
      // Clear location and environment when project or viewType changes
      setSelectedLocation('');
      setSelectedEnvironment('');
      setSecrets([]);
      // Load locations for the selected project (async, but don't await in effect)
      loadLocations().catch(err => console.error('Error loading locations:', err));
    } else if (provider === 'gcp') {
      // Clear everything if no project selected
      setLocations([]);
      setEnvironments([]);
      setSelectedLocation('');
      setSelectedEnvironment('');
      setSecrets([]);
    }
  });

  // Reactive effect: Load environments when project+location+viewType changes (GCP only)
  createEffect(() => {
    // Read signals to track dependencies
    const provider = selectedProvider();
    const currentProject = project();
    const currentLocation = selectedLocation();
    const currentViewType = viewType();
    
    if (provider === 'gcp' && currentProject) {
      if (currentViewType === 'secrets') {
        // For secrets, load environments when project is selected (location is independent)
        loadEnvironments().catch(err => console.error('Error loading environments:', err));
      } else {
        // For parameters, load environments when both project and location are selected
        if (currentLocation) {
          loadEnvironments().catch(err => console.error('Error loading environments:', err));
        } else {
          setEnvironments([]);
          setSelectedEnvironment('');
          setSecrets([]);
        }
      }
    }
  });

  // Reactive effect: Load secrets when all filters are selected (GCP only)
  createEffect(() => {
    const provider = selectedProvider();
    const currentProject = project();
    const currentEnvironment = selectedEnvironment();
    const currentLocation = selectedLocation();
    
    if (provider === 'gcp' && currentProject && currentEnvironment && currentLocation) {
      loadSecrets();
    } else if (provider === 'gcp') {
      setSecrets([]);
    }
  });

  const fetchGCPParameters = async (projectId: string) => {
    try {
      const endpoint = getEndpoint('gcp');
      // Build query parameters for filtering
      // All filters are required, so always include them
      const queryParams = new URLSearchParams();
      queryParams.append('environment', selectedEnvironment());
      queryParams.append('location', selectedLocation());
      const queryString = queryParams.toString();
      // Location is required, so use the selected location
      const location = selectedLocation();
      const url = `${endpoint}/v1/projects/${projectId}/locations/${location}/parameters?${queryString}`;
      const response = await fetch(url);
      
      if (!response.ok) {
        const errorText = await response.text().catch(() => response.statusText);
        throw new Error(`Failed to fetch GCP parameters: ${response.status} ${response.statusText} - ${errorText}`);
      }

      const data = await response.json();
      const parameterList: Secret[] = (data.parameters || []).map((param: any) => {
        const paramName = param.name.split('/').pop() || param.name;
        
        // Extract metadata from labels (if controller stored them)
        const labels = param.labels || {};
        const location = labels.location || labels.region || labels.Location || labels.Region || 'global';
        const environment = labels.environment || labels.Environment || labels.env || labels.Env;
        
        return {
          name: paramName,
          provider: 'gcp' as const,
          project: projectId,
          location,
          environment,
          created: param.create_time,
          enabled: true,
        };
      });

      // Fetch values for each parameter
      const parametersWithValues = await Promise.all(
        parameterList.map(async (param) => {
          try {
            const location = param.location || 'global';
            const valueResponse = await fetch(
              `${endpoint}/v1/projects/${projectId}/locations/${location}/parameters/${param.name}/versions/latest:render`
            );
            if (valueResponse.ok) {
              const valueData = await valueResponse.json();
              // Decode base64
              if (valueData.payload?.data) {
                try {
                  const decoded = atob(valueData.payload.data);
                  return { ...param, value: decoded };
                } catch {
                  return { ...param, value: valueData.payload.data };
                }
              }
            }
          } catch {
            // Ignore errors fetching individual values
          }
          return param;
        })
      );

      return parametersWithValues;
    } catch (err) {
      throw new Error(`Failed to fetch GCP parameters: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const loadSecrets = async () => {
    // Don't load secrets for GCP if required filters are not selected
    if (selectedProvider() === 'gcp') {
      if (!project() || !selectedEnvironment() || !selectedLocation()) {
        setSecrets([]);
        return;
      }
    }

    setLoading(true);
    setError(null);

    try {
      let fetchedSecrets: Secret[] = [];

      if (selectedProvider() === 'gcp') {
        if (!project() || !selectedEnvironment() || !selectedLocation()) {
          setSecrets([]);
          return;
        }
        // Check viewType to load either secrets or parameters
        if (viewType() === 'secrets') {
          fetchedSecrets = await fetchGCPSecrets(project());
        } else {
          fetchedSecrets = await fetchGCPParameters(project());
        }
      } else if (selectedProvider() === 'aws') {
        fetchedSecrets = await fetchAWSSecrets();
      } else if (selectedProvider() === 'azure') {
        fetchedSecrets = await fetchAzureSecrets();
      }

      setSecrets(fetchedSecrets);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load secrets');
      console.error('Error loading secrets:', err);
    } finally {
      setLoading(false);
    }
  };

  const toggleSecret = (secretName: string) => {
    const expanded = new Set(expandedSecrets());
    if (expanded.has(secretName)) {
      expanded.delete(secretName);
    } else {
      expanded.add(secretName);
    }
    setExpandedSecrets(expanded);
  };

  onMount(() => {
    // Load projects when GCP is selected
    if (selectedProvider() === 'gcp') {
      loadProjects();
    } else if (selectedProvider() === 'azure' || selectedProvider() === 'aws') {
      // Load filter values for Azure and AWS
      loadFilterValues();
    }
  });

  const loadProjects = async () => {
    if (selectedProvider() !== 'gcp') {
      return;
    }

    setLoadingProjects(true);
    try {
      const projectList = await fetchGCPProjects();
      setProjects(projectList);
      
      // Don't auto-select project - user must select explicitly
      if (projectList.length === 0) {
        // Clear project selection if no projects found
        setProject('');
        setSecrets([]);
      } else if (project()) {
        // Project already selected, just load filter values
        await loadFilterValues();
      }
    } catch (err) {
      console.error('Error loading projects:', err);
      setProjects([]);
    } finally {
      setLoadingProjects(false);
    }
  };

  return (
    <div class="space-y-6">
      <div>
        <h1 class="text-3xl font-bold text-gray-900 mb-2">Pact Mock Secrets Viewer</h1>
        <p class="text-gray-600">
          View secrets synced by the controller from <code class="bg-gray-100 px-1 rounded">application.secrets.env</code> files.
          Secrets are displayed directly from the mock server API - no hardcoded values.
        </p>
        <p class="text-sm text-gray-500 mt-2">
          💡 <strong>Tip:</strong> Update values in <code class="bg-gray-100 px-1 rounded">application.secrets.env</code> and commit to trigger controller reconciliation.
          The viewer will show updated values after the controller syncs.
        </p>
      </div>

      {/* Provider Selection */}
      <div class="bg-white border border-gray-200 rounded-lg p-4">
        <div class="flex flex-wrap items-center gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Provider</label>
            <select
              value={selectedProvider()}
              onChange={async (e) => {
                const newProvider = e.currentTarget.value as 'gcp' | 'aws' | 'azure';
                setSelectedProvider(newProvider);
                setProject('');
                setSelectedEnvironment('');
                setSelectedLocation('');
                setEnvironments([]);
                setLocations([]);
                setSecrets([]);
                // Load projects if switching to GCP
                if (newProvider === 'gcp') {
                  loadProjects();
                } else {
                  // For AWS/Azure, load filter values and then secrets
                  await loadFilterValues();
                  loadSecrets();
                }
              }}
              class="border border-gray-300 rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-green-500"
            >
              <option value="gcp">GCP Secret Manager</option>
              <option value="aws">AWS Secrets Manager</option>
              <option value="azure">Azure Key Vault</option>
            </select>
          </div>

          <Show when={selectedProvider() === 'gcp'}>
            <div>
              <label class="block text-sm font-medium text-gray-700 mb-1">Project ID</label>
              <Show when={loadingProjects()}>
                <div class="text-sm text-gray-500">Loading projects...</div>
              </Show>
              <Show when={!loadingProjects() && projects().length > 0}>
              <select
                value={project()}
                onChange={(e) => {
                  const newProject = e.currentTarget.value;
                  setProject(newProject);
                  // Reactive effects will handle loading locations and clearing dependent dropdowns
                }}
                required
                class="border border-gray-300 rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-green-500 w-full"
              >
                  <option value="">Select Project</option>
                  <For each={projects()}>
                    {(proj) => (
                      <option value={proj}>{proj}</option>
                    )}
                  </For>
                </select>
              </Show>
              <Show when={!loadingProjects() && projects().length === 0}>
                <div class="border border-gray-300 rounded-md px-3 py-2 text-sm bg-gray-50 text-gray-500">
                  No projects found. Projects will appear here after the controller syncs secrets to the mock provider.
                </div>
              </Show>
            </div>
          </Show>

          {/* View Type Toggle (Secrets vs Parameters) */}
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">View Type</label>
            <select
              value={viewType()}
              onChange={(e) => {
                setViewType(e.currentTarget.value as 'secrets' | 'parameters');
                // Clear location and environment when switching view type
                setSelectedLocation('');
                setSelectedEnvironment('');
                setSecrets([]);
                // Reactive effects will handle reloading filter values
              }}
              class="border border-gray-300 rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-green-500"
            >
              <option value="secrets">Secrets</option>
              <option value="parameters">Parameters</option>
            </select>
          </div>

          {/* Environment Filter */}
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Environment *</label>
            <select
              value={selectedEnvironment()}
              onChange={(e) => {
                setSelectedEnvironment(e.currentTarget.value);
                // Reactive effect will handle loading secrets when all filters are selected
              }}
              disabled={
                selectedProvider() === 'gcp' && 
                (!project() || (viewType() === 'parameters' && !selectedLocation()))
              }
              required
              class={`border border-gray-300 rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-green-500 ${
                selectedProvider() === 'gcp' && 
                (!project() || (viewType() === 'parameters' && !selectedLocation()))
                  ? 'bg-gray-100 text-gray-400 cursor-not-allowed'
                  : ''
              }`}
            >
              <option value="">Select Environment</option>
              <For each={environments()}>
                {(env) => (
                  <option value={env}>{env}</option>
                )}
              </For>
            </select>
          </div>

          {/* Location Filter (GCP only) */}
          <Show when={selectedProvider() === 'gcp'}>
            <div>
              <label class="block text-sm font-medium text-gray-700 mb-1">Location *</label>
              <select
                value={selectedLocation()}
                onChange={(e) => {
                  const newLocation = e.currentTarget.value;
                  setSelectedLocation(newLocation);
                  // Clear environment when location changes (for parameters)
                  if (viewType() === 'parameters') {
                    setSelectedEnvironment('');
                    setSecrets([]);
                  }
                  // Reactive effects will handle loading environments and secrets
                }}
                disabled={!project()}
                required
                class={`border border-gray-300 rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-green-500 ${
                  !project() ? 'bg-gray-100 text-gray-400 cursor-not-allowed' : ''
                }`}
              >
                <option value="">Select Location</option>
                <For each={locations()}>
                  {(loc) => (
                    <option value={loc}>{loc}</option>
                  )}
                </For>
              </select>
            </div>
          </Show>

          <div class="flex items-end">
            <button
              onClick={loadSecrets}
              disabled={loading()}
              class="bg-green-600 text-white px-4 py-2 rounded-md text-sm font-medium hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading() ? 'Loading...' : 'Refresh'}
            </button>
          </div>
        </div>
      </div>

      {/* Error Message */}
      <Show when={error()}>
        <div class="bg-red-50 border border-red-200 rounded-lg p-4">
          <div class="flex items-center gap-2">
            <svg class="w-5 h-5 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p class="text-red-800">{error()}</p>
          </div>
          <div class="text-sm text-red-600 mt-2 space-y-1">
            <p><strong>Troubleshooting:</strong></p>
            <ul class="list-disc list-inside mt-1 space-y-1">
              <li>Ensure the docs-site nginx proxy is configured to forward requests to mock servers</li>
              <li>Verify mock servers are running: <code class="bg-red-100 px-1 rounded">kubectl get pods -n secret-manager-controller-pact-broker</code></li>
              <li>Check nginx logs if requests are not being proxied correctly</li>
            </ul>
            <p class="mt-2 text-xs">Current endpoint: <code class="bg-red-100 px-1 rounded">{getEndpoint(selectedProvider())}</code></p>
          </div>
        </div>
      </Show>

      {/* Loading State */}
      <Show when={loading()}>
        <div class="text-center py-12">
          <div class="animate-spin rounded-full h-12 w-12 border-2 border-gray-200 border-t-green-600 mx-auto mb-4"></div>
          <p class="text-gray-600">Loading secrets...</p>
        </div>
      </Show>

      {/* Secrets List */}
      <Show when={!loading() && !error()}>
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <h2 class="text-xl font-semibold text-gray-900">
              Secrets ({secrets().length})
            </h2>
          </div>

          <Show when={secrets().length === 0}>
            <div class="bg-gray-50 border border-gray-200 rounded-lg p-8 text-center">
              <svg class="w-12 h-12 text-gray-400 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
              </svg>
              <p class="text-gray-600">No secrets found</p>
              <p class="text-sm text-gray-500 mt-2">
                Secrets will appear here after the controller syncs them from <code class="bg-gray-100 px-1 rounded">application.secrets.env</code> files to the mock providers.
              </p>
              <p class="text-xs text-gray-400 mt-3">
                All values are read directly from the mock server API - no hardcoded test data.
              </p>
            </div>
          </Show>

          <Show when={(selectedProvider() !== 'gcp' || (project() && selectedEnvironment() && selectedLocation())) && secrets().length > 0}>
            <For each={secrets()}>
            {(secret) => (
              <div class="bg-white border border-gray-200 rounded-lg overflow-hidden">
                <div
                  class="p-4 cursor-pointer hover:bg-gray-50 transition-colors"
                  onClick={() => toggleSecret(secret.name)}
                >
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3">
                      <div class={`w-3 h-3 rounded-full ${secret.enabled !== false ? 'bg-green-500' : 'bg-gray-300'}`}></div>
                      <div>
                        <h3 class="font-medium text-gray-900">{secret.name}</h3>
                        <div class="flex flex-wrap items-center gap-3 mt-1 text-sm text-gray-500">
                          <span class="uppercase font-medium">{secret.provider}</span>
                          {secret.project && (
                            <span class="flex items-center gap-1">
                              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
                              </svg>
                              <span>Project: {secret.project}</span>
                            </span>
                          )}
                          {secret.location && (
                            <span class="flex items-center gap-1">
                              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z" />
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 11a3 3 0 11-6 0 3 3 0 016 0z" />
                              </svg>
                              <span>Location: {secret.location}</span>
                            </span>
                          )}
                          {secret.environment && (
                            <span class="flex items-center gap-1">
                              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
                              </svg>
                              <span class="px-2 py-0.5 rounded bg-blue-100 text-blue-800 font-medium">
                                {secret.environment}
                              </span>
                            </span>
                          )}
                          {secret.created && (
                            <span class="flex items-center gap-1">
                              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                              </svg>
                              <span>Created: {new Date(secret.created).toLocaleString()}</span>
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                    <svg
                      class={`w-5 h-5 text-gray-400 transition-transform ${expandedSecrets().has(secret.name) ? 'transform rotate-180' : ''}`}
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                    </svg>
                  </div>
                </div>

                <Show when={expandedSecrets().has(secret.name)}>
                  <div class="border-t border-gray-200 bg-gray-50 p-4 space-y-4">
                    {/* Metadata Grid */}
                    <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                      {secret.project && (
                        <div>
                          <label class="block text-xs font-medium text-gray-500 mb-1">Project</label>
                          <div class="text-sm text-gray-900">{secret.project}</div>
                        </div>
                      )}
                      {secret.location && (
                        <div>
                          <label class="block text-xs font-medium text-gray-500 mb-1">Location</label>
                          <div class="text-sm text-gray-900">{secret.location}</div>
                        </div>
                      )}
                      {secret.environment && (
                        <div>
                          <label class="block text-xs font-medium text-gray-500 mb-1">Environment</label>
                          <div class="text-sm">
                            <span class="px-2 py-1 rounded bg-blue-100 text-blue-800 font-medium">
                              {secret.environment}
                            </span>
                          </div>
                        </div>
                      )}
                      {secret.created && (
                        <div>
                          <label class="block text-xs font-medium text-gray-500 mb-1">Created</label>
                          <div class="text-sm text-gray-900">{new Date(secret.created).toLocaleString()}</div>
                        </div>
                      )}
                    </div>
                    
                    {/* Secret Value */}
                    <Show when={secret.value !== undefined}>
                      <div>
                        <label class="block text-sm font-medium text-gray-700 mb-2">Value</label>
                        <div class="bg-white border border-gray-300 rounded-md p-3 font-mono text-sm break-all">
                          {secret.value}
                        </div>
                      </div>
                    </Show>
                    <Show when={secret.value === undefined}>
                      <p class="text-sm text-gray-500">No value available. Secret may not have any versions yet.</p>
                    </Show>
                  </div>
                </Show>
              </div>
            )}
          </For>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default SecretsViewer;

