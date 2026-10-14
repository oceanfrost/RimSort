use core::identity::{ModId, ModSource, PackageId};
use core::metadata::{InstalledMod, MetadataError, MetadataWarning, parse_about_xml};

#[test]
fn upstream_local_fixture_preserves_dependency_details() {
    let parsed = parse_about_xml(include_str!("fixtures/local-mod.xml")).unwrap();
    let metadata = parsed.metadata;
    assert_eq!(metadata.package_id.as_str(), "local.mod1");
    assert_eq!(metadata.name.as_deref(), Some("local mod 1"));
    assert_eq!(metadata.authors, ["local mod 1 author"]);
    assert_eq!(metadata.supported_versions, ["1.3", "1.4", "1.5"]);
    let dependency = &metadata.dependencies[0];
    assert_eq!(dependency.target.package_id.as_str(), "brrainz.harmony");
    assert_eq!(dependency.display_name.as_deref(), Some("Harmony"));
    assert_eq!(
        dependency.steam_workshop_url.as_deref(),
        Some("steam://url/CommunityFilePage/2009463077")
    );
    assert_eq!(
        dependency.download_url.as_deref(),
        Some("https://github.com/pardeike/HarmonyRimWorld/releases/latest")
    );
    assert_eq!(
        parsed.warnings,
        [MetadataWarning::UnsupportedVersionField(
            "descriptionsByVersion".into()
        )]
    );
}

#[test]
fn upstream_core_fixture_accepts_bom_and_missing_display_fields() {
    let parsed = parse_about_xml(include_str!("fixtures/core.xml")).unwrap();
    assert!(parsed.warnings.is_empty());
    assert_eq!(parsed.metadata.package_id.as_str(), "ludeon.rimworld");
    assert_eq!(parsed.metadata.name, None);
    assert_eq!(parsed.metadata.authors, ["Ludeon Studios"]);
    assert_eq!(
        parsed
            .metadata
            .force_load_before
            .iter()
            .map(|r| r.package_id.as_str())
            .collect::<Vec<_>>(),
        ["ludeon.rimworld.ideology", "ludeon.rimworld.royalty"]
    );
}

#[test]
fn parses_all_base_fields_preserving_cdata_and_entities() {
    let parsed = parse_about_xml(include_str!("fixtures/full.xml")).unwrap();
    let metadata = parsed.metadata;
    assert!(parsed.warnings.is_empty());
    assert_eq!(metadata.package_id.as_str(), "author.example");
    assert_eq!(metadata.name.as_deref(), Some("Example Mod & Tools"));
    assert_eq!(metadata.authors, ["Bob", "Alice"]);
    assert_eq!(
        metadata.description.as_deref(),
        Some("Example description <b>label</b> & literal text")
    );
    assert_eq!(metadata.supported_versions, ["1.5", "1.6"]);
    assert_eq!(metadata.mod_version.as_deref(), Some("2.0"));
    assert_eq!(metadata.url.as_deref(), Some("https://example.test/mod"));
    assert_eq!(metadata.dependencies.len(), 1);
    for (targets, expected) in [
        (&metadata.load_before, "other.before"),
        (&metadata.load_after, "other.after"),
        (&metadata.force_load_before, "other.forcebefore"),
        (&metadata.force_load_after, "other.forceafter"),
        (&metadata.incompatible_with, "other.conflict"),
    ] {
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].package_id.as_str(), expected);
        assert!(!targets[0].conditions.is_conditional());
    }
}

#[test]
fn missing_optional_fields_have_empty_or_absent_values() {
    let metadata = parse_about_xml("<ModMetaData><packageId>a.mod</packageId></ModMetaData>")
        .unwrap()
        .metadata;
    assert_eq!(metadata.name, None);
    assert_eq!(metadata.description, None);
    assert!(metadata.authors.is_empty());
    assert!(metadata.supported_versions.is_empty());
    assert!(metadata.dependencies.is_empty());
    assert!(metadata.load_after.is_empty());
}

