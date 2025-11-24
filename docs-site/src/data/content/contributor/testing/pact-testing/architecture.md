# Pact Testing Architecture

Comprehensive architecture documentation for the Pact testing infrastructure, including detailed component diagrams, sequence flows, and design rationale.

## Component Architecture

### Deployment Structure

```mermaid
graph TB
    subgraph "pact-infrastructure Pod"
        subgraph "Init Containers"
            InitDB[init-pact-db<br/>Creates /pacts directory<br/>chmod 777]
        end
        
        subgraph "Main Containers"
            Broker[pact-broker<br/>Port: 9292<br/>SQLite Database]
            Manager[manager<br/>Port: 1238<br/>Contract Publisher]
            Webhook[mock-webhook<br/>Port: 1237<br/>Webhook Receiver]
            AWSMock[aws-mock-server<br/>Port: 1234<br/>AWS API Mock]
            GCPMock[gcp-mock-server<br/>Port: 1235<br/>GCP API Mock]
            AzureMock[azure-mock-server<br/>Port: 1236<br/>Azure API Mock]
        end
        
        subgraph "Volumes"
            PactsStorage[(pacts-storage<br/>emptyDir<br/>SQLite DB)]
            ConfigMapVol[(pacts-configmap-volume<br/>ConfigMap<br/>pact-contracts)]
            TmpVol[(tmp-volume<br/>emptyDir<br/>Published Flag)]
        end
    end
    
    subgraph "Kubernetes Resources"
        ConfigMap[pact-contracts<br/>ConfigMap<br/>Pact JSON Files]
        ServiceAccount[pact-manager<br/>ServiceAccount]
        Role[pact-manager<br/>Role<br/>ConfigMap Access]
        RoleBinding[pact-manager<br/>RoleBinding]
    end
    
    subgraph "Services"
        BrokerSvc[pact-broker:9292]
        AWSSvc[aws-mock-server:1234]
        GCPSvc[gcp-mock-server:1234]
        AzureSvc[azure-mock-server:1234]
        WebhookSvc[mock-webhook:1237]
    end
    
    InitDB -->|Creates| PactsStorage
    Broker -->|Reads/Writes| PactsStorage
    Manager -->|Reads| ConfigMapVol
    Manager -->|Writes Flag| TmpVol
    ConfigMapVol -.->|Mounted From| ConfigMap
    ServiceAccount -->|Bound To| RoleBinding
    RoleBinding -->|References| Role
    Manager -->|Uses| ServiceAccount
    
    BrokerSvc -->|Routes To| Broker
    AWSSvc -->|Routes To| AWSMock
    GCPSvc -->|Routes To| GCPMock
    AzureSvc -->|Routes To| AzureMock
    WebhookSvc -->|Routes To| Webhook
```

### Port Assignments

| Component | Container Port | Service Port | Purpose |
|-----------|---------------|--------------|---------|
| **pact-broker** | 9292 | 9292 | HTTP API for contract storage |
| **manager** | 1238 | - | Health endpoints (liveness, readiness, ready) |
| **mock-webhook** | 1237 | 1237 | Webhook receiver for testing |
| **aws-mock-server** | 1234 | 1234 | AWS Secrets Manager API mock |
| **gcp-mock-server** | 1235 | 1234 | GCP Secret Manager API mock |
| **azure-mock-server** | 1236 | 1234 | Azure Key Vault API mock |

**Note**: All mock server services use port 1234 externally, but route to different container ports internally.

### Network Topology

```mermaid
graph LR
    subgraph "Pod Network Namespace"
        Broker[Broker<br/>:9292]
        Manager[Manager<br/>:1238]
        AWS[AWS Mock<br/>:1234]
        GCP[GCP Mock<br/>:1235]
        Azure[Azure Mock<br/>:1236]
    end
    
    subgraph "Cluster Network"
        Controller[Secret Manager<br/>Controller]
        Tests[Pact Tests<br/>Port Forward]
    end
    
    Manager -->|localhost:9292| Broker
    AWS -->|localhost:9292| Broker
    GCP -->|localhost:9292| Broker
    Azure -->|localhost:9292| Broker
    AWS -->|localhost:1238| Manager
    GCP -->|localhost:1238| Manager
    Azure -->|localhost:1238| Manager
    
    Controller -->|Service DNS| AWS
    Controller -->|Service DNS| GCP
    Controller -->|Service DNS| Azure
    Tests -->|Port Forward| Broker
    Tests -->|Port Forward| AWS
    Tests -->|Port Forward| GCP
    Tests -->|Port Forward| Azure
```

**Key Points**:
- All containers share the pod network namespace
- Components communicate via `localhost` within the pod
- External access via Kubernetes Services
- Port forwarding for local test execution

## Sequence Diagrams

### Startup Sequence

