//! Full-Shor integration layer for the 5-bit ECDLP demo.
//!
//! This module is separate from the scored oracle implementation. It is reserved
//! for combining oracle semantics, QFT/sampling, and secret recovery once that
//! contract is specified.

pub mod oracle_adapter;
pub mod qft;
pub mod recovery;
