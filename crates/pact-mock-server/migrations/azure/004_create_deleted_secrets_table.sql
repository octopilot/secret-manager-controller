-- Create Azure deleted_secrets table
CREATE TABLE IF NOT EXISTS azure.deleted_secrets (
    secret_name TEXT PRIMARY KEY,
    deleted_date BIGINT NOT NULL,
    scheduled_purge_date BIGINT NOT NULL,
    FOREIGN KEY (secret_name) REFERENCES azure.secrets(name) ON DELETE CASCADE
);