```mermaid
sequenceDiagram
    participant K8s as Kubernetes
    participant Init as init-pact-db
    participant Broker as pact-broker
    participant Manager as manager
    participant AWS as aws-mock-server
    participant GCP as gcp-mock-server
    participant Azure as azure-mock-server
    
    K8s->>Init: Start init container
    Init->>Init: mkdir -p /pacts
    Init->>Init: chmod 777 /pacts
    Init->>K8s: Complete
    
    par Parallel Container Startup
        K8s->>Broker: Start broker container
        Broker->>Broker: Initialize SQLite DB
        Broker->>Broker: Start Puma server
        Broker->>K8s: Ready (heartbeat)
    and
        K8s->>Manager: Start manager container
        Manager->>Manager: Initialize HTTP server
        Manager->>Broker: Wait for broker (poll)
        Broker-->>Manager: Health check OK
        Manager->>Manager: Check ConfigMap
        Manager->>Broker: Publish contracts
        Broker-->>Manager: Contracts published
        Manager->>Manager: Set pacts_published=true
        Manager->>K8s: Ready (/readiness)
    and
        K8s->>AWS: Start AWS mock server
        AWS->>Broker: Wait for broker (poll)
        Broker-->>AWS: Health check OK
        AWS->>Manager: Wait for manager (/ready)
        Manager-->>AWS: Pacts published
        AWS->>Broker: Load contracts
        Broker-->>AWS: Contract data
        AWS->>AWS: Start Axum server
        AWS->>K8s: Ready (/health)
    and
        K8s->>GCP: Start GCP mock server
        GCP->>Broker: Wait for broker (poll)
        Broker-->>GCP: Health check OK
        GCP->>Manager: Wait for manager (/ready)
        Manager-->>GCP: Pacts published
        GCP->>Broker: Load contracts
        Broker-->>GCP: Contract data
        GCP->>GCP: Start Axum server
        GCP->>K8s: Ready (/health)
    and
        K8s->>Azure: Start Azure mock server
        Azure->>Broker: Wait for broker (poll)
        Broker-->>Azure: Health check OK
        Azure->>Manager: Wait for manager (/ready)
        Manager-->>Azure: Pacts published
        Azure->>Broker: Load contracts
        Broker-->>Azure: Contract data
        Azure->>Azure: Start Axum server
        Azure->>K8s: Ready (/health)
    end
```

### Contract Publishing Flow

```mermaid
sequenceDiagram
    participant Tests as Pact Tests
    participant ConfigMap as pact-contracts<br/>ConfigMap
    participant Manager as manager
    participant Broker as pact-broker
    participant AWS as aws-mock-server
    participant GCP as gcp-mock-server
    participant Azure as azure-mock-server
    
    Tests->>Tests: Run cargo test --test pact_*
    Tests->>Tests: Generate Pact JSON files
    Tests->>ConfigMap: Create/Update ConfigMap<br/>with Pact files
    
    Manager->>Manager: Watch ConfigMap (kube-runtime)
    Manager->>ConfigMap: Detect change
    Manager->>Broker: Check broker health
    Broker-->>Manager: Health OK
    
    Manager->>ConfigMap: Read Pact files
    ConfigMap-->>Manager: Pact JSON data
    
    loop For each provider
        Manager->>Broker: POST /pacts/provider/{provider}/consumer/{consumer}
        Manager->>Broker: Include Pact JSON body
        Broker-->>Manager: 201 Created
    end
    
    Manager->>Manager: Set pacts_published=true
    Manager->>Manager: Update /ready endpoint
    
    par Mock Servers Poll Manager
        AWS->>Manager: GET /ready
        Manager-->>AWS: {status: "ready", published_providers: ["AWS-Secrets-Manager"]}
        AWS->>Broker: GET /pacts/provider/AWS-Secrets-Manager/consumer/Secret-Manager-Controller/latest
        Broker-->>AWS: Pact JSON
        AWS->>AWS: Load contracts into memory
    and
        GCP->>Manager: GET /ready
        Manager-->>GCP: {status: "ready", published_providers: ["GCP-Secret-Manager"]}
        GCP->>Broker: GET /pacts/provider/GCP-Secret-Manager/consumer/Secret-Manager-Controller/latest
        Broker-->>GCP: Pact JSON
        GCP->>GCP: Load contracts into memory
    and
        Azure->>Manager: GET /ready
        Manager-->>Azure: {status: "ready", published_providers: ["Azure-Key-Vault"]}
        Azure->>Broker: GET /pacts/provider/Azure-Key-Vault/consumer/Secret-Manager-Controller/latest
        Broker-->>Azure: Pact JSON
        Azure->>Azure: Load contracts into memory
    end
```

### ConfigMap Watch Flow

```mermaid
sequenceDiagram
    participant Dev as Developer
    participant ConfigMap as pact-contracts<br/>ConfigMap
    participant Manager as manager
    participant Broker as pact-broker
    participant MockServers as Mock Servers
    
    Dev->>Dev: Run tests (generate new contracts)
    Dev->>ConfigMap: Update ConfigMap<br/>with new Pact files
    
    Manager->>Manager: kube-runtime watcher<br/>detects ConfigMap change
    Manager->>ConfigMap: Read updated Pact files
    ConfigMap-->>Manager: New/updated Pact JSON
    
    Manager->>Broker: Check broker health
    Broker-->>Manager: Health OK
    
    Manager->>Broker: Re-publish contracts<br/>POST /pacts/provider/{provider}/consumer/{consumer}
    Broker-->>Manager: 201 Created (or 200 Updated)
    
    Manager->>Manager: Update published_providers set
    Manager->>Manager: Update /ready endpoint
    
    Note over MockServers: Mock servers can reload contracts<br/>on next request or via health check
    MockServers->>Broker: GET /pacts/provider/{provider}/consumer/{consumer}/latest
    Broker-->>MockServers: Updated Pact JSON
    MockServers->>MockServers: Reload contracts
```