#[test]
fn empty_authors_list_falls_back_to_single_author() {
    let xml = r#"
        <ModMetaData>
            <packageId>a</packageId>
            <author> Author </author>
            <authors><li> </li></authors>
        </ModMetaData>
    "#;
    let metadata = parse_about_xml(xml).unwrap().metadata;
    assert_eq!(metadata.authors, ["Author"]);
}

#[test]
fn preserves_conditional_rule_attributes_and_warns_once() {
    let xml = r#"
        <ModMetaData>
            <packageId>a</packageId>
            <loadAfter>
                <li MayRequire="Some.DLC"> other.mod </li>
            </loadAfter>
            <forceLoadBefore>
                <li MayRequireAnyOf="A.Mod,B.Mod">third.mod</li>
            </forceLoadBefore>
            <modDependencies>
                <li MayRequire="Some.DLC"><packageId>Lib.Mod</packageId></li>
            </modDependencies>
        </ModMetaData>
    "#;
    let parsed = parse_about_xml(xml).unwrap();
    assert_eq!(
        parsed.metadata.load_after[0]
            .conditions
            .may_require
            .as_deref(),
        Some("Some.DLC")
    );
    assert_eq!(
        parsed.metadata.force_load_before[0]
            .conditions
            .may_require_any_of
            .as_deref(),
        Some("A.Mod,B.Mod")
    );
    assert_eq!(
        parsed.metadata.dependencies[0]
            .target
            .conditions
            .may_require
            .as_deref(),
        Some("Some.DLC")
    );
    assert_eq!(
        parsed.warnings,
        [MetadataWarning::ConditionalRulesNotEvaluated]
    );
}

#[test]
fn version_specific_fields_are_reported_without_merging_into_base_rules() {
    let parsed = parse_about_xml(
        "<ModMetaData><packageId>a</packageId>
        <loadAfter><li>base.mod</li></loadAfter>
        <loadAfterByVersion><v1.6><li>version.mod</li></v1.6></loadAfterByVersion>
        <modDependenciesByVersion><v1.6 /></modDependenciesByVersion>
        <loadBeforeByVersion /><incompatibleWithByVersion />
        </ModMetaData>",
    )
    .unwrap();
    assert_eq!(parsed.metadata.load_after.len(), 1);
    assert_eq!(
        parsed.metadata.load_after[0].package_id.as_str(),
        "base.mod"
    );
    assert_eq!(
        parsed.warnings,
        [
            MetadataWarning::UnsupportedVersionField("loadAfterByVersion".into()),
            MetadataWarning::UnsupportedVersionField("modDependenciesByVersion".into()),
            MetadataWarning::UnsupportedVersionField("loadBeforeByVersion".into()),
            MetadataWarning::UnsupportedVersionField("incompatibleWithByVersion".into()),
        ]
    );
}

#[test]
fn reports_missing_and_invalid_identifiers() {
    assert!(matches!(
        parse_about_xml("<ModMetaData />"),
        Err(MetadataError::MissingPackageId)
    ));
    for xml in [
        "<ModMetaData><packageId /></ModMetaData>",
        "<ModMetaData><packageId> </packageId></ModMetaData>",
        "<ModMetaData><packageId>bad id</packageId></ModMetaData>",
    ] {
        assert!(matches!(
            parse_about_xml(xml),
            Err(MetadataError::InvalidPackageId {
                field: "packageId",
                ..
            })
        ));
    }
    assert!(matches!(
        parse_about_xml(
            "<ModMetaData><packageId>a</packageId><modDependencies><li /></modDependencies></ModMetaData>"
        ),
        Err(MetadataError::InvalidPackageId {
            field: "modDependencies/packageId",
            ..
        })
    ));
    assert!(matches!(
        parse_about_xml(
            "<ModMetaData><packageId>a</packageId><loadAfter><li>bad id</li></loadAfter></ModMetaData>"
        ),
        Err(MetadataError::InvalidPackageId {
            field: "loadAfter",
            ..
        })
    ));
}

