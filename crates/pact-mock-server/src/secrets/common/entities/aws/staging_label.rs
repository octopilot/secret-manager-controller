//! AWS Secrets Manager staging label entity
//!
//! Maps staging labels (AWSCURRENT, AWSPREVIOUS) to version IDs

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "staging_labels", schema_name = "aws")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub secret_name: String,
    #[sea_orm(primary_key)]
    pub label: String, // "AWSCURRENT", "AWSPREVIOUS", or custom labels
    pub version_id: String, // Points to a version_id in the versions table
    pub created_at: i64,
    pub updated_at: i64,
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
