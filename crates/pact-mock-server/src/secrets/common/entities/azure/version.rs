//! Azure Key Vault version entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "versions", schema_name = "azure")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub secret_name: String,
    #[sea_orm(primary_key)]
    pub version_id: String, // UUID-like: "a1b2c3d4..."
    #[sea_orm(column_type = "Json")]
    pub data: serde_json::Value,
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::secret::Entity",
        from = "Column::SecretName",
        to = "super::secret::Column::Name"
    )]
    Secret,
}

impl Related<super::secret::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Secret.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