### Test Execution Flow

```mermaid
sequenceDiagram
    participant Test as Pact Test
    participant Controller as Secret Manager<br/>Controller
    participant MockServer as Mock Server<br/>(AWS/GCP/Azure)
    participant Broker as pact-broker
    
    Test->>Test: Set PACT_MODE=true
    Test->>Test: Set provider endpoint<br/>(e.g., GCP_SECRET_MANAGER_ENDPOINT)
    
    Test->>Controller: Create SecretManagerConfig
    Controller->>Controller: Reconcile triggered
    
    Controller->>Controller: Process Git artifacts
    Controller->>Controller: Extract secrets
    
    Controller->>MockServer: POST /v1/projects/{project}/secrets<br/>(Create secret request)
    
    MockServer->>MockServer: Match request to contract
    MockServer->>MockServer: Find matching interaction
    
    alt Contract Matches
        MockServer->>MockServer: Generate response from contract
        MockServer-->>Controller: 200 OK<br/>{contract response}
        Controller->>Controller: Process response
        Controller->>Controller: Update status
    else No Match
        MockServer-->>Controller: 500 Internal Server Error<br/>{error: "No matching interaction"}
        Controller->>Controller: Handle error
    end
    
    Test->>Controller: Verify status/behavior
    Test->>Test: Assert test expectations
```

### Manager Sidecar Workflow

```mermaid
sequenceDiagram
    participant Manager as manager
    participant Broker as pact-broker
    participant ConfigMap as pact-contracts<br/>ConfigMap
    participant K8s as Kubernetes API
    
    Manager->>Manager: Initialize HTTP server<br/>(/liveness, /readiness, /ready)
    Manager->>Broker: Poll broker health<br/>(every 2 seconds)
    
    loop Until Broker Ready
        Broker-->>Manager: Not ready / Connection refused
        Manager->>Manager: Wait 2 seconds
    end
    
    Broker-->>Manager: Health check OK
    
    Manager->>K8s: Watch ConfigMap<br/>(kube-runtime watcher)
    K8s-->>Manager: ConfigMap events
    
    alt ConfigMap Exists
        Manager->>ConfigMap: Read Pact files
        ConfigMap-->>Manager: Pact JSON data
        
        loop For each provider
            Manager->>Broker: POST /pacts/provider/{provider}/consumer/{consumer}
            Broker-->>Manager: 201 Created
        end
        
        Manager->>Manager: Set pacts_published=true
        Manager->>Manager: Add providers to published_providers set
    else ConfigMap Missing
        Manager->>Manager: Log info (expected state)
        Manager->>Manager: Keep pacts_published=false
    end
    
    loop Continuous Monitoring
        Manager->>Broker: Health check (every 30 seconds)
        Broker-->>Manager: Health status
        Manager->>Manager: Update broker_healthy flag
        
        Manager->>K8s: Watch ConfigMap changes
        K8s-->>Manager: ConfigMap updated
        Manager->>ConfigMap: Read updated Pact files
        Manager->>Broker: Re-publish contracts
        Manager->>Manager: Update published_providers
    end
```

### Mock Server Startup

```mermaid
sequenceDiagram
    participant MockServer as Mock Server
    participant Broker as pact-broker
    participant Manager as manager
    
    MockServer->>MockServer: Start application
    MockServer->>MockServer: Initialize Axum router
    
    MockServer->>Broker: wait_for_broker_and_pacts()<br/>(max 90 seconds)
    
    loop Every 2 seconds
        MockServer->>Broker: GET /diagnostic/status/heartbeat
        Broker-->>MockServer: 200 OK or Connection Error
    end
    
    Broker-->>MockServer: Health check OK
    
    MockServer->>Manager: wait_for_manager_ready()<br/>(max 90 seconds)
    
    loop Every 2 seconds
        MockServer->>Manager: GET /ready
        Manager-->>MockServer: {status, broker_healthy, pacts_published, published_providers}
    end
    
    alt Pacts Published
        Manager-->>MockServer: {status: "ready", pacts_published: true, published_providers: ["Provider-Name"]}
        MockServer->>Broker: GET /pacts/provider/{provider}/consumer/{consumer}/latest
        Broker-->>MockServer: Pact JSON
        MockServer->>MockServer: Parse contracts
        MockServer->>MockServer: Build interaction map
        MockServer->>MockServer: Start HTTP server
        MockServer->>MockServer: Ready to serve requests
    else Timeout
        MockServer->>MockServer: Log error
        MockServer->>MockServer: Exit with error
    end
```

## Call Flow Diagrams

