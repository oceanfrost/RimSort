//! Package names, configuration tokens and installation identity are distinct.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// A normalized About.xml package name. Source suffixes are not stripped here;
/// interpret ModsConfig.xml strings with [`PackageToken`] instead.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPackageId;

impl fmt::Display for InvalidPackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("package ID must be non-empty and contain no whitespace")
    }
}

impl std::error::Error for InvalidPackageId {}

impl PackageId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, InvalidPackageId> {
        let value = value.as_ref().trim();
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(InvalidPackageId);
        }
        Ok(Self(value.to_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PackageId {
    type Err = InvalidPackageId;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// The normalized spelling of a ModsConfig.xml token is preserved alongside its
/// canonical package name. Local/Steam are preferences, not installation IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackagePreference {
    Default,
    Local,
    Steam,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageToken {
    package_id: PackageId,
    preference: PackagePreference,
}

impl PackageToken {
    pub fn new(package_id: PackageId, preference: PackagePreference) -> Self {
        Self {
            package_id,
            preference,
        }
    }

    pub fn package_id(&self) -> &PackageId {
        &self.package_id
    }

    pub fn preference(&self) -> PackagePreference {
        self.preference
    }
}

impl FromStr for PackageToken {
    type Err = InvalidPackageId;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_lowercase();
        let (name, preference) = if let Some(name) = normalized.strip_suffix("_steam") {
            (name, PackagePreference::Steam)
        } else if let Some(name) = normalized.strip_suffix("_local") {
            (name, PackagePreference::Local)
        } else {
            (normalized.as_str(), PackagePreference::Default)
        };
        Ok(Self::new(PackageId::new(name)?, preference))
    }
}

impl fmt::Display for PackageToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.preference {
            PackagePreference::Default => "",
            PackagePreference::Local => "_local",
            PackagePreference::Steam => "_steam",
        };
        write!(f, "{}{suffix}", self.package_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModSource {
    Expansion,
    Local,
    Workshop,
    SteamCmd,
    Git,
}

/// An installation is identified by its absolute directory, not package name.
/// No filesystem access, hashing or case folding is performed. Discovery must
/// resolve aliases/symlinks before construction if it needs deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModId(PathBuf);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeModPath;

impl fmt::Display for RelativeModPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("mod installation path must be absolute")
    }
}

impl std::error::Error for RelativeModPath {}

impl ModId {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, RelativeModPath> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(RelativeModPath);
        }
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Steam Workshop IDs are numeric and non-zero, independently of package IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublishedFileId(std::num::NonZeroU64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPublishedFileId;

impl fmt::Display for InvalidPublishedFileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Workshop ID must be a non-zero unsigned integer")
    }
}

impl std::error::Error for InvalidPublishedFileId {}
impl PublishedFileId {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl FromStr for PublishedFileId {
    type Err = InvalidPublishedFileId;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
            return Err(InvalidPublishedFileId);
        }
        let number = value.parse::<u64>().map_err(|_| InvalidPublishedFileId)?;
        std::num::NonZeroU64::new(number)
            .map(Self)
            .ok_or(InvalidPublishedFileId)
    }
}

impl fmt::Display for PublishedFileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_package_names_without_interpreting_token_suffixes() {
        assert_eq!(
            PackageId::new("  Example.Package  ").unwrap().as_str(),
            "example.package"
        );
        assert_eq!(
            PackageId::new("Example.Package_STEAM").unwrap().as_str(),
            "example.package_steam"
        );
        for value in ["", "  ", "example. package", "a\nb"] {
            assert!(PackageId::new(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn token_suffixes_round_trip_without_becoming_package_identity() {
        for (input, expected, preference) in [
            (
                " Example.Package ",
                "example.package",
                PackagePreference::Default,
            ),
            (
                "Example.Package_STEAM",
                "example.package_steam",
                PackagePreference::Steam,
            ),
            (
                "Example.Package_LOCAL",
                "example.package_local",
                PackagePreference::Local,
            ),
        ] {
            let token: PackageToken = input.parse().unwrap();
            assert_eq!(token.package_id().as_str(), "example.package");
            assert_eq!(token.preference(), preference);
            assert_eq!(token.to_string(), expected);
            assert_eq!(expected.parse::<PackageToken>().unwrap(), token);
        }
        for invalid in ["", "_steam", "_local"] {
            assert!(invalid.parse::<PackageToken>().is_err());
        }
    }

    #[test]
    fn workshop_ids_reject_zero_invalid_and_overflow_values() {
        let id: PublishedFileId = " 2009463077 ".parse().unwrap();
        assert_eq!(id.get(), 2_009_463_077);
        assert_eq!(id.to_string(), "2009463077");
        for invalid in ["", "0", "-1", "+1", "1.5", "abc", "18446744073709551616"] {
            assert!(invalid.parse::<PublishedFileId>().is_err());
        }
    }

    #[test]
    fn installation_identity_preserves_paths_and_distinguishes_copies() {
        let base = std::env::temp_dir();
        let local = ModId::new(base.join("local/Example")).unwrap();
        let workshop = ModId::new(base.join("workshop/Example")).unwrap();
        assert_ne!(local, workshop);
        assert_eq!(local.path(), base.join("local/Example"));
        assert!(ModId::new("relative/mod").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn installation_identity_keeps_case_sensitive_paths_distinct() {
        assert_ne!(
            ModId::new("/mods/Example").unwrap(),
            ModId::new("/mods/example").unwrap()
        );
    }
}
