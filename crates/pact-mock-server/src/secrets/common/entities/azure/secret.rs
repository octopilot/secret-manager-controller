//! Azure Key Vault SeaORM entities
//!
//! Schema: azure
//! Tables:
//! - secrets: Main secret metadata
//! - versions: Secret versions with data
//! - deleted_secrets: Soft-delete tracking (deleted_date, scheduled_purge_date)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "secrets", schema_name = "azure")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub name: String, // Secret name (no path prefix)
    pub disabled: bool,
    #[sea_orm(column_type = "Json")]
    pub metadata: serde_json::Value,
    /// Environment extracted from metadata.tags (e.g., "dev", "prod", "pact")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Location extracted from metadata.tags (e.g., "eastus", "westus2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::version::Entity")]
    Versions,
    #[sea_orm(has_one = "super::deleted_secret::Entity")]
    DeletedSecret,
}

impl Related<super::version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl Related<super::deleted_secret::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DeletedSecret.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
