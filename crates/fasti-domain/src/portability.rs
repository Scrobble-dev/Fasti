//! B3 portability policy values.
//!
//! These values describe the portable promise. Archive headers, filesystem
//! paths, locks, compression implementations, and activation syscalls belong
//! to adapters.

use serde::{Deserialize, Serialize};

pub const ARCHIVE_ZSTD_LEVEL: i32 = 3;
pub const ARCHIVE_MAX_ZSTD_WINDOW_BYTES: u64 = 4 * 1024 * 1024;
pub const ARCHIVE_MAX_IO_CHUNK_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportScope {
    FullWorkspace,
}

impl ExportScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullWorkspace => "full_workspace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveProfile {
    #[serde(rename = "zstd-l3-w22")]
    ZstdL3W22,
}

impl ArchiveProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ZstdL3W22 => "zstd-l3-w22",
        }
    }

    pub const fn compression_level(self) -> i32 {
        match self {
            Self::ZstdL3W22 => ARCHIVE_ZSTD_LEVEL,
        }
    }

    pub const fn max_window_bytes(self) -> u64 {
        match self {
            Self::ZstdL3W22 => ARCHIVE_MAX_ZSTD_WINDOW_BYTES,
        }
    }

    pub const fn max_io_chunk_bytes(self) -> usize {
        match self {
            Self::ZstdL3W22 => ARCHIVE_MAX_IO_CHUNK_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestorePolicy {
    CleanOnly,
}

impl RestorePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CleanOnly => "clean_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryGrantPolicy {
    RequireFreshBootstrap,
}

impl RecoveryGrantPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequireFreshBootstrap => "require_fresh_bootstrap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreStatus {
    Received,
    Staging,
    Verified,
    Activating,
    Complete,
    Rejected,
}

impl RestoreStatus {
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Received, Self::Staging | Self::Rejected)
                | (Self::Staging, Self::Verified | Self::Rejected)
                | (Self::Verified, Self::Activating | Self::Rejected)
                | (Self::Activating, Self::Complete | Self::Rejected)
        )
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_profile_is_the_fixed_b3_profile() {
        let profile = ArchiveProfile::ZstdL3W22;
        assert_eq!(profile.as_str(), "zstd-l3-w22");
        assert_eq!(profile.compression_level(), 3);
        assert_eq!(profile.max_window_bytes(), 4 * 1024 * 1024);
        assert_eq!(profile.max_io_chunk_bytes(), 256 * 1024);
    }

    #[test]
    fn restore_state_machine_has_two_terminal_outcomes() {
        assert!(RestoreStatus::Received.can_transition_to(RestoreStatus::Staging));
        assert!(RestoreStatus::Staging.can_transition_to(RestoreStatus::Verified));
        assert!(RestoreStatus::Verified.can_transition_to(RestoreStatus::Activating));
        assert!(RestoreStatus::Activating.can_transition_to(RestoreStatus::Complete));
        assert!(RestoreStatus::Staging.can_transition_to(RestoreStatus::Rejected));
        assert!(!RestoreStatus::Complete.can_transition_to(RestoreStatus::Staging));
        assert!(RestoreStatus::Complete.is_terminal());
        assert!(RestoreStatus::Rejected.is_terminal());
    }

    #[test]
    fn portable_policies_have_one_unambiguous_wire_value() {
        assert_eq!(ExportScope::FullWorkspace.as_str(), "full_workspace");
        assert_eq!(RestorePolicy::CleanOnly.as_str(), "clean_only");
        assert_eq!(
            RecoveryGrantPolicy::RequireFreshBootstrap.as_str(),
            "require_fresh_bootstrap"
        );
    }
}
