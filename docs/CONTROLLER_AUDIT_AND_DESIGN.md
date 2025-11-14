# Controller Audit and Design Documentation

## Executive Summary

This document provides a comprehensive audit of the Secret Manager Controller implementation, documenting what was broken, what has been fixed, and the current operational state. It also outlines the future state design with complete flow diagrams.

---

## Part 1: Audit - What Was Broken vs What Works

### Issues Identified and Fixed

#### 1. **HTTP Server Startup Race Condition** ✅ FIXED
**Problem:**
- Readiness probes failed with "connection refused" errors
- Server was marked ready before actually binding to port
- Fixed 200ms delay assumption didn't account for slow startup

**Solution:**
- Implemented polling loop to wait for server to actually bind
- Server marks itself ready when `TcpListener::bind()` succeeds
- 10-second timeout with 50ms poll interval
- Verifies server task hasn't crashed before proceeding

**Status:** ✅ Working - Server ready before controller starts

#### 2. **Missing Resource Validation** ✅ FIXED
**Problem:**
- No validation of required fields before processing
- Could panic on empty strings or missing values
- No early error detection

**Solution:**
- Added validation at start of reconciliation:
  - `sourceRef.kind` must not be empty
  - `sourceRef.name` must not be empty
  - `sourceRef.namespace` must not be empty
  - `secrets.environment` must not be empty
  - `gcp.projectId` must not be empty (for GCP provider)
- Clear error messages for invalid configs

**Status:** ✅ Working - Invalid configs caught early

#### 3. **Panic-Prone Error Handling** ✅ FIXED
**Problem:**
- Used `.unwrap()` calls that could panic
- No graceful error recovery
- Controller could crash on unexpected errors

**Solution:**
- Wrapped reconcile in error handler
- Replaced all `.unwrap()` with proper error handling
- All errors logged and returned gracefully
- Controller continues running even when individual reconciliations fail

**Status:** ✅ Working - No panics, graceful error handling

#### 4. **Missing GitRepository Handling** ✅ FIXED
**Problem:**
- Treated 404 (resource not found) as fatal error
- Incremented error metrics for expected conditions
- No retry logic for missing dependencies

**Solution:**
- Detect 404 errors specifically
- Log as warning (not error) - expected condition
- Return `Action::requeue(30s)` instead of failing
- Don't increment error metrics for 404s
- Automatic retry every 30 seconds

**Status:** ✅ Working - Graceful handling of missing dependencies

#### 5. **Insufficient Debug Logging** ✅ FIXED
**Problem:**
- Limited visibility into reconciliation process
- Hard to debug what resource is being processed
- No validation logging

**Solution:**
- Added comprehensive debug logging:
  - Resource details (name, namespace, sourceRef)
  - Secrets config (environment, prefix, basePath)
  - Provider config (type)
  - Source checking status
  - GitRepository fetch status

**Status:** ✅ Working - Full visibility into reconciliation

---

## Part 2: Current State - Sequence Diagrams and Flow Charts

### Current Startup Sequence

```mermaid
sequenceDiagram
    participant K8s as Kubernetes
    participant Pod as Controller Pod
    participant Server as HTTP Server
    participant Client as K8s Client
    participant CRD as CRD API
    participant Watch as Watch Loop
    participant Reconciler as Reconciler

    Note over Pod: Pod Starts
    Pod->>Server: Create ServerState (is_ready=false)
    Pod->>Server: Spawn server task
    Server->>Server: Bind TcpListener
    Server->>Server: Set is_ready=true
    Server-->>Pod: Server ready signal
    
    Pod->>Pod: Poll server (50ms interval)
    Pod->>Server: Check is_ready flag
    Server-->>Pod: Ready=true
    
    Pod->>Client: Create K8s Client
    Client-->>Pod: Client ready
    
    Pod->>CRD: Check CRD queryability
    CRD-->>Pod: CRD exists and queryable
    
    Pod->>Reconciler: Create Reconciler
    Reconciler->>Reconciler: Load SOPS private key
    Reconciler-->>Pod: Reconciler ready
    
    Pod->>Watch: Start controller watch loop
    Watch->>Watch: Watch for SecretManagerConfig resources
    
    Note over Pod,Watch: Controller Ready - Accepting Reconciliations
```

### Current Reconciliation Flow