#[test]
fn rejects_wrong_roots_multiple_documents_and_trailing_content() {
    for xml in [
        "",
        "<Wrong><packageId>a</packageId></Wrong>",
        "<ModMetaData><packageId>a</packageId></ModMetaData><ModMetaData />",
        "<ModMetaData><packageId>a</packageId></ModMetaData>garbage",
        "<ModMetaData><packageId>a</packageId></ModMetaData><![CDATA[tail]]>",
        "<ModMetaData><packageId>a</packageId></ModMetaData>&amp;",
    ] {
        assert!(
            matches!(parse_about_xml(xml), Err(MetadataError::InvalidRoot)),
            "{xml}"
        );
    }
}

#[test]
fn rejects_truncated_mismatched_or_invalid_xml_even_in_unknown_fields() {
    for xml in [
        "<ModMetaData><packageId>a</packageId>",
        "<ModMetaData><packageId>a</name></ModMetaData>",
        "<ModMetaData><packageId>a</packageId><name>A & B</name></ModMetaData>",
        "<ModMetaData><packageId>a</packageId><extension>&undefined;</extension></ModMetaData>",
        "<ModMetaData><packageId>a</packageId><extension key='&undefined;' /></ModMetaData>",
        "<ModMetaData><packageId>a</packageId><extension key='1' key='2' /></ModMetaData>",
        "<ModMetaData><packageId>a</packageId><packageId>b</packageId></ModMetaData>",
    ] {
        assert!(
            matches!(parse_about_xml(xml), Err(MetadataError::InvalidXml(_))),
            "{xml}"
        );
    }
}

#[test]
fn rejects_doctype_and_accepts_comments_and_numeric_references() {
    assert!(matches!(
        parse_about_xml(
            "<!DOCTYPE ModMetaData><ModMetaData><packageId>a</packageId></ModMetaData>"
        ),
        Err(MetadataError::UnsupportedDoctype)
    ));
    let xml = r#"
        <!-- before -->
        <ModMetaData>
            <packageId>a</packageId>
            <name>Caf&#xE9; &amp; Caf&#233;</name>
        </ModMetaData>
        <!-- after -->
    "#;
    let parsed = parse_about_xml(xml).unwrap();
    assert_eq!(
        parsed.metadata.name.as_deref(),
        Some("Caf\u{e9} & Caf\u{e9}")
    );
}

#[test]
fn two_installations_can_share_one_package_without_collapsing_identity() {
    let metadata = parse_about_xml(include_str!("fixtures/full.xml"))
        .unwrap()
        .metadata;
    let base = std::env::temp_dir();
    let local = InstalledMod {
        id: ModId::new(base.join("local/example")).unwrap(),
        source: ModSource::Local,
        published_file_id: None,
        metadata: metadata.clone(),
    };
    let workshop = InstalledMod {
        id: ModId::new(base.join("workshop/example")).unwrap(),
        source: ModSource::Workshop,
        published_file_id: Some("2009463077".parse().unwrap()),
        metadata,
    };
    assert_eq!(
        local.metadata.package_id,
        PackageId::new("Author.Example").unwrap()
    );
    assert_eq!(local.metadata.package_id, workshop.metadata.package_id);
    assert_ne!(local.id, workshop.id);
    assert_ne!(local.source, workshop.source);
}

#[test]
fn preserves_multibyte_utf8_text() {
    let name = "Caf\u{e9} \u{1f680}";
    let xml = format!("<ModMetaData><packageId>a</packageId><name>{name}</name></ModMetaData>");
    let parsed = parse_about_xml(&xml).unwrap();
    assert_eq!(parsed.metadata.name.as_deref(), Some(name));
}