### Controller → Mock Server Interaction (PACT_MODE)

```mermaid
flowchart TD
    Start[Controller Reconciles<br/>SecretManagerConfig]
    CheckMode{Check PACT_MODE<br/>Environment Variable}
    
    PACTMode[PACT_MODE=true]
    ProdMode[PACT_MODE=false]
    
    PACTMode --> GetEndpoint[Get Provider Endpoint<br/>from Environment<br/>e.g., GCP_SECRET_MANAGER_ENDPOINT]
    ProdMode --> UseDefault[Use Default Provider Endpoint<br/>e.g., secretmanager.googleapis.com]
    
    GetEndpoint --> MakeRequest[Make HTTP Request<br/>to Mock Server Endpoint]
    UseDefault --> MakeProdRequest[Make HTTP Request<br/>to Real Provider API]
    
    MakeRequest --> MockServer[Mock Server Receives Request]
    MakeProdRequest --> RealProvider[Real Provider API]
    
    MockServer --> MatchContract{Match Request<br/>to Contract}
    
    MatchContract -->|Match Found| GenerateResponse[Generate Response<br/>from Contract]
    MatchContract -->|No Match| ErrorResponse[Return 500 Error<br/>No matching interaction]
    
    GenerateResponse --> ReturnResponse[Return Contract Response<br/>Status, Headers, Body]
    ErrorResponse --> ReturnError[Return Error Response]
    
    ReturnResponse --> ControllerProcess[Controller Processes Response]
    ReturnError --> ControllerError[Controller Handles Error]
    
    ControllerProcess --> UpdateStatus[Update SecretManagerConfig Status]
    ControllerError --> UpdateErrorStatus[Update Status with Error]
    
    UpdateStatus --> End[Reconciliation Complete]
    UpdateErrorStatus --> End
```

### Manager → Broker Interaction

```mermaid
flowchart TD
    Start[Manager Starts]
    InitServer[Initialize HTTP Server<br/>Port 1238]
    WaitBroker[Wait for Broker<br/>Poll /diagnostic/status/heartbeat]
    
    WaitBroker --> BrokerReady{Broker<br/>Ready?}
    BrokerReady -->|No| WaitBroker
    BrokerReady -->|Yes| CheckConfigMap[Check ConfigMap<br/>pact-contracts]
    
    CheckConfigMap --> ConfigMapExists{ConfigMap<br/>Exists?}
    ConfigMapExists -->|No| WatchConfigMap[Watch ConfigMap<br/>kube-runtime watcher]
    ConfigMapExists -->|Yes| ReadPacts[Read Pact Files<br/>from ConfigMap]
    
    ReadPacts --> ParsePacts[Parse Pact JSON Files]
    ParsePacts --> PublishLoop[For Each Provider]
    
    PublishLoop --> PublishContract["POST /pacts/provider/{provider}/consumer/{consumer}"<br/>Include Pact JSON Body]
    PublishContract --> BrokerResponse{Broker<br/>Response}
    
    BrokerResponse -->|201 Created| Success[Contract Published]
    BrokerResponse -->|200 OK| Updated[Contract Updated]
    BrokerResponse -->|Error| LogError[Log Error<br/>Continue]
    
    Success --> NextProvider{More<br/>Providers?}
    Updated --> NextProvider
    LogError --> NextProvider
    
    NextProvider -->|Yes| PublishLoop
    NextProvider -->|No| UpdateStatus[Update Manager Status<br/>pacts_published=true<br/>Add to published_providers]
    
    UpdateStatus --> WatchLoop[Watch ConfigMap<br/>for Changes]
    WatchLoop --> ConfigMapChange{ConfigMap<br/>Changed?}
    ConfigMapChange -->|Yes| ReadPacts
    ConfigMapChange -->|No| HealthCheck[Health Check Broker<br/>Every 30 seconds]
    
    HealthCheck --> WatchLoop
```

### Manager → ConfigMap Interaction

```mermaid
flowchart TD
    Start[Manager Initializes]
    CreateWatcher[Create kube-runtime Watcher<br/>Watch ConfigMap: pact-contracts]
    
    CreateWatcher --> WatchEvents[Watch for Events]
    WatchEvents --> EventReceived{Event<br/>Received?}
    
    EventReceived -->|No| WatchEvents
    EventReceived -->|Yes| CheckEventType{Event<br/>Type?}
    
    CheckEventType -->|Added| ReadConfigMap[Read ConfigMap<br/>Get all Pact files]
    CheckEventType -->|Modified| ReadConfigMap
    CheckEventType -->|Deleted| LogInfo[Log Info<br/>ConfigMap deleted<br/>Expected state]
    
    ReadConfigMap --> ParseFiles[Parse Pact Files<br/>Extract provider names]
    ParseFiles --> PublishContracts[Publish Contracts<br/>to Broker]
    
    PublishContracts --> UpdatePublished[Update published_providers set]
    UpdatePublished --> UpdateReady[Update /ready endpoint<br/>Include published providers]
    
    UpdateReady --> WatchEvents
    LogInfo --> WatchEvents
```

### Mock Server → Broker Interaction

