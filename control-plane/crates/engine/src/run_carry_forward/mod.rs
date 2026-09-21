//! Canonical carry-forward composition, with separately configured admission.

pub mod approval;
mod budget;
mod finalize;
mod git;
pub mod inputs;
mod inventory;
pub mod launch;
pub mod manifest;
mod materialize;
pub mod preview;
pub mod readback;
mod safe_files;
mod service;
mod worker;

pub use finalize::CarryForwardFinalizer;
pub use inventory::{
    InventoryLimits, PreviewOptions, WorkspaceEntry, WorkspacePlanner, WorkspacePreview,
    WorkspaceReplacement,
};
pub use manifest::{read_manifest, read_verified_manifest, ManifestV1};
pub use materialize::{materialize, MaterializationReceiptV1};
pub use service::{CarryForwardService, ServiceConfig, ServiceError};
pub use worker::{
    PreparationContext, PreparationFinalizer, PreparationJob, PreparationLane, PreparationObserver,
    PreparationPermit, PreparationTicket, PreparedManifest,
};
