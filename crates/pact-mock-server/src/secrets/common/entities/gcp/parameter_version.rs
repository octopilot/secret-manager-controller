//! GCP Parameter Manager version entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "parameter_versions", schema_name = "gcp")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub parameter_key: String,
    #[sea_orm(primary_key)]
    pub version_id: String, // User-provided: "v1234567890"
    #[sea_orm(column_type = "Json")]
    pub data: serde_json::Value,
    pub created_at: i64, // Unix timestamp
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::parameter::Entity",
        from = "Column::ParameterKey",
        to = "super::parameter::Column::Key"
    )]
    Parameter,
}

impl Related<super::parameter::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Parameter.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
