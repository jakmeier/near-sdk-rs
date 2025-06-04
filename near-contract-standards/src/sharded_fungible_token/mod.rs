//! A sharded version of fungible tokens.
//!
//! This is a work-in-progress module, developed alongside the protocol standard
//! for sharded contracts in general (github.com/near/NEPs/pull/605/). It will
//! probably become a contract standard and be submitted as a separate NEP.

pub mod call_receiver;
pub mod core;
pub mod core_impl;
pub mod core_root_impl;
pub mod method_version;
