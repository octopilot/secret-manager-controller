//! GCP Secret Manager version entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "versions", schema_name = "gcp")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub secret_key: String,
    #[sea_orm(primary_key)]
    pub version_id: String, // Sequential: "1", "2", "3", ...
    #[sea_orm(column_type = "Json")]
    pub data: serde_json::Value,
    pub enabled: bool,
    pub created_at: i64, // Unix timestamp
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::secret::Entity",
        from = "Column::SecretKey",
        to = "super::secret::Column::Key"
    )]
    Secret,
}

impl Related<super::secret::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Secret.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
