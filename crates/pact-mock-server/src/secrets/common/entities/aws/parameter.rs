//! AWS Systems Manager Parameter Store entity
//!
//! AWS Parameter Store stores configuration values (non-secrets)
//! Format: /{prefix}/{environment}/{key}

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "parameters", schema_name = "aws")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub name: String, // Parameter path: /{prefix}/{environment}/{key}
    pub parameter_type: String, // String, StringList, SecureString
    pub description: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub metadata: serde_json::Value,
    /// Environment extracted from metadata.Tags or name format (e.g., "dev", "prod", "pact")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Location extracted from metadata.Tags (e.g., "us-east-1", "eu-west-1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::parameter_version::Entity")]
    Versions,
}

impl Related<super::parameter_version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
