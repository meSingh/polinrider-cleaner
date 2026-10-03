//! polinrider-cleaner: detect and clean up after the PolinRider supply-chain
//! campaign.
//!
//! The 2.0.0 port, built against the conformance corpus in `conformance/`.
//! An implementation is finished when it agrees with that corpus, not when it
//! compiles. See ADR-0026.

pub mod checks;
pub mod cli;
pub mod guide;
mod guide_fix;
mod guide_github;
pub mod host;
pub mod host_checks;
pub mod indicators;
pub mod pattern;
pub mod quarantine;
pub mod remote;
pub mod remote_fix;
pub mod scan;
pub mod sha256;
pub mod strip;
pub mod ui;
pub mod verdict;
pub mod walk;

pub use host::{Host, LiveHost, Platform, Probe, Snapshot};
pub use indicators::Indicators;
pub use quarantine::{Apply, DryRun, Outcome, Quarantine};
pub use verdict::{Entry, ExitCode, Finding, Kind, Level, Verdict};
