# Postgres Manager

The `postgres-manager` is a sidecar container that manages PostgreSQL database migrations for the Pact mock servers. It ensures database schema is up-to-date and handles migration lifecycle automatically.

## Overview

The `postgres-manager` runs alongside the PostgreSQL container in the Pact broker deployment and is responsible for:

- **Database Creation**: Ensures the target database (`pact_mock_servers`) exists before migrations run
- **Migration Execution**: Runs SQL migrations from ConfigMap in the correct order
- **ConfigMap Watching**: Monitors ConfigMap changes and re-runs migrations when needed
- **Health Monitoring**: Provides HTTP health endpoints for Kubernetes probes
- **Idempotent Operations**: Safely handles re-runs and partial failures

## Architecture

### Deployment Structure

The `postgres-manager` runs as a sidecar container in the PostgreSQL pod:

```yaml
spec:
  containers:
    - name: postgres
      image: docker.io/casibbald/postgres:17-duckdb-supabase-v2
      # ... PostgreSQL configuration ...
    
    - name: postgres-manager
      image: postgres-manager
      # ... Manager configuration ...
```

### Container Responsibilities

1. **PostgreSQL Container**: Runs the actual database server
2. **Postgres-Manager Container**: Manages migrations and database lifecycle
3. **Init Containers**: Prepare directories and process ConfigMap

## How It Works

### Startup Sequence

1. **Init Container (`create-dirs`)**: Creates PostgreSQL data directory with proper permissions
2. **Init Container (`prepare-migrations`)**: Processes ConfigMap and organizes migration files into schema directories (`gcp/`, `aws/`, `azure/`)
3. **PostgreSQL Container**: Starts and initializes database
4. **Postgres-Manager Container**:
   - Starts HTTP health server (for liveness/readiness probes)
   - Waits for PostgreSQL to be ready
   - Ensures target database exists
   - Runs migrations in order
   - Watches ConfigMap for changes

### Migration Discovery

Migrations are discovered from the prepared directory structure:

```
/shared/postgres-migrations/
├── gcp/
│   ├── 001_create_schema.sql
│   ├── 002_create_secrets_table.sql
│   └── ...
├── aws/
│   ├── 001_create_schema.sql
│   └── ...
└── azure/
    ├── 001_create_schema.sql
    └── ...
```

Migrations are sorted by key (`schema/filename`) to ensure consistent execution order.

### Database Creation

Before running migrations, the manager ensures the target database exists:

```rust
async fn ensure_database_exists(config: &ManagerConfig) -> Result<()> {
    // Connect to default 'postgres' database
    // Check if 'pact_mock_servers' exists
    // Create if missing
}
```

This handles cases where PostgreSQL is initialized but the target database wasn't created.

### Migration Execution

Each migration file is executed as a single transaction:

```rust
async fn run_migration_file(db: &Database, path: &Path) -> Result<()> {
    let sql = std::fs::read_to_string(path)?;
    let stmt = Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        sql
    );
    db.execute(stmt).await?;
    Ok(())
}
```

Migrations are executed sequentially, and failures are logged but don't stop the process (to allow partial recovery).

### ConfigMap Watching

The manager watches the ConfigMap for changes:

```rust
let watcher = watcher(configmaps, Config::default());
pin_mut!(stream);
while let Some(event) = stream.next().await {
    match event {
        Event::Applied(cm) => {
            // ConfigMap changed, re-run migrations
            run_migrations(&config).await?;
        }
        // ...
    }
}
```

When the ConfigMap changes, migrations are re-run automatically. The manager tracks which migrations have been applied to avoid unnecessary re-execution.

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgresql://pact:pact@localhost:5432/pact_mock_servers` | PostgreSQL connection string |
| `NAMESPACE` | `secret-manager-controller-pact-broker` | Kubernetes namespace |
| `CONFIGMAP_NAME` | `postgres-migrations` | ConfigMap name containing migrations |
| `CONFIGMAP_PATH` | `/migrations-configmap` | Path where ConfigMap is mounted |
| `POSTGRES_HOST` | `localhost` | PostgreSQL host |
| `POSTGRES_PORT` | `5432` | PostgreSQL port |
| `HEALTH_PORT` | `1239` | HTTP health server port |

### Health Endpoints

The manager provides HTTP health endpoints:

- **`GET /liveness`**: Returns 200 if manager is running (always available)
- **`GET /readiness`**: Returns 200 if PostgreSQL is ready AND migrations are applied

These endpoints are used by Kubernetes liveness and readiness probes.

### Probe Configuration

```yaml
livenessProbe:
  httpGet:
    path: /liveness
    port: 1239
  initialDelaySeconds: 30
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /readiness
    port: 1239
  initialDelaySeconds: 90
  periodSeconds: 5

startupProbe:
  httpGet:
    path: /liveness
    port: 1239
  initialDelaySeconds: 2
  periodSeconds: 2
  timeoutSeconds: 1
  failureThreshold: 60
```

