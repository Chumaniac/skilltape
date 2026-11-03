//! Shared source/delivery inventory ceilings, not process memory or disk quotas.
pub const ARTIFACT_MAX_ENTRIES: u64 = 10_000;
pub const ARTIFACT_MAX_DEPTH: u64 = 64;
pub const ARTIFACT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const ARTIFACT_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
