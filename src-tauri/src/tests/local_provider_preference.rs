use crate::*;
fn sample_match(source: &str, confidence: &str, project: &str) -> LocalImportedProviderMatch {
    LocalImportedProviderMatch {
        source: source.to_string(),
        project_id: project.to_string(),
        version_id: "v1".to_string(),
        name: "Sample".to_string(),
        version_number: "1.0.0".to_string(),
        hashes: HashMap::new(),
        confidence: confidence.to_string(),
        reason: "test".to_string(),
        verification_status: "verified".to_string(),
    }
}

#[test]
fn select_preferred_provider_match_prefers_modrinth_on_tie() {
    let matches = vec![
        sample_match("curseforge", "high", "cf:123"),
        sample_match("modrinth", "high", "mr:abc"),
    ];
    let selected = select_preferred_provider_match(&matches, None).expect("selected match");
    assert_eq!(selected.source, "modrinth");
}

#[test]
fn to_provider_candidates_keeps_modrinth_first_on_tie() {
    let deduped = dedupe_provider_matches(vec![
        sample_match("curseforge", "high", "cf:123"),
        sample_match("modrinth", "high", "mr:abc"),
    ]);
    let candidates = to_provider_candidates(&deduped);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].source, "modrinth");
    assert_eq!(candidates[1].source, "curseforge");
}

#[test]
fn provider_match_auto_activation_blocks_medium_github() {
    let github_medium = sample_match("github", "medium", "owner/repo");
    let github_high = sample_match("github", "high", "owner/repo");
    let modrinth_high = sample_match("modrinth", "high", "mr:test");
    assert!(!provider_match_is_auto_activatable(&github_medium));
    assert!(provider_match_is_auto_activatable(&github_high));
    assert!(provider_match_is_auto_activatable(&modrinth_high));
}

#[test]
fn provider_match_auto_activation_allows_manual_unverified_direct_repo_hint() {
    let github_manual = LocalImportedProviderMatch {
            source: "github".to_string(),
            project_id: "gh:example/repo".to_string(),
            version_id: "gh_repo_unverified".to_string(),
            name: "Example".to_string(),
            version_number: "unverified".to_string(),
            hashes: HashMap::new(),
            confidence: "manual".to_string(),
            reason: "GitHub local identify manual candidate: direct metadata repo hint matched, but release verification is unavailable (GitHub API rate limit reached).".to_string(),
            verification_status: "manual_unverified".to_string(),
        };
    assert!(provider_match_is_auto_activatable(&github_manual));
}

#[test]
fn compact_provider_candidates_dedupes_same_source_project() {
    let compacted = compact_provider_candidates(vec![
        ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:example/repo".to_string(),
            version_id: "gh_repo_unverified".to_string(),
            name: "Example Repo".to_string(),
            version_number: "unverified".to_string(),
            confidence: Some("manual".to_string()),
            reason: Some("manual".to_string()),
            verification_status: Some("manual_unverified".to_string()),
        },
        ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:example/repo".to_string(),
            version_id: "gh_release:42".to_string(),
            name: "Example Repo".to_string(),
            version_number: "v1.2.3".to_string(),
            confidence: Some("high".to_string()),
            reason: Some("verified".to_string()),
            verification_status: Some("verified".to_string()),
        },
    ]);
    assert_eq!(compacted.len(), 1);
    assert_eq!(compacted[0].version_id, "gh_release:42");
}

#[test]
fn effective_updatable_provider_allows_safe_local_github_candidate() {
    let entry = LockEntry {
        source: "local".to_string(),
        project_id: "local:mods:test.jar".to_string(),
        version_id: "local_1".to_string(),
        name: "Test".to_string(),
        version_number: "local-file".to_string(),
        filename: "test.jar".to_string(),
        content_type: "mods".to_string(),
        target_scope: "instance".to_string(),
        target_worlds: vec![],
        pinned_version: None,
        enabled: true,
        hashes: HashMap::new(),
        provider_candidates: vec![ProviderCandidate {
            source: "github".to_string(),
            project_id: "gh:example/repo".to_string(),
            version_id: "gh_release:7".to_string(),
            name: "Example Repo".to_string(),
            version_number: "v1.0.0".to_string(),
            confidence: Some("high".to_string()),
            reason: Some("verified".to_string()),
            verification_status: Some("verified".to_string()),
        }],
        local_analysis: None,
    };
    let effective = effective_updatable_provider_for_entry(&entry, UpdateScope::AllContent)
        .expect("effective provider");
    assert_eq!(effective.source.to_ascii_lowercase(), "github");
    assert_eq!(effective.project_id, "gh:example/repo");
}

#[test]
fn effective_updatable_provider_blocks_weak_local_github_candidate() {
    let entry = LockEntry {
            source: "local".to_string(),
            project_id: "local:mods:test.jar".to_string(),
            version_id: "local_1".to_string(),
            name: "Test".to_string(),
            version_number: "local-file".to_string(),
            filename: "test.jar".to_string(),
            content_type: "mods".to_string(),
            target_scope: "instance".to_string(),
            target_worlds: vec![],
            pinned_version: None,
            enabled: true,
            hashes: HashMap::new(),
            provider_candidates: vec![ProviderCandidate {
                source: "github".to_string(),
                project_id: "gh:example/repo".to_string(),
                version_id: "gh_repo_unverified".to_string(),
                name: "Example Repo".to_string(),
                version_number: "unverified".to_string(),
                confidence: Some("manual".to_string()),
                reason: Some(
                    "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file."
                        .to_string(),
                ),
                verification_status: Some("manual_unverified".to_string()),
            }],
            local_analysis: None,
        };
    assert!(effective_updatable_provider_for_entry(&entry, UpdateScope::AllContent).is_none());
}
