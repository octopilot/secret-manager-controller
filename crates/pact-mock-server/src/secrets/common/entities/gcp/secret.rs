//! GCP Secret Manager SeaORM entities
//!
//! Schema: gcp
//! Tables:
//! - secrets: Main secret metadata
//! - versions: Secret versions with data
//! - parameters: GCP Parameter Manager parameters (separate from secrets)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "secrets", schema_name = "gcp")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub key: String, // Format: "projects/{project}/secrets/{secret}"
    pub disabled: bool,
    #[sea_orm(column_type = "Json")]
    pub metadata: serde_json::Value,
    /// Environment extracted from metadata.labels (e.g., "dev", "prod", "pact")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Location extracted from metadata.labels (e.g., "us-central1", "global")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub created_at: i64, // Unix timestamp
    pub updated_at: i64, // Unix timestamp
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::version::Entity")]
    Versions,
}

impl Related<super::version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
