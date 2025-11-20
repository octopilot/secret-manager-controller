//! # Notifications
//!
//! Handles sending notifications when drift is detected.
//! Supports both FluxCD (via Alert CRD) and ArgoCD (via Application annotations).

mod argocd;
mod fluxcd;

pub use argocd::{remove_argocd_notifications, send_argocd_notification};
pub use fluxcd::{ensure_fluxcd_alert, remove_fluxcd_alert};