```mermaid
flowchart TD
    Start[Mock Server Starts]
    WaitBroker[wait_for_broker_and_pacts<br/>Max 90 seconds]
    
    WaitBroker --> CheckBroker["Check Broker Health <br/> GET /diagnostic/status/heartbeat"]
    CheckBroker --> BrokerReady{Broker<br/>Ready?}
    
    BrokerReady -->|No| Wait2Sec[Wait 2 seconds]
    Wait2Sec --> CheckBroker
    
    BrokerReady -->|Yes| WaitManager[Wait for Manager<br/>GET /ready]
    WaitManager --> ManagerReady{Manager<br/>Ready?}
    
    ManagerReady -->|No| Wait2Sec2[Wait 2 seconds]
    Wait2Sec2 --> WaitManager
    
    ManagerReady -->|Yes| CheckPacts{Provider in<br/>published_providers?}
    CheckPacts -->|No| Wait2Sec2
    CheckPacts -->|Yes| LoadContracts["Load Contracts<br/>GET /pacts/provider/{provider}/consumer/{consumer}/latest"]
    
    LoadContracts --> BrokerResponse{Broker<br/>Response}
    BrokerResponse -->|200 OK| ParseContracts[Parse Pact JSON<br/>Extract interactions]
    BrokerResponse -->|404 Not Found| RetryLoad[Retry Loading<br/>Wait 2 seconds]
    BrokerResponse -->|Error| LogError[Log Error<br/>Exit]
    
    RetryLoad --> LoadContracts
    ParseContracts --> BuildInteractions[Build Interaction Map<br/>Request → Response]
    BuildInteractions --> StartServer[Start Axum HTTP Server<br/>Ready to serve]
    
    StartServer --> ServeRequests[Serve API Requests<br/>Match to contracts]
    LogError --> End[Exit with Error]
```

### Test → Controller → Mock Server → Broker Flow

```mermaid
flowchart TD
    Start[Pact Test Starts]
    SetupPact[Setup Pact Mode<br/>PACT_MODE=true<br/>Set provider endpoints]
    
    SetupPact --> CreateConfig[Create SecretManagerConfig<br/>in Kubernetes]
    CreateConfig --> TriggerReconcile[Controller Reconciles]
    
    TriggerReconcile --> ProcessGit[Process Git Artifacts<br/>Extract Secrets]
    ProcessGit --> CallProvider[Call Provider API<br/>Create/Update Secret]
    
    CallProvider --> CheckMode{PACT_MODE<br/>Enabled?}
    CheckMode -->|Yes| RouteToMock[Route to Mock Server<br/>via endpoint override]
    CheckMode -->|No| RouteToReal[Route to Real Provider<br/>Production API]
    
    RouteToMock --> MockServer[Mock Server Receives Request]
    RouteToReal --> RealProvider[Real Provider API]
    
    MockServer --> MatchRequest[Match Request to Contract<br/>Method, Path, Headers, Body]
    MatchRequest --> FoundMatch{Match<br/>Found?}
    
    FoundMatch -->|Yes| GetResponse[Get Response from Contract<br/>Status, Headers, Body]
    FoundMatch -->|No| ReturnError[Return 500 Error<br/>No matching interaction]
    
    GetResponse --> ReturnResponse[Return Contract Response]
    ReturnError --> ReturnErrorResponse[Return Error Response]
    
    ReturnResponse --> ControllerProcess[Controller Processes Response]
    ReturnErrorResponse --> ControllerError[Controller Handles Error]
    
    ControllerProcess --> UpdateStatus[Update Status]
    ControllerError --> UpdateErrorStatus[Update Error Status]
    
    UpdateStatus --> TestVerify[Test Verifies Behavior]
    UpdateErrorStatus --> TestVerify
    
    TestVerify --> Assert[Assert Expectations]
    Assert --> End[Test Complete]
```

## State Diagrams

### Manager State Machine

```mermaid
stateDiagram-v2
    [*] --> Initializing: Manager Starts
    
    Initializing --> WaitingForBroker: Initialize HTTP Server
    
    WaitingForBroker --> CheckingBroker: Poll Broker Health<br/>(every 2 seconds)
    CheckingBroker --> WaitingForBroker: Broker Not Ready
    CheckingBroker --> BrokerReady: Broker Health OK
    
    BrokerReady --> CheckingConfigMap: Check ConfigMap Exists
    CheckingConfigMap --> WatchingConfigMap: ConfigMap Missing<br/>(Expected)
    CheckingConfigMap --> ReadingPacts: ConfigMap Exists
    
    ReadingPacts --> PublishingContracts: Parse Pact Files
    PublishingContracts --> ContractsPublished: Publish to Broker
    
    ContractsPublished --> WatchingConfigMap: Update Status<br/>pacts_published=true
    
    WatchingConfigMap --> ConfigMapChanged: ConfigMap Event<br/>(Added/Modified)
    WatchingConfigMap --> HealthChecking: Periodic Health Check<br/>(every 30 seconds)
    
    ConfigMapChanged --> ReadingPacts: Re-read ConfigMap
    HealthChecking --> WatchingConfigMap: Update broker_healthy
    
    WatchingConfigMap --> [*]: Shutdown
```