```mermaid
flowchart TD
    Start([Controller Detects Resource]) --> Validate{Validate Resource}
    Validate -->|Invalid| Error1[Log Error<br/>Return Error]
    Validate -->|Valid| LogDetails[Log Resource Details]
    
    LogDetails --> CheckSource{Check Source Type}
    CheckSource -->|GitRepository| FetchGitRepo[Fetch GitRepository]
    CheckSource -->|Application| FetchApp[Fetch ArgoCD Application]
    CheckSource -->|Other| Error2[Log Error<br/>Return Error]
    
    FetchGitRepo --> GitRepoExists{GitRepository<br/>Exists?}
    GitRepoExists -->|404 Not Found| Warn404[Log Warning<br/>Requeue in 30s]
    Warn404 --> End1([Wait 30s])
    End1 --> Start
    
    GitRepoExists -->|Error| Error3[Log Error<br/>Return Error]
    GitRepoExists -->|Found| GetArtifactPath[Get Artifact Path]
    
    GetArtifactPath --> ArtifactExists{Artifact Path<br/>Valid?}
    ArtifactExists -->|Error| Error4[Log Error<br/>Return Error]
    ArtifactExists -->|Valid| CreateProvider[Create Cloud Provider Client]
    
    CreateProvider --> ProviderReady{Provider<br/>Ready?}
    ProviderReady -->|Error| Error5[Log Error<br/>Return Error]
    ProviderReady -->|Ready| CheckMode{Processing Mode?}
    
    CheckMode -->|Kustomize| KustomizeMode[Extract Secrets from<br/>Kustomize Build]
    CheckMode -->|Raw Files| RawFileMode[Parse application.secrets.env<br/>Files]
    
    KustomizeMode --> ProcessSecrets[Process Secrets]
    RawFileMode --> ProcessSecrets
    
    ProcessSecrets --> SyncSecrets[Sync to Cloud Provider]
    SyncSecrets --> UpdateStatus[Update Resource Status]
    UpdateStatus --> Success([Reconciliation Complete])
    
    Error1 --> End2([Requeue with Error Handler])
    Error2 --> End2
    Error3 --> End2
    Error4 --> End2
    Error5 --> End2
    
    style Start fill:#e1f5ff
    style Success fill:#d4edda
    style Warn404 fill:#fff3cd
    style Error1 fill:#f8d7da
    style Error2 fill:#f8d7da
    style Error3 fill:#f8d7da
    style Error4 fill:#f8d7da
    style Error5 fill:#f8d7da
```

### Current Error Handling Flow

```mermaid
sequenceDiagram
    participant Watch as Watch Loop
    participant Reconciler as Reconciler
    participant Validator as Validator
    participant K8sAPI as K8s API
    participant ErrorHandler as Error Handler

    Watch->>Reconciler: Resource detected
    Reconciler->>Reconciler: Wrap in error handler
    
    Reconciler->>Validator: Validate required fields
    Validator-->>Reconciler: Validation result
    
    alt Validation Failed
        Reconciler->>Reconciler: Log error
        Reconciler->>ErrorHandler: Return error
        ErrorHandler->>Watch: Requeue after 60s
    else Validation Passed
        Reconciler->>K8sAPI: Fetch GitRepository
        
        alt GitRepository Not Found (404)
            Reconciler->>Reconciler: Log warning (not error)
            Reconciler->>Watch: Return Action::requeue(30s)
            Note over Watch: No error metrics incremented
        else GitRepository Error
            Reconciler->>Reconciler: Log error
            Reconciler->>ErrorHandler: Return error
            ErrorHandler->>Watch: Requeue after 60s
        else GitRepository Found
            Reconciler->>Reconciler: Continue reconciliation
            Note over Reconciler: Process secrets, sync, update status
        end
    end
```

---

## Part 3: Future State - Complete Reconciliation Flow

### Future State Sequence Diagram (Complete Flow)

```mermaid
sequenceDiagram
    participant K8s as Kubernetes
    participant Controller as Controller
    participant GitRepo as GitRepository
    participant Flux as FluxCD Source
    participant SOPS as SOPS Decryptor
    participant Parser as File Parser
    participant Provider as Cloud Provider
    participant Status as Status Updater

    Note over Controller: Resource Detected
    Controller->>Controller: Validate Resource Spec
    
    Controller->>GitRepo: Get GitRepository Resource
    GitRepo-->>Controller: GitRepository with Status
    
    Controller->>GitRepo: Extract Artifact Path
    GitRepo-->>Controller: /tmp/flux-source-{ns}-{name}
    
    Controller->>Flux: Check Artifact Directory
    Flux-->>Controller: Directory exists with files
    
    Controller->>Parser: Find Application Files
    Parser->>Parser: Scan for application.secrets.env
    Parser-->>Controller: List of files found
    
    loop For each application file
        Controller->>Parser: Read file content
        Parser-->>Controller: File content (encrypted or plain)
        
        alt File is SOPS Encrypted
            Controller->>SOPS: Decrypt with GPG key
            SOPS->>SOPS: Import key to temp keyring
            SOPS->>SOPS: Decrypt file
            SOPS-->>Controller: Decrypted content
        else File is Plain Text
            Note over Controller: Use content as-is
        end
        
        Controller->>Parser: Parse key=value pairs
        Parser-->>Controller: Map of secrets
        
        loop For each secret
            Controller->>Provider: Create or Update Secret
            Provider->>Provider: Generate secret name<br/>(prefix-key-suffix)
            Provider->>Provider: Sync to cloud
            Provider-->>Controller: Success
        end
    end
    
    Controller->>Status: Update Resource Status
    Status->>K8s: Patch SecretManagerConfig status
    K8s-->>Status: Status updated
    
    Controller->>Controller: Log success metrics
    Note over Controller: Reconciliation Complete
```

