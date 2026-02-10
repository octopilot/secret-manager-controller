//! Azure App Configuration entity
//!
//! Azure App Configuration stores key-value pairs for configuration
//! Format: {prefix}:{environment}:{key}

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "app_config", schema_name = "azure")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub key: String, // Format: {prefix}:{environment}:{key}
    pub value: String,
    pub content_type: Option<String>,
    pub label: Option<String>, // Optional label for key-value
    #[sea_orm(column_type = "Json")]
    pub tags: serde_json::Value, // JSON object of tags
    /// Environment extracted from tags or key format (e.g., "dev", "prod", "pact")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Location extracted from tags (e.g., "eastus", "westus2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::app_config_version::Entity")]
    Versions,
}

impl Related<super::app_config_version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