## Migration Files

### Migration Structure

Migrations are organized by provider schema:

- **GCP**: `gcp/001_create_schema.sql`, `gcp/002_create_secrets_table.sql`, etc.
- **AWS**: `aws/001_create_schema.sql`, `aws/002_create_secrets_table.sql`, etc.
- **Azure**: `azure/001_create_schema.sql`, `azure/002_create_secrets_table.sql`, etc.

### Migration Naming

Migrations use a numbered prefix for ordering:

- `001_*`: Schema creation
- `002_*`: Initial tables
- `003_*`: Additional tables
- `010_*`: Test data (optional)
- `011_*`: Constraints and indexes

### Idempotent Migrations

Migrations should be idempotent (safe to run multiple times):

```sql
-- Good: Uses IF NOT EXISTS
CREATE TABLE IF NOT EXISTS gcp.secrets (
    -- ...
);

-- Good: Drops constraint before adding
ALTER TABLE gcp.secrets
DROP CONSTRAINT IF EXISTS chk_gcp_secrets_environment_not_empty;

ALTER TABLE gcp.secrets
ADD CONSTRAINT chk_gcp_secrets_environment_not_empty
CHECK (environment != '');
```

## Troubleshooting

### Migrations Not Running

**Problem**: Migrations don't execute on startup.

**Check**:
1. Verify ConfigMap exists: `kubectl get configmap postgres-migrations -n secret-manager-controller-pact-broker`
2. Check init container logs: `kubectl logs postgres-<pod-id> -n secret-manager-controller-pact-broker -c prepare-migrations`
3. Verify migrations directory: `kubectl exec postgres-<pod-id> -n secret-manager-controller-pact-broker -c postgres-manager -- ls -la /shared/postgres-migrations`

**Solution**: Ensure ConfigMap is populated and init container completed successfully.

### Database Connection Errors

**Problem**: `postgres-manager` can't connect to PostgreSQL.

**Check**:
1. Verify PostgreSQL is running: `kubectl logs postgres-<pod-id> -n secret-manager-controller-pact-broker -c postgres`
2. Check `DATABASE_URL` environment variable
3. Verify network connectivity within pod

**Solution**: Ensure PostgreSQL container started before manager attempts connection.

### Migration Failures

**Problem**: Specific migrations fail with errors.

**Check**:
1. View manager logs: `kubectl logs postgres-<pod-id> -n secret-manager-controller-pact-broker -c postgres-manager`
2. Check PostgreSQL logs for detailed errors
3. Verify migration SQL syntax

**Solution**: Fix migration SQL and update ConfigMap. Manager will re-run on next ConfigMap change.

### Health Probe Failures

**Problem**: Readiness probe fails, pod marked as not ready.

**Check**:
1. Verify health endpoint: `kubectl exec postgres-<pod-id> -n secret-manager-controller-pact-broker -c postgres-manager -- curl http://localhost:1239/readiness`
2. Check if migrations completed: Look for "✅ All migrations applied" in logs
3. Verify PostgreSQL is ready: `kubectl exec postgres-<pod-id> -n secret-manager-controller-pact-broker -c postgres -- pg_isready`

**Solution**: Increase `readinessProbe.initialDelaySeconds` if migrations take longer than expected.

## Development

### Building the Manager

```bash
# Build postgres-manager binary
cargo build --release --bin postgres-manager -p pact-mock-server

# Build Docker image
docker build -f dockerfiles/Dockerfile.postgres-manager -t postgres-manager:dev .
```

### Testing Locally

```bash
# Run manager locally (requires PostgreSQL)
DATABASE_URL="postgresql://pact:pact@localhost:5432/pact_mock_servers" \
NAMESPACE="default" \
CONFIGMAP_NAME="postgres-migrations" \
cargo run --bin postgres-manager -p pact-mock-server
```

### Adding New Migrations

1. Create migration file in `crates/pact-mock-server/migrations/<schema>/<number>_<description>.sql`
2. Update ConfigMap (via Tilt or manually)
3. Manager will automatically detect and run new migrations

### Migration Best Practices

1. **Use Transactions**: Wrap migrations in transactions when possible
2. **Be Idempotent**: Use `IF NOT EXISTS`, `DROP IF EXISTS`, etc.
3. **Test Backward Compatibility**: Ensure migrations work on existing data
4. **Document Breaking Changes**: Add comments for schema changes
5. **Version Constraints**: Add version checks if needed

## Related Documentation

- [Kind Cluster Setup](./kind-cluster-setup.md) - Local development environment
- [Tilt Integration](./tilt-integration.md) - Using Tilt for development
- [Pact Testing Architecture](../testing/pact-testing/architecture.md) - Pact infrastructure overview

