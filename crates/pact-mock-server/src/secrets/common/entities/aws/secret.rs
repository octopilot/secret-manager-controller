//! AWS Secrets Manager SeaORM entities
//!
//! Schema: aws
//! Tables:
//! - secrets: Main secret metadata
//! - versions: Secret versions with data
//! - staging_labels: Maps staging labels (AWSCURRENT, AWSPREVIOUS) to version IDs

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "secrets", schema_name = "aws")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub name: String, // Secret name (no path prefix)
    pub disabled: bool,
    #[sea_orm(column_type = "Json")]
    pub metadata: serde_json::Value,
    /// Environment extracted from metadata.Tags (e.g., "dev", "prod", "pact")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Location extracted from metadata.Tags or ARN (e.g., "us-east-1", "eu-west-1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::version::Entity")]
    Versions,
    #[sea_orm(has_many = "super::staging_label::Entity")]
    StagingLabels,
}

impl Related<super::version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl Related<super::staging_label::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StagingLabels.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