### Future State Flow Chart (Complete Process)

```mermaid
flowchart TD
    Start([Resource Detected]) --> Validate[Validate Resource Spec]
    Validate -->|Invalid| Error1[Log Error<br/>Return Error]
    Validate -->|Valid| LogResource[Log Resource Details]
    
    LogResource --> GetGitRepo[Get GitRepository Resource]
    GetGitRepo --> GitRepoCheck{GitRepository<br/>Exists?}
    
    GitRepoCheck -->|404| WaitGitRepo[Log Warning<br/>Requeue 30s]
    WaitGitRepo --> EndWait([Wait])
    EndWait --> Start
    
    GitRepoCheck -->|Error| Error2[Log Error<br/>Return Error]
    GitRepoCheck -->|Found| ExtractPath[Extract Artifact Path<br/>from Status]
    
    ExtractPath --> PathCheck{Artifact Path<br/>Valid?}
    PathCheck -->|Error| Error3[Log Error<br/>Return Error]
    PathCheck -->|Valid| CheckDir[Check Artifact<br/>Directory Exists]
    
    CheckDir --> DirExists{Directory<br/>Exists?}
    DirExists -->|No| Error4[Log Error<br/>Return Error]
    DirExists -->|Yes| FindFiles[Find Application Files<br/>application.secrets.env]
    
    FindFiles --> FilesFound{Files<br/>Found?}
    FilesFound -->|No| Error5[Log Warning<br/>No secrets to sync]
    FilesFound -->|Yes| ProcessFiles[Process Each File]
    
    ProcessFiles --> ReadFile[Read File Content]
    ReadFile --> CheckSOPS{Is SOPS<br/>Encrypted?}
    
    CheckSOPS -->|Yes| DecryptSOPS[Decrypt with SOPS]
    DecryptSOPS --> DecryptCheck{Decryption<br/>Success?}
    DecryptCheck -->|Error| Error6[Log Error<br/>Skip file]
    DecryptCheck -->|Success| ParseSecrets[Parse key=value pairs]
    
    CheckSOPS -->|No| ParseSecrets
    
    ParseSecrets --> CreateProvider[Create Cloud Provider Client]
    CreateProvider --> ProviderCheck{Provider<br/>Ready?}
    ProviderCheck -->|Error| Error7[Log Error<br/>Return Error]
    ProviderCheck -->|Ready| SyncSecrets[Sync Secrets to Cloud]
    
    SyncSecrets --> SecretLoop{More<br/>Secrets?}
    SecretLoop -->|Yes| SyncSecrets
    SecretLoop -->|No| UpdateStatus[Update Resource Status]
    
    UpdateStatus --> StatusCheck{Status<br/>Updated?}
    StatusCheck -->|Error| Error8[Log Error<br/>Continue]
    StatusCheck -->|Success| UpdateMetrics[Update Metrics]
    
    UpdateMetrics --> Success([Reconciliation Complete])
    
    Error1 --> ErrorHandler[Error Handler<br/>Requeue 60s]
    Error2 --> ErrorHandler
    Error3 --> ErrorHandler
    Error4 --> ErrorHandler
    Error7 --> ErrorHandler
    
    Error5 --> UpdateStatus
    Error6 --> SecretLoop
    Error8 --> UpdateMetrics
    
    style Start fill:#e1f5ff
    style Success fill:#d4edda
    style WaitGitRepo fill:#fff3cd
    style Error1 fill:#f8d7da
    style Error2 fill:#f8d7da
    style Error3 fill:#f8d7da
    style Error4 fill:#f8d7da
    style Error5 fill:#fff3cd
    style Error6 fill:#fff3cd
    style Error7 fill:#f8d7da
    style Error8 fill:#fff3cd
```

### Future State - Complete Reconciliation with Kustomize Mode