### Mock Server State Machine

```mermaid
stateDiagram-v2
    [*] --> Starting: Mock Server Starts
    
    Starting --> WaitingForBroker: Initialize Application
    
    WaitingForBroker --> CheckingBroker: Poll Broker Health<br/>(every 2 seconds, max 90s)
    CheckingBroker --> WaitingForBroker: Broker Not Ready
    CheckingBroker --> BrokerReady: Broker Health OK
    
    BrokerReady --> WaitingForManager: Wait for Manager<br/>GET /ready
    
    WaitingForManager --> CheckingManager: Poll Manager<br/>(every 2 seconds, max 90s)
    CheckingManager --> WaitingForManager: Manager Not Ready<br/>or Pacts Not Published
    CheckingManager --> ManagerReady: Manager Ready<br/>Provider in published_providers
    
    ManagerReady --> LoadingContracts: Load Contracts<br/>GET /pacts/provider/{provider}/consumer/{consumer}/latest
    
    LoadingContracts --> ParsingContracts: Receive Pact JSON
    ParsingContracts --> BuildingInteractions: Parse Interactions
    BuildingInteractions --> Ready: Build Interaction Map<br/>Start HTTP Server
    
    Ready --> Serving: Serve API Requests
    Serving --> MatchingRequest: Receive Request
    MatchingRequest --> FoundMatch: Match to Contract
    MatchingRequest --> NoMatch: No Match Found
    
    FoundMatch --> GeneratingResponse: Get Response from Contract
    GeneratingResponse --> ReturningResponse: Return Response
    ReturningResponse --> Serving: Continue Serving
    
    NoMatch --> ReturningError: Return 500 Error
    ReturningError --> Serving: Continue Serving
    
    Serving --> [*]: Shutdown
```

### Contract Publishing State

```mermaid
stateDiagram-v2
    [*] --> NotPublished: Contracts Not Published
    
    NotPublished --> Publishing: Manager Detects ConfigMap<br/>or ConfigMap Change
    
    Publishing --> PublishingToBroker: POST /pacts/provider/{provider}/consumer/{consumer}
    PublishingToBroker --> BrokerResponse: Broker Processes Request
    
    BrokerResponse --> Published: 201 Created<br/>Contract Published
    BrokerResponse --> Updated: 200 OK<br/>Contract Updated
    BrokerResponse --> Error: 4xx/5xx Error
    
    Published --> Published: Contract Available<br/>Mock Servers Can Load
    Updated --> Published: Contract Updated<br/>Mock Servers Can Reload
    
    Error --> NotPublished: Log Error<br/>Retry on Next ConfigMap Change
    
    Published --> RePublishing: ConfigMap Changed<br/>Manager Detects Update
    RePublishing --> PublishingToBroker: Re-publish Contracts
    
    Published --> [*]: Contracts Available
```

## Architecture Rationale

### Why Combined Deployment?

All Pact infrastructure components are deployed in a single `pact-infrastructure` pod:

**Benefits**:
- **Reduced Startup Time**: Single pod vs multiple deployments (faster CI/CD)
- **Simplified Orchestration**: One resource to manage, single Service
- **Better Resource Utilization**: Shared volumes, shared networking
- **Faster Tilt Startup**: One deployment to wait for
- **Localhost Communication**: Components communicate via `localhost` (no network overhead)

**Trade-offs**:
- **Single Point of Failure**: If pod crashes, all components go down (acceptable for testing)
- **Resource Limits**: All components share pod resource limits (sufficient for testing)

### Why Manager Sidecar?

The manager is a Rust sidecar container that handles contract publishing:

**Benefits over Init Containers**:
- **Dynamic Updates**: Can handle ConfigMap changes without pod restart
- **Health Monitoring**: Provides health endpoints for Kubernetes probes
- **Better Reliability**: More robust than one-time init container execution
- **Observability**: Can monitor and report publishing status

**Responsibilities**:
- Watch Pact broker for readiness
- Publish contracts from ConfigMap to broker
- Watch ConfigMap for changes and re-publish
- Provide health endpoints (`/liveness`, `/readiness`, `/ready`)

### Why ConfigMap Watching?

The manager watches the `pact-contracts` ConfigMap for changes:

**Benefits**:
- **Dynamic Updates**: Contracts can be updated without pod restart
- **Reliability**: Manager ensures contracts are published before mock servers start
- **Observability**: Manager provides health endpoints to track publishing status
- **Flexibility**: Contracts can be updated during development without redeploying

**Implementation**:
- Uses `kube-runtime` watcher for efficient event-driven updates
- Re-publishes contracts when ConfigMap changes
- Tracks published providers in memory

### Port Assignment Rationale

Each component uses a unique port:

| Component | Port | Rationale |
|-----------|------|-----------|
| Broker | 9292 | Standard Pact Broker port |
| Manager | 1238 | Health endpoints, separate from mock servers |
| AWS Mock | 1234 | Standard HTTP port for AWS API |
| GCP Mock | 1235 | Avoid conflict with AWS, sequential numbering |
| Azure Mock | 1236 | Avoid conflict with others, sequential numbering |
| Webhook | 1237 | Separate from mock servers |

