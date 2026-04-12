//! Lightweight semver comparison helpers for capability versioning.

/// Tri-state semver comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOrdering {
    /// Left version is older.
    Older,
    /// Identical.
    Equal,
    /// Left version is newer.
    Newer,
    /// Could not parse one or both versions.
    Invalid,
}

/// Compare two version strings of the form `MAJOR.MINOR.PATCH`.
pub fn compare(a: &str, b: &str) -> VersionOrdering {
    let pa = parse(a);
    let pb = parse(b);
    match (pa, pb) {
        (Some(a), Some(b)) => match a.cmp(&b) {
            std::cmp::Ordering::Less => VersionOrdering::Older,
            std::cmp::Ordering::Equal => VersionOrdering::Equal,
            std::cmp::Ordering::Greater => VersionOrdering::Newer,
        },
        _ => VersionOrdering::Invalid,
    }
}

fn parse(s: &str) -> Option<(u32, u32, u32)> {
    let mut parts = s.trim_start_matches('v').split('.');
    let maj: u32 = parts.next()?.parse().ok()?;
    let min: u32 = parts.next()?.parse().ok()?;
    let pat: u32 = parts.next().unwrap_or("0").parse().ok()?;
    Some((maj, min, pat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering_basic() {
        assert_eq!(compare("1.0.0", "1.0.1"), VersionOrdering::Older);
        assert_eq!(compare("2.0.0", "1.9.9"), VersionOrdering::Newer);
        assert_eq!(compare("1.2.3", "1.2.3"), VersionOrdering::Equal);
        assert_eq!(compare("not-a-version", "1.0.0"), VersionOrdering::Invalid);
    }
}
