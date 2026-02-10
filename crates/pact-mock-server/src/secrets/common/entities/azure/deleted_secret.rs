//! Azure Key Vault deleted secret entity
//!
//! Tracks soft-deleted secrets with deletion date and scheduled purge date

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "deleted_secrets", schema_name = "azure")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub secret_name: String,
    pub deleted_date: i64,         // Unix timestamp when deleted
    pub scheduled_purge_date: i64, // Unix timestamp when it will be purged (default: 90 days)
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
