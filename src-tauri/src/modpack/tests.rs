#[cfg(test)]
mod modpack_tests {
    use crate::modpack::apply::{build_lock_snapshot, detect_drift};
    use crate::modpack::layers::{diff_entries, make_base_spec, reduce_layers};
    use crate::modpack::migration::migrate_legacy_payload;
    use crate::modpack::types::{EntriesDelta, Layer, ModEntry};
    use std::collections::HashMap;

    fn entry(provider: &str, project_id: &str) -> ModEntry {
        ModEntry {
            provider: provider.to_string(),
            project_id: project_id.to_string(),
            slug: None,
            content_type: "mods".to_string(),
            required: true,
            pin: None,
            channel_policy: "stable".to_string(),
            fallback_policy: "inherit".to_string(),
            replacement_group: None,
            notes: None,
            disabled_by_default: false,
            optional: false,
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            local_file_name: None,
            local_file_path: None,
            local_sha512: None,
            local_fingerprints: vec![],
        }
    }

    #[test]
    fn layer_reduce_detects_duplicates() {
        let mut spec = make_base_spec("modpack_1".to_string(), "A".to_string(), "now".to_string());
        spec.layers = vec![
            Layer {
                id: "l1".to_string(),
                name: "Template".to_string(),
                source: None,
                is_frozen: false,
                entries_delta: EntriesDelta {
                    add: vec![entry("modrinth", "abc")],
                    remove: vec![],
                    override_entries: vec![],
                },
            },
            Layer {
                id: "l2".to_string(),
                name: "User".to_string(),
                source: None,
                is_frozen: false,
                entries_delta: EntriesDelta {
                    add: vec![entry("modrinth", "abc")],
                    remove: vec![],
                    override_entries: vec![],
                },
            },
        ];

        let (_entries, conflicts, _warnings) = reduce_layers(&spec);
        assert!(!conflicts.is_empty());
    }

    #[test]
    fn diff_entries_reports_added_removed_overridden() {
        let current = vec![entry("modrinth", "a"), entry("modrinth", "b")];
        let mut next = vec![entry("modrinth", "b"), entry("modrinth", "c")];
        next[0].pin = Some("123".to_string());

        let (added, removed, overridden) = diff_entries(&current, &next);
        assert_eq!(added.len(), 1);
        assert_eq!(removed.len(), 1);
        assert_eq!(overridden.len(), 1);
    }

    #[test]
    fn migration_converts_legacy_presets() {
        let payload = serde_json::json!({
            "presets": [
                {
                    "id": "preset_1",
                    "name": "Legacy",
                    "entries": [
                        {
                            "source": "modrinth",
                            "project_id": "abc",
                            "title": "ABC",
                            "content_type": "mods",
                            "enabled": true
                        }
                    ]
                }
            ]
        });

        let (report, specs) = migrate_legacy_payload(&payload);
        assert_eq!(report.migrated_count, 1);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].layers.iter().find(|l| l.id == "layer_user").unwrap().entries_delta.add.len(), 1);
    }

    #[test]
    fn drift_detects_version_changes() {
        let lock = crate::Lockfile {
            version: 2,
            entries: vec![crate::LockEntry {
                source: "modrinth".to_string(),
                project_id: "abc".to_string(),
                version_id: "v2".to_string(),
                name: "ABC".to_string(),
                version_number: "2.0".to_string(),
                filename: "abc.jar".to_string(),
                content_type: "mods".to_string(),
                target_scope: "instance".to_string(),
                target_worlds: vec![],
                pinned_version: None,
                enabled: true,
                hashes: HashMap::new(),
            }],
        };

        let expected_lock = crate::Lockfile {
            version: 2,
            entries: vec![crate::LockEntry {
                source: "modrinth".to_string(),
                project_id: "abc".to_string(),
                version_id: "v1".to_string(),
                name: "ABC".to_string(),
                version_number: "1.0".to_string(),
                filename: "abc.jar".to_string(),
                content_type: "mods".to_string(),
                target_scope: "instance".to_string(),
                target_worlds: vec![],
                pinned_version: None,
                enabled: true,
                hashes: HashMap::new(),
            }],
        };

        let snapshot = build_lock_snapshot("inst", "plan", &expected_lock, None);
        let drift = detect_drift("inst", &lock, &snapshot);
        assert_eq!(drift.status, "drifted");
        assert_eq!(drift.version_changed.len(), 1);
    }
}
