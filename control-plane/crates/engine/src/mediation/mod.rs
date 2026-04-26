//! P017 Phase B: Mediation settlement service and Phase B lead resolver.
//!
//! MediationSettlementService is the engine-owned boundary through which all
//! mediation settlements must pass. Direct write access from GraphQL, MCP server,
//! or DB repos calling TransitionAuthorityResolver is forbidden.
//!
//! PhaseBLeadResolver provides exactly-one fail-closed lead resolution during
//! Phase B using a versioned JSON compatibility map.

pub mod feature_flag;
pub mod lead_resolver;
pub mod phase_c_validator;
pub mod settlement;
