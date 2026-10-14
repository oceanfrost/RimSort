use std::fmt;

use quick_xml::{Reader, events::Event};
use serde::Deserialize;

use super::{Dependency, ModMetadata, RuleConditions, RuleTarget};
use crate::identity::{InvalidPackageId, PackageId};

/// Errors are returned to the caller rather than silently dropping a mod.
#[derive(Debug)]
pub enum MetadataError {
    InvalidXml(String),
    InvalidRoot,
    UnsupportedDoctype,
    MissingPackageId,
    InvalidPackageId {
        field: &'static str,
        source: InvalidPackageId,
    },
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidXml(message) => write!(f, "invalid About.xml: {message}"),
            Self::InvalidRoot => f.write_str("About.xml must contain exactly one ModMetaData root"),
            Self::UnsupportedDoctype => {
                f.write_str("About.xml document type declarations are unsupported")
            }
            Self::MissingPackageId => f.write_str("About.xml is missing packageId"),
            Self::InvalidPackageId { field, source } => write!(f, "invalid {field}: {source}"),
        }
    }
}

impl std::error::Error for MetadataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPackageId { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataWarning {
    /// Recognized version-specific data is not yet part of the base model.
    UnsupportedVersionField(String),
    /// Conditions are preserved, but no installed-mod context was supplied.
    ConditionalRulesNotEvaluated,
}

/// Base declarations and warnings that callers must inspect before using rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMetadata {
    pub metadata: ModMetadata,
    pub warnings: Vec<MetadataWarning>,
}

/// Parse a single UTF-8 About.xml document, accepting an optional leading BOM.
///
/// Missing display fields are optional; packageId and non-empty rule targets
/// must be valid. Empty list items are ignored. Unknown extension fields are
/// tolerated. ByVersion fields produce warnings rather than claiming parity
/// with a game-version resolver. XML entities must be well formed; input is
/// never silently repaired, and CDATA preserves literal text.
pub fn parse_about_xml(xml: &str) -> Result<ParsedMetadata, MetadataError> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let mut warnings = validate_document(xml)?;
    let raw: RawMetadata = quick_xml::de::from_str(xml)
        .map_err(|error| MetadataError::InvalidXml(error.to_string()))?;
    let package_id = parse_package_id(
        raw.package_id
            .as_deref()
            .ok_or(MetadataError::MissingPackageId)?,
        "packageId",
    )?;
    let mut authors = raw.authors.map(RawList::into_strings).unwrap_or_default();
    if authors.is_empty() {
        authors.extend(trimmed_non_empty(raw.author));
    }
    let dependencies = raw
        .mod_dependencies
        .unwrap_or_default()
        .items
        .into_iter()
        .map(|dependency| {
            Ok(Dependency {
                target: RuleTarget {
                    package_id: parse_package_id(
                        dependency.package_id.as_deref().unwrap_or(""),
                        "modDependencies/packageId",
                    )?,
                    conditions: dependency.conditions(),
                },
                display_name: trimmed_non_empty(dependency.display_name),
                steam_workshop_url: trimmed_non_empty(dependency.steam_workshop_url),
                download_url: trimmed_non_empty(dependency.download_url),
            })
        })
        .collect::<Result<Vec<_>, MetadataError>>()?;
    let metadata = ModMetadata {
        package_id,
        name: trimmed_non_empty(raw.name),
        authors,
        description: trimmed_non_empty(raw.description),
        supported_versions: raw
            .supported_versions
            .map(RawList::into_strings)
            .unwrap_or_default(),
        mod_version: trimmed_non_empty(raw.mod_version),
        url: trimmed_non_empty(raw.url),
        dependencies,
        load_before: parse_rule_targets(raw.load_before, "loadBefore")?,
        load_after: parse_rule_targets(raw.load_after, "loadAfter")?,
        force_load_before: parse_rule_targets(raw.force_load_before, "forceLoadBefore")?,
        force_load_after: parse_rule_targets(raw.force_load_after, "forceLoadAfter")?,
        incompatible_with: parse_rule_targets(raw.incompatible_with, "incompatibleWith")?,
    };
    if metadata
        .dependencies
        .iter()
        .any(|dep| dep.target.conditions.is_conditional())
        || [
            &metadata.load_before,
            &metadata.load_after,
            &metadata.force_load_before,
            &metadata.force_load_after,
            &metadata.incompatible_with,
        ]
        .into_iter()
        .flatten()
        .any(|target| target.conditions.is_conditional())
    {
        warnings.push(MetadataWarning::ConditionalRulesNotEvaluated);
    }
    Ok(ParsedMetadata { metadata, warnings })
}

