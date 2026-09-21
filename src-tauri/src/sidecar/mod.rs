//! x2t sidecar management.
//!
//! Rules (see harness/skills/x2t-sidecar/SKILL.md):
//! - One short-lived process per conversion task; never a long-running daemon.
//! - Max 1 concurrent task per instance; parallelism = more instances.
//! - Timeout (default 60s) → kill; no zombie processes left behind.
//! - stderr of sidecar is the first evidence on failure → surface it.
//!
//! TODO(ph1): spawn bundled `engines/x2t` with a temp config file,
//! wait with timeout, validate output exists and is non-empty.