```mermaid
sequenceDiagram
    participant Controller as Controller
    participant GitRepo as GitRepository
    participant Kustomize as Kustomize Build
    participant Parser as Secret Parser
    participant Provider as Cloud Provider
    participant Status as Status

    Controller->>GitRepo: Get GitRepository
    GitRepo-->>Controller: Artifact Path
    
    Controller->>Kustomize: Run kustomize build
    Kustomize->>Kustomize: Build with overlays
    Kustomize-->>Controller: Generated YAML
    
    Controller->>Parser: Extract Secret Resources
    Parser->>Parser: Parse Kubernetes Secrets
    Parser-->>Controller: Map of secrets
    
    loop For each secret
        Controller->>Provider: Create/Update Secret
        Provider-->>Controller: Success
    end
    
    Controller->>Status: Update Status
    Status-->>Controller: Updated
```

### Future State - Error Recovery Flow

```mermaid
flowchart TD
    Start([Reconciliation Starts]) --> TryReconcile[Attempt Reconciliation]
    
    TryReconcile --> CatchError{Catch<br/>Error}
    CatchError -->|No Error| Success([Success])
    CatchError -->|Error| ClassifyError{Classify<br/>Error}
    
    ClassifyError -->|404 Not Found| Retry404[Log Warning<br/>Requeue 30s]
    ClassifyError -->|Temporary Error| RetryTemp[Log Warning<br/>Requeue 60s]
    ClassifyError -->|Permanent Error| LogPerm[Log Error<br/>Update Status]
    ClassifyError -->|Panic| CatchPanic[Catch Panic<br/>Log Stack Trace]
    
    Retry404 --> Wait30([Wait 30s])
    RetryTemp --> Wait60([Wait 60s])
    LogPerm --> UpdateErrorStatus[Update Status<br/>with Error]
    CatchPanic --> LogPerm
    
    Wait30 --> Start
    Wait60 --> Start
    UpdateErrorStatus --> End([End])
    Success --> End
    
    style Start fill:#e1f5ff
    style Success fill:#d4edda
    style Retry404 fill:#fff3cd
    style RetryTemp fill:#fff3cd
    style LogPerm fill:#f8d7da
    style CatchPanic fill:#dc3545,color:#fff
```

---

## Part 4: Implementation Status

### ✅ Completed Features

1. **HTTP Server Startup**
   - Polling-based startup verification
   - Readiness probe support
   - Health check endpoints

2. **Resource Validation**
   - Early validation of required fields
   - Clear error messages
   - Prevents invalid configs from being processed

3. **Error Handling**
   - No panics - all errors caught and handled
   - Graceful degradation
   - Proper error logging

4. **Missing Dependency Handling**
   - 404 detection for GitRepository
   - Automatic retry with backoff
   - Warning-level logging for expected conditions

5. **Debug Logging**
   - Comprehensive resource logging
   - Validation logging
   - Source checking status

### 🚧 In Progress / Next Steps

1. **GitRepository Integration**
   - ✅ Detection and fetching
   - ⏳ Artifact path extraction (partial)
   - ⏳ Artifact directory validation

2. **SOPS Decryption**
   - ✅ Key loading from Kubernetes secret
   - ✅ Decryption function exists
   - ⏳ Integration with file processing

3. **Secret Processing**
   - ⏳ File discovery (application.secrets.env)
   - ⏳ SOPS decryption integration
   - ⏳ Key-value parsing
   - ⏳ Cloud provider sync

4. **Status Updates**
   - ⏳ Condition tracking
   - ⏳ Success/failure status
   - ⏳ Metrics reporting

---

## Part 5: Key Design Decisions

### 1. Polling vs Fixed Delays
**Decision:** Use polling with timeout instead of fixed delays
**Rationale:** More reliable, handles slow startup, fails fast on errors

### 2. Warning vs Error for 404s
**Decision:** Log 404s as warnings, not errors
**Rationale:** Expected condition when dependencies don't exist yet, reduces noise in error metrics

### 3. Early Validation
**Decision:** Validate all required fields before processing
**Rationale:** Fail fast, clear error messages, prevents wasted processing

### 4. Error Wrapper
**Decision:** Wrap reconcile function in error handler
**Rationale:** Prevents panics from crashing controller, ensures all errors are logged

### 5. Requeue Strategy
**Decision:** Different requeue delays for different error types
**Rationale:** 
- 30s for missing dependencies (expected, frequent checks)
- 60s for other errors (less frequent, avoid thundering herd)

---

## Part 6: Metrics and Observability

### Current Metrics
- Reconciliation attempts
- Reconciliation errors (excluding 404s)
- Reconciliation duration
- Secrets managed count

### Future Metrics (Planned)
- GitRepository fetch success/failure
- SOPS decryption success/failure
- File processing counts
- Cloud provider sync success/failure
- Status update success/failure

---

## Conclusion

The controller has been significantly improved with proper error handling, validation, and graceful dependency management. The current implementation provides a solid foundation for the complete reconciliation flow, with clear separation of concerns and robust error recovery.

The future state diagrams show the complete end-to-end flow that will be implemented, including SOPS decryption, secret processing, and cloud provider synchronization.

