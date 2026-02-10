//! Azure App Configuration version entity
//!
//! Tracks version history of key-value pairs in App Configuration

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "app_config_versions", schema_name = "azure")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub config_key: String,
    #[sea_orm(primary_key)]
    pub version_id: String, // ETag-based version identifier
    pub value: String,
    pub content_type: Option<String>,
    pub label: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub tags: serde_json::Value,
    pub created_at: i64, // Unix timestamp
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::app_config::Entity",
        from = "Column::ConfigKey",
        to = "super::app_config::Column::Key"
    )]
    AppConfig,
}

impl Related<super::app_config::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AppConfig.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
