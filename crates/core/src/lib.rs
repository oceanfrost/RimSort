//! UI-independent RimWorld mod identity and About.xml parsing.
//!
//! The parser takes UTF-8 XML in memory and performs no filesystem or GUI work.
//!
//! ```
//! use core::metadata::{MetadataError, parse_about_xml};
//!
//! fn main() -> Result<(), MetadataError> {
//!     let parsed = parse_about_xml(
//!         "<ModMetaData><packageId>Author.Mod</packageId></ModMetaData>"
//!     )?;
//!     assert_eq!(parsed.metadata.package_id.as_str(), "author.mod");
//!     Ok(())
//! }
//! ```

pub mod identity;
pub mod metadata;
