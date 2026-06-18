//! Unscored QFT and sampling support for the 5-bit Shor ECDLP demo.
//!
//! This module is intentionally separate from the scored oracle track. It will
//! hold statevector/QFT helpers used to demonstrate end-to-end Shor recovery
//! without changing the oracle leaderboard contract.

pub mod recovery;
pub mod sampling;
pub mod statevector;
