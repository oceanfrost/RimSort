//! About.xml declarations; these are not yet resolved rules for a game version.

mod xml;

pub use xml::{MetadataError, MetadataWarning, ParsedMetadata, parse_about_xml};

use crate::identity::{ModId, ModSource, PackageId, PublishedFileId};

/// Conditions are retained as text and must be evaluated by a future rule
/// resolver. Consumers must not treat conditional entries as unconditional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleConditions {
    pub may_require: Option<String>,
    pub may_require_any_of: Option<String>,
}

impl RuleConditions {
    pub fn is_conditional(&self) -> bool {
        self.may_require.is_some() || self.may_require_any_of.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleTarget {
    pub package_id: PackageId,
    pub conditions: RuleConditions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub target: RuleTarget,
    pub display_name: Option<String>,
    pub steam_workshop_url: Option<String>,
    pub download_url: Option<String>,
}

/// Base declarations from one About.xml, independent of installation or UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMetadata {
    pub package_id: PackageId,
    pub name: Option<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub supported_versions: Vec<String>,
    pub mod_version: Option<String>,
    pub url: Option<String>,
    pub dependencies: Vec<Dependency>,
    pub load_before: Vec<RuleTarget>,
    pub load_after: Vec<RuleTarget>,
    pub force_load_before: Vec<RuleTarget>,
    pub force_load_after: Vec<RuleTarget>,
    pub incompatible_with: Vec<RuleTarget>,
}

/// Separate records can share metadata.package_id while having distinct IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledMod {
    pub id: ModId,
    pub source: ModSource,
    pub published_file_id: Option<PublishedFileId>,
    pub metadata: ModMetadata,
}
