//! GCP Parameter Manager entity
//!
//! Separate from secrets, GCP has a Parameter Manager service
//! Format: "projects/{project}/locations/{location}/parameters/{parameter}"

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "parameters", schema_name = "gcp")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub key: String, // Format: "projects/{project}/locations/{location}/parameters/{parameter}"
    #[sea_orm(column_type = "Json")]
    pub metadata: serde_json::Value,
    /// Environment extracted from metadata.labels (e.g., "dev", "prod", "pact")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Location extracted from key format: projects/{project}/locations/{location}/parameters/{parameter}
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
