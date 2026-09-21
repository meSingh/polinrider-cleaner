//! The `polinrider` binary. Still a stub: the engine lands check by check,
//! each one gated on the conformance corpus.

use std::process::ExitCode as ProcExit;

fn main() -> ProcExit {
    eprintln!("polinrider 2.0.0-alpha.1 — not yet wired up.");
    eprintln!("The shell implementation is the working tool: ./polinrider.sh");
    // 3: could not run. Never 0, so nothing mistakes a stub for a clean scan.
    ProcExit::from(polinrider::ExitCode::CouldNotRun.code() as u8)
}