**Service Port Mapping**:
- All mock server services expose port 1234 externally
- Route to different container ports internally (1234, 1235, 1236)
- Simplifies controller configuration (all use port 1234)

### Service Account and RBAC Setup

The manager uses a dedicated ServiceAccount with minimal permissions:

**ServiceAccount**: `pact-manager`
- Bound to `pact-infrastructure` pod
- Used by manager sidecar only

**Role Permissions**:
- `list` ConfigMaps (required for watcher)
- `get`, `watch` on `pact-contracts` ConfigMap only
- No write permissions (read-only access)

**Rationale**:
- **Least Privilege**: Only permissions needed for ConfigMap reading
- **Security**: No write access to prevent accidental modifications
- **Isolation**: Manager can't access other resources

## Component Details

### Pact Broker

**Purpose**: Central repository for storing and managing Pact contracts

**Image**: `pactfoundation/pact-broker:latest`

**Configuration**:
- **Database**: SQLite (stored in `/pacts` volume)
- **Port**: 9292
- **Authentication**: Basic auth (username: `pact`, password: `pact`)
- **Public Access**: Heartbeat endpoint public, read access public

**Endpoints**:
- `/diagnostic/status/heartbeat` - Health check (public)
- `/pacts/provider/{provider}/consumer/{consumer}/latest` - Get latest contract
- `/pacts/provider/{provider}/consumer/{consumer}` - Publish contract (POST)

**Storage**:
- SQLite database in `/pacts/pact_broker.sqlite`
- Ephemeral storage (emptyDir) for testing
- Persists contracts in memory during pod lifetime

### Manager Sidecar

**Purpose**: Manages Pact infrastructure lifecycle

**Image**: `pact-mock-server` (Rust binary: `/app/manager`)

**Responsibilities**:
1. **Broker Monitoring**: Polls broker health endpoint
2. **Contract Publishing**: Publishes contracts from ConfigMap to broker
3. **ConfigMap Watching**: Watches for ConfigMap changes
4. **Health Reporting**: Provides health endpoints for Kubernetes probes

**Endpoints**:
- `/liveness` - Liveness probe (broker healthy)
- `/readiness` - Readiness probe (broker healthy AND pacts published)
- `/ready` - Ready check (broker healthy AND pacts published, includes published_providers list)

**State Management**:
- `broker_healthy`: AtomicBool (broker health status)
- `pacts_published`: AtomicBool (contracts published status)
- `published_providers`: RwLock<HashSet<String>> (track published providers)

### Mock Servers

**Purpose**: Serve mock APIs based on contracts loaded from the broker

**Implementation**: Rust/Axum HTTP servers

**Mock Servers**:
- **AWS Mock Server**: Port 1234, replicates AWS Secrets Manager REST API
- **GCP Mock Server**: Port 1235, replicates GCP Secret Manager REST API v1
- **Azure Mock Server**: Port 1236, replicates Azure Key Vault REST API

**Startup Sequence**:
1. Wait for broker to be ready (health check)
2. Wait for manager to confirm pacts are published (`/ready` endpoint)
3. Load contracts from broker
4. Parse contracts and build interaction map
5. Start HTTP server and serve requests

**Contract Matching**:
- Matches requests to contract interactions by:
  - HTTP method
  - Path (exact or pattern)
  - Headers (if specified)
  - Body (if specified)
- Returns contract response on match
- Returns 500 error if no match found

**Features**:
- Request logging middleware
- Rate limiting middleware (via `X-Rate-Limit` header)
- Service unavailable middleware (via `X-Service-Unavailable` header)
- Authentication failure middleware (via `X-Auth-Failure` header)

### Mock Webhook

**Purpose**: Webhook receiver for testing webhook integrations

**Image**: `mock-webhook` (separate image)

**Port**: 1237

**Features**:
- Receives webhook POST requests
- Logs webhook payloads
- Returns success responses
- Used for testing webhook-based integrations

### ConfigMap Structure

**Name**: `pact-contracts`

**Namespace**: `secret-manager-controller-pact-broker`

**Structure**:
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: pact-contracts
  namespace: secret-manager-controller-pact-broker
data:
  secret-manager-controller-gcp-secret-manager.json: |
    {
      "consumer": {"name": "Secret-Manager-Controller"},
      "provider": {"name": "GCP-Secret-Manager"},
      "interactions": [...]
    }
  secret-manager-controller-aws-secrets-manager.json: |
    {
      "consumer": {"name": "Secret-Manager-Controller"},
      "provider": {"name": "AWS-Secrets-Manager"},
      "interactions": [...]
    }
  # ... other provider contracts
```

**Volume Mount**:
- Mounted at `/pacts-configmap` in manager container
- Read-only access
- Optional (may not exist if contracts haven't been generated)

## Network Communication

### Within Pod (localhost)

All containers share the pod network namespace:

- **Broker**: `http://localhost:9292`
- **Manager**: `http://localhost:1238`
- **Mock Servers**: `http://localhost:1234/1235/1236`