fn trimmed_non_empty(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn parse_package_id(value: &str, field: &'static str) -> Result<PackageId, MetadataError> {
    PackageId::new(value).map_err(|source| MetadataError::InvalidPackageId { field, source })
}

fn parse_rule_targets(
    list: Option<RawList>,
    field: &'static str,
) -> Result<Vec<RuleTarget>, MetadataError> {
    list.unwrap_or_default()
        .items
        .into_iter()
        .filter(|item| !item.text.trim().is_empty())
        .map(|item| {
            Ok(RuleTarget {
                package_id: parse_package_id(&item.text, field)?,
                conditions: item.conditions(),
            })
        })
        .collect()
}

// Validate the entire document before serde maps only the fields it understands.
// In particular, reject extra roots, trailing text, truncation, DTDs and invalid
// references even when they appear inside an unknown extension field.
fn validate_document(xml: &str) -> Result<Vec<MetadataWarning>, MetadataError> {
    let mut reader = Reader::from_str(xml);
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut warnings = Vec::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| MetadataError::InvalidXml(e.to_string()))?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if depth == 0 {
                    if root_seen || element.name().as_ref() != b"ModMetaData" {
                        return Err(MetadataError::InvalidRoot);
                    }
                    root_seen = true;
                } else if depth == 1 {
                    let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
                    if name.ends_with("ByVersion") {
                        let warning = MetadataWarning::UnsupportedVersionField(name);
                        if !warnings.contains(&warning) {
                            warnings.push(warning);
                        }
                    }
                }
                for attribute in element.attributes() {
                    let attribute =
                        attribute.map_err(|e| MetadataError::InvalidXml(e.to_string()))?;
                    attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|e| MetadataError::InvalidXml(e.to_string()))?;
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(MetadataError::InvalidRoot)?;
            }
            Event::Text(text) => {
                let value = text
                    .decode()
                    .map_err(|e| MetadataError::InvalidXml(e.to_string()))?;
                quick_xml::escape::unescape(&value)
                    .map_err(|e| MetadataError::InvalidXml(e.to_string()))?;
                if depth == 0 && !value.trim().is_empty() {
                    return Err(MetadataError::InvalidRoot);
                }
            }
            Event::GeneralRef(reference) => {
                if depth == 0 {
                    return Err(MetadataError::InvalidRoot);
                }
                let name = reference
                    .decode()
                    .map_err(|e| MetadataError::InvalidXml(e.to_string()))?;
                quick_xml::escape::unescape(&format!("&{name};"))
                    .map_err(|e| MetadataError::InvalidXml(e.to_string()))?;
            }
            Event::CData(_) if depth == 0 => return Err(MetadataError::InvalidRoot),
            Event::DocType(_) => return Err(MetadataError::UnsupportedDoctype),
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen {
        return Err(MetadataError::InvalidRoot);
    }
    if depth != 0 {
        return Err(MetadataError::InvalidXml("unclosed element".into()));
    }
    Ok(warnings)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMetadata {
    package_id: Option<String>,
    name: Option<String>,
    author: Option<String>,
    authors: Option<RawList>,
    description: Option<String>,
    supported_versions: Option<RawList>,
    mod_version: Option<String>,
    url: Option<String>,
    mod_dependencies: Option<RawDependencies>,
    load_before: Option<RawList>,
    load_after: Option<RawList>,
    force_load_before: Option<RawList>,
    force_load_after: Option<RawList>,
    incompatible_with: Option<RawList>,
}

#[derive(Debug, Default, Deserialize)]
struct RawList {
    #[serde(rename = "li", default)]
    items: Vec<RawItem>,
}

impl RawList {
    fn into_strings(self) -> Vec<String> {
        self.items
            .into_iter()
            .filter_map(|item| trimmed_non_empty(Some(item.text)))
            .collect()
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawItem {
    #[serde(rename = "$text", default)]
    text: String,
    #[serde(rename = "@MayRequire")]
    may_require: Option<String>,
    #[serde(rename = "@MayRequireAnyOf")]
    may_require_any_of: Option<String>,
}

impl RawItem {
    fn conditions(&self) -> RuleConditions {
        RuleConditions {
            may_require: trimmed_non_empty(self.may_require.clone()),
            may_require_any_of: trimmed_non_empty(self.may_require_any_of.clone()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawDependencies {
    #[serde(rename = "li", default)]
    items: Vec<RawDependency>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDependency {
    package_id: Option<String>,
    display_name: Option<String>,
    steam_workshop_url: Option<String>,
    download_url: Option<String>,
    #[serde(rename = "@MayRequire")]
    may_require: Option<String>,
    #[serde(rename = "@MayRequireAnyOf")]
    may_require_any_of: Option<String>,
}

impl RawDependency {
    fn conditions(&self) -> RuleConditions {
        RuleConditions {
            may_require: trimmed_non_empty(self.may_require.clone()),
            may_require_any_of: trimmed_non_empty(self.may_require_any_of.clone()),
        }
    }
}
