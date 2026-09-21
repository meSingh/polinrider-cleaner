//! polinrider-cleaner: detect and clean up after the PolinRider supply-chain
//! campaign.
//!
//! The 2.0.0 port, built against the conformance corpus in `conformance/`.
//! An implementation is finished when it agrees with that corpus, not when it
//! compiles. See ADR-0026.

pub mod quarantine;
pub mod verdict;

pub use quarantine::{Apply, DryRun, Outcome, Quarantine};
pub use verdict::{ExitCode, Finding, Level, Verdict};