**Benefits**:
- No network overhead
- No DNS resolution needed
- Fast communication
- Simplified configuration

### External Access (Services)

Kubernetes Services provide external access:

- **Broker Service**: `pact-broker:9292`
- **Mock Server Services**: `aws-mock-server:1234`, `gcp-mock-server:1234`, `azure-mock-server:1234`
- **Webhook Service**: `mock-webhook:1237`

**Controller Access**:
```yaml
env:
  - name: GCP_SECRET_MANAGER_ENDPOINT
    value: "http://gcp-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234"
```

### Port Forwarding (Local Testing)

For local test execution:

```bash
kubectl port-forward -n secret-manager-controller-pact-broker svc/pact-broker 9292:9292
kubectl port-forward -n secret-manager-controller-pact-broker svc/gcp-mock-server 1235:1234
```

Tests connect to `localhost:9292` and `localhost:1235`.

## Health Checks and Probes

### Broker Probes

**Startup Probe**:
- Path: `/diagnostic/status/heartbeat`
- Initial Delay: 30s
- Period: 15s
- Timeout: 5s
- Failure Threshold: 5

**Readiness Probe**:
- Path: `/diagnostic/status/heartbeat`
- Initial Delay: 15s
- Period: 30s
- Timeout: 3s
- Failure Threshold: 3

**Liveness Probe**:
- Path: `/diagnostic/status/heartbeat`
- Initial Delay: 30s
- Period: 30s
- Timeout: 3s
- Failure Threshold: 3

### Manager Probes

**Liveness Probe**:
- Path: `/liveness`
- Port: 1238
- Returns 200 if broker is healthy

**Readiness Probe**:
- Path: `/readiness`
- Port: 1238
- Returns 200 if broker is healthy AND pacts are published

**Ready Endpoint**:
- Path: `/ready`
- Port: 1238
- Returns JSON with status, broker_healthy, pacts_published, published_providers

### Mock Server Probes

**Startup Probe**:
- Path: `/health`
- Initial Delay: 60s (allows time for broker and manager)
- Period: 10s
- Timeout: 3s
- Failure Threshold: 15 (allows up to 3.5 minutes total)

**Readiness Probe**:
- Path: `/health`
- Initial Delay: 2s
- Period: 30s
- Timeout: 3s
- Failure Threshold: 3

**Liveness Probe**:
- Path: `/health`
- Initial Delay: 10s
- Period: 30s
- Timeout: 3s
- Failure Threshold: 3

## Resource Requirements

### Broker

```yaml
resources:
  requests:
    memory: 128Mi
    cpu: 50m
  limits:
    memory: 256Mi
    cpu: 200m
```

### Manager

```yaml
resources:
  requests:
    memory: 64Mi
    cpu: 50m
  limits:
    memory: 128Mi
    cpu: 100m
```

### Mock Servers

```yaml
resources:
  requests:
    memory: 64Mi
    cpu: 50m
  limits:
    memory: 128Mi
    cpu: 100m
```

**Total Pod Resources**:
- **Requests**: ~512Mi memory, 350m CPU
- **Limits**: ~896Mi memory, 700m CPU

## Troubleshooting

### Broker Not Starting

**Symptoms**: Broker container keeps restarting

**Diagnosis**:
```bash
kubectl logs -n secret-manager-controller-pact-broker -l app=pact-infrastructure -c pact-broker
```

**Common Issues**:
- Database directory not created (check init container)
- Port conflict (unlikely in pod)
- Resource constraints

### Manager Not Publishing

**Symptoms**: Mock servers wait indefinitely for contracts

**Diagnosis**:
```bash
kubectl logs -n secret-manager-controller-pact-broker -l app=pact-infrastructure -c manager
```

**Common Issues**:
- ConfigMap doesn't exist (expected if contracts not generated)
- Broker not ready (check broker logs)
- RBAC permissions (check ServiceAccount, Role, RoleBinding)

### Mock Servers Not Starting

**Symptoms**: Mock server containers fail startup probe

**Diagnosis**:
```bash
kubectl logs -n secret-manager-controller-pact-broker -l app=pact-infrastructure -c gcp-mock-server
```

**Common Issues**:
- Broker not ready (check broker health)
- Manager not ready (check manager `/ready` endpoint)
- Contracts not published (check manager logs)
- Timeout waiting (increase timeout or check broker/manager)

### Contracts Not Loading

**Symptoms**: Mock servers start but return "No matching interaction"

**Diagnosis**:
```bash
# Check if contracts are published
curl http://localhost:9292/pacts/provider/GCP-Secret-Manager/consumer/Secret-Manager-Controller/latest

# Check manager status
curl http://localhost:1238/ready
```

**Common Issues**:
- Contracts not published (check manager logs)
- Wrong provider/consumer names (check contract files)
- Contracts expired or deleted (re-publish)

## Next Steps

- [Pact Testing Overview](./overview.md) - Pact concepts and workflow
- [Pact Testing Setup](./setup.md) - Setting up Pact infrastructure
- [Writing Pact Tests](./writing-tests.md) - How to write contract tests

