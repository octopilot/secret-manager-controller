//! AWS Parameter Store version entity
//!
//! AWS Parameter Store doesn't have explicit versions like secrets,
//! but we track value history for audit purposes

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "parameter_versions", schema_name = "aws")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub parameter_name: String,
    #[sea_orm(primary_key)]
    pub version_id: String, // Auto-incrementing version number
    pub value: String,   // Parameter value
    pub created_at: i64, // Unix timestamp
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::parameter::Entity",
        from = "Column::ParameterName",
        to = "super::parameter::Column::Name"
    )]
    Parameter,
}

impl Related<super::parameter::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Parameter.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
