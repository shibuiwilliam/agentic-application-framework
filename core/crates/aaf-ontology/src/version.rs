//! Entity version compatibility.

use crate::entity::EntityVersion;

/// Compatibility between two entity versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionCompatibility {
    /// Downstream consumers need zero changes.
    Compatible,
    /// New fields / relations were added; consumers may adopt them
    /// lazily.
    Additive,
    /// Breaking schema change; consumers must migrate.
    Breaking,
}

/// Compare two entity versions and classify the change direction.
/// `new` is what a registrar is trying to land, `current` is what is
/// already in the registry.
pub fn compare_versions(current: EntityVersion, new: EntityVersion) -> VersionCompatibility {
    if new.major > current.major {
        VersionCompatibility::Breaking
    } else if new.major == current.major && new.minor > current.minor {
        VersionCompatibility::Additive
    } else if new.major == current.major && new.minor == current.minor && new.patch >= current.patch
    {
        VersionCompatibility::Compatible
    } else {
        // Any older-than-current change is treated as breaking so the
        // registry forces an explicit rollback.
        VersionCompatibility::Breaking
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_bump_is_compatible() {
        assert_eq!(
            compare_versions(EntityVersion::new(1, 2, 3), EntityVersion::new(1, 2, 4)),
            VersionCompatibility::Compatible
        );
    }

    #[test]
    fn minor_bump_is_additive() {
        assert_eq!(
            compare_versions(EntityVersion::new(1, 2, 3), EntityVersion::new(1, 3, 0)),
            VersionCompatibility::Additive
        );
    }

    #[test]
    fn major_bump_is_breaking() {
        assert_eq!(
            compare_versions(EntityVersion::new(1, 2, 3), EntityVersion::new(2, 0, 0)),
            VersionCompatibility::Breaking
        );
    }

    #[test]
    fn downgrade_is_breaking() {
        assert_eq!(
            compare_versions(EntityVersion::new(1, 5, 0), EntityVersion::new(1, 4, 9)),
            VersionCompatibility::Breaking
        );
    }
}
