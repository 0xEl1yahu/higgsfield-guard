//! Library target. The hook binary (`rust/src/main.rs`) and the review binary
//! (`rust/src/bin/review.rs`) both build on these modules; neither may duplicate them.
//!
//! Nothing here reads stdin, writes stdout, or decides an exit code - those are the
//! binaries' jobs, and they have opposite obligations. The hook must be silent on every
//! path (SPEC 7); the review tool is allowed to talk.

pub mod checks;
pub mod lint;
pub mod log;
pub mod models;
