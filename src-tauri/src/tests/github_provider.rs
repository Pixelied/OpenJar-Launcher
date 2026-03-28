use crate::*;
fn sample_repo() -> GithubRepository {
    GithubRepository {
        full_name: "OpenJar/test-mod".to_string(),
        name: "test-mod".to_string(),
        description: Some("A test Minecraft mod".to_string()),
        stargazers_count: 4200,
        forks_count: 120,
        archived: false,
        fork: false,
        disabled: false,
        html_url: "https://github.com/OpenJar/test-mod".to_string(),
        homepage: None,
        watchers_count: 4200,
        open_issues_count: 12,
        pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
        updated_at: Some("2026-03-01T00:00:00Z".to_string()),
        topics: vec!["minecraft".to_string(), "fabric".to_string()],
        default_branch: "main".to_string(),
        owner: GithubOwner {
            login: "OpenJar".to_string(),
            owner_type: "Organization".to_string(),
        },
    }
}

fn release_with_assets(id: u64, published_at: &str, assets: Vec<&str>) -> GithubRelease {
    GithubRelease {
        id,
        tag_name: format!("v{id}"),
        html_url: format!("https://github.com/OpenJar/test-mod/releases/tag/v{id}"),
        name: None,
        draft: false,
        prerelease: false,
        created_at: Some(published_at.to_string()),
        published_at: Some(published_at.to_string()),
        assets: assets
            .into_iter()
            .map(|name| GithubReleaseAsset {
                name: name.to_string(),
                browser_download_url: format!(
                    "https://github.com/OpenJar/test-mod/releases/download/v{id}/{name}"
                ),
                content_type: Some("application/java-archive".to_string()),
                size: 2 * 1024 * 1024,
                digest: None,
            })
            .collect(),
    }
}

#[test]
fn github_repo_policy_rejects_unsafe_repository_states() {
    let mut repo = sample_repo();
    repo.archived = true;
    assert_eq!(
        github_repo_policy_rejection_reason(&repo),
        Some("repository is archived")
    );
    repo.archived = false;
    repo.fork = true;
    assert_eq!(
        github_repo_policy_rejection_reason(&repo),
        Some("repository is a fork")
    );
    repo.fork = false;
    repo.disabled = true;
    assert_eq!(
        github_repo_policy_rejection_reason(&repo),
        Some("repository is disabled")
    );
}

#[test]
fn github_error_classification_detects_auth_or_rate_limit() {
    assert!(github_error_is_auth_or_rate_limit(
        "GitHub API rate limit reached (403 Forbidden)."
    ));
    assert!(github_error_is_auth_or_rate_limit(
        "GitHub request failed with status 401 Unauthorized."
    ));
    assert!(!github_error_is_auth_or_rate_limit(
        "GitHub request failed with status 404 Not Found."
    ));
}

#[test]
fn github_reason_transient_detection_covers_verification_unavailable_messages() {
    assert!(github_reason_is_transient_verification_failure(
            "GitHub local identify manual candidate: direct metadata repo hint matched, but release verification is unavailable (GitHub API rate limit reached)."
        ));
    assert!(github_reason_is_transient_verification_failure(
            "GitHub local identify manual candidate: direct metadata repo hint found, but release evidence is currently unverifiable."
        ));
    assert!(!github_reason_is_transient_verification_failure(
            "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file."
        ));
}

#[test]
fn github_release_selector_picks_latest_real_jar_asset() {
    let repo = sample_repo();
    let releases = vec![
        release_with_assets(
            1,
            "2026-01-01T00:00:00Z",
            vec!["test-mod-1.0.0.jar", "checksums.sha256"],
        ),
        release_with_assets(
            2,
            "2026-02-01T00:00:00Z",
            vec!["test-mod-1.1.0-sources.jar", "test-mod-1.1.0.jar"],
        ),
    ];
    let selected =
        select_github_release_with_asset(&repo, &releases, "test mod", None, None, None, None)
            .expect("expected a selected github release");
    assert_eq!(selected.release.id, 2);
    assert_eq!(selected.asset.name, "test-mod-1.1.0.jar");
    assert!(!selected.asset.name.contains("sources"));
}

#[test]
fn github_release_query_hint_prefers_installed_filename() {
    let repo = sample_repo();
    let hint = github_release_query_hint("test-mod-1.2.3.jar.disabled", "Pretty Mod Name", &repo);
    assert_eq!(hint, "test-mod-1.2.3");
}

#[test]
fn github_release_selection_match_detects_same_release_label() {
    let repo = sample_repo();
    let releases = vec![release_with_assets(
        7,
        "2026-03-01T00:00:00Z",
        vec!["test-mod-1.2.0.jar"],
    )];
    let selection = select_github_release_with_asset(
        &repo,
        &releases,
        "test-mod-1.2.0",
        None,
        None,
        None,
        None,
    )
    .expect("expected selected release");
    assert!(github_release_selection_matches_current(
        &selection,
        "gh_release:999",
        "v7",
        &HashMap::new(),
    ));
}

#[test]
fn github_discover_hit_contains_confidence_metadata() {
    let repo = sample_repo();
    let releases = vec![release_with_assets(
        3,
        "2026-03-01T00:00:00Z",
        vec!["test-mod-1.2.0.jar", "checksums.sha256"],
    )];
    let selected =
        select_github_release_with_asset(&repo, &releases, "test mod", None, None, None, None)
            .expect("expected selected release");
    let hit = github_release_to_discover_hit(&repo, &selected, "test mod", None, None);
    assert_eq!(hit.source, "github");
    assert_eq!(hit.content_type, "mods");
    assert!(hit.confidence.is_some());
    assert!(hit.reason.is_some());
}

#[test]
fn github_release_selector_enforces_instance_compatibility() {
    let repo = sample_repo();
    let releases = vec![
        release_with_assets(
            1,
            "2026-01-01T00:00:00Z",
            vec!["test-mod-fabric-1.20.4.jar"],
        ),
        release_with_assets(
            2,
            "2026-02-01T00:00:00Z",
            vec!["test-mod-fabric-1.21.1.jar"],
        ),
    ];
    let selected = select_github_release_with_asset(
        &repo,
        &releases,
        "test mod",
        Some("1.21.1"),
        Some("fabric"),
        None,
        None,
    )
    .expect("expected a compatible github selection");
    assert_eq!(selected.release.id, 2);

    let incompatible = select_github_release_with_asset(
        &repo,
        &releases,
        "test mod",
        Some("1.21.1"),
        Some("forge"),
        None,
        None,
    );
    assert!(incompatible.is_none());
}

#[test]
fn github_asset_digest_matching_rejects_mismatch() {
    let mut digests = HashMap::new();
    digests.insert("sha256".to_string(), "abc123".to_string());
    assert_eq!(
        github_asset_digest_matches_local_hashes(&digests, "abc123", "zzz"),
        Some(true)
    );
    assert_eq!(
        github_asset_digest_matches_local_hashes(&digests, "nope", "zzz"),
        Some(false)
    );
}

#[test]
fn github_local_release_selector_uses_exact_asset_filename() {
    let repo = sample_repo();
    let releases = vec![
        release_with_assets(1, "2026-01-01T00:00:00Z", vec!["test-mod-1.0.0.jar"]),
        release_with_assets(
            2,
            "2026-02-01T00:00:00Z",
            vec!["test-mod-1.2.0.jar", "test-mod-1.2.0-sources.jar"],
        ),
    ];
    let selected =
        select_github_release_for_local_file(&repo, &releases, "test-mod-1.2.0.jar", "test mod")
            .expect("expected exact filename match");
    assert_eq!(selected.release.id, 2);
    assert_eq!(selected.asset.name, "test-mod-1.2.0.jar");
}

#[test]
fn github_local_release_selector_accepts_strong_name_pattern_match() {
    let repo = sample_repo();
    let releases = vec![
        release_with_assets(
            1,
            "2026-01-01T00:00:00Z",
            vec!["meteor-client-fabric-1.21.1-0.5.8.jar"],
        ),
        release_with_assets(2, "2026-02-01T00:00:00Z", vec!["something-else-1.0.0.jar"]),
    ];
    let selected = select_github_release_for_local_file(
        &repo,
        &releases,
        "meteor-client-0.5.8.jar",
        "meteor client",
    )
    .expect("expected strong fuzzy filename pattern match");
    assert_eq!(selected.release.id, 1);
    assert_eq!(selected.asset.name, "meteor-client-fabric-1.21.1-0.5.8.jar");
}

#[test]
fn github_local_match_rejects_similarity_only_without_hard_evidence() {
    let repo = sample_repo();
    let release = release_with_assets(1, "2026-01-01T00:00:00Z", vec!["totally-different.jar"]);
    let selection = GithubReleaseSelection {
        release: release.clone(),
        asset: release.assets[0].clone(),
        has_checksum_sidecar: false,
    };
    let result = github_local_match_confidence_and_reason(
        &repo,
        &selection,
        "meteor-client-1.0.0.jar",
        "meteor client",
        None,
        false,
        0,
        None,
        None,
    );
    assert!(result.is_err());
}

#[test]
fn github_local_match_rejects_ambiguous_baritone_on_weak_repo() {
    let weak_repo = GithubRepository {
        full_name: "kaushikkumarbora/forager".to_string(),
        name: "forager".to_string(),
        description: Some("A random utility project".to_string()),
        stargazers_count: 12,
        forks_count: 1,
        archived: false,
        fork: false,
        disabled: false,
        html_url: "https://github.com/kaushikkumarbora/forager".to_string(),
        homepage: None,
        watchers_count: 12,
        open_issues_count: 0,
        pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
        updated_at: Some("2026-03-01T00:00:00Z".to_string()),
        topics: vec![],
        default_branch: "main".to_string(),
        owner: GithubOwner {
            login: "kaushikkumarbora".to_string(),
            owner_type: "User".to_string(),
        },
    };
    let release = release_with_assets(2, "2026-01-01T00:00:00Z", vec!["baritone-1.0.0.jar"]);
    let selection = GithubReleaseSelection {
        release: release.clone(),
        asset: release.assets[0].clone(),
        has_checksum_sidecar: false,
    };
    let result = github_local_match_confidence_and_reason(
        &weak_repo,
        &selection,
        "baritone-1.0.0.jar",
        "baritone",
        None,
        false,
        0,
        None,
        None,
    );
    assert!(result.is_err());
}

#[test]
fn github_local_known_repo_boost_enables_canonical_baritone_match() {
    let canonical_repo = GithubRepository {
        full_name: "cabaletta/baritone".to_string(),
        name: "baritone".to_string(),
        description: Some("Minecraft pathfinding bot".to_string()),
        stargazers_count: 8_000,
        forks_count: 900,
        archived: false,
        fork: false,
        disabled: false,
        html_url: "https://github.com/cabaletta/baritone".to_string(),
        homepage: None,
        watchers_count: 8_000,
        open_issues_count: 12,
        pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
        updated_at: Some("2026-03-01T00:00:00Z".to_string()),
        topics: vec!["minecraft".to_string()],
        default_branch: "main".to_string(),
        owner: GithubOwner {
            login: "cabaletta".to_string(),
            owner_type: "Organization".to_string(),
        },
    };
    let (boost, reason) =
        github_local_known_repo_boost(&canonical_repo, "baritone-1.0.0.jar", "baritone", None);
    assert!(boost >= 40);
    assert!(reason.is_some());

    let release = release_with_assets(3, "2026-01-01T00:00:00Z", vec!["baritone-1.0.0.jar"]);
    let selection = GithubReleaseSelection {
        release: release.clone(),
        asset: release.assets[0].clone(),
        has_checksum_sidecar: false,
    };
    let evaluated = github_local_match_confidence_and_reason(
        &canonical_repo,
        &selection,
        "baritone-1.0.0.jar",
        "baritone",
        None,
        false,
        boost,
        reason,
        None,
    )
    .expect("canonical match accepted");
    assert!(matches!(evaluated.0.as_str(), "high" | "deterministic"));
}

#[test]
fn extract_github_repo_slug_parses_owner_repo_urls() {
    let parsed = extract_github_repo_slug("https://github.com/MeteorDevelopment/meteor-client");
    assert_eq!(parsed.as_deref(), Some("MeteorDevelopment/meteor-client"));
}

#[test]
fn extract_github_repo_slug_rejects_non_github_urls() {
    assert!(extract_github_repo_slug("https://meteorclient.com").is_none());
    assert!(extract_github_repo_slug("https://jfronny.gitlab.io").is_none());
}

#[test]
fn parse_github_project_id_rejects_non_github_urls() {
    assert!(parse_github_project_id("https://meteorclient.com").is_err());
    assert!(parse_github_project_id("gh:https://meteorclient.com").is_err());
    assert!(parse_github_project_id("https://jfronny.gitlab.io").is_err());
    assert!(parse_github_project_id("gh:https://jfronny.gitlab.io").is_err());
}

#[test]
fn parse_github_project_id_accepts_github_urls_with_extra_path_segments() {
    let parsed = parse_github_project_id(
        "https://github.com/MeteorDevelopment/meteor-client/releases/tag/v1.0.0",
    )
    .expect("github release URL should parse");
    assert_eq!(parsed.0, "MeteorDevelopment");
    assert_eq!(parsed.1, "meteor-client");
}

#[test]
fn parse_toml_assignment_is_case_insensitive() {
    let toml = r#"
            modId = "examplemod"
            displayName = "Example Mod"
            displayURL = "https://github.com/example/mod-repo"
        "#;
    assert_eq!(
        parse_toml_assignment(toml, "modid").as_deref(),
        Some("examplemod")
    );
    assert_eq!(
        parse_toml_assignment(toml, "displayname").as_deref(),
        Some("Example Mod")
    );
    assert_eq!(
        parse_toml_assignment(toml, "displayurl").as_deref(),
        Some("https://github.com/example/mod-repo")
    );
}

#[test]
fn github_api_tokens_from_env_entries_supports_pool_and_numbered_tokens() {
    let entries = vec![
        (
            "MPM_GITHUB_TOKENS".to_string(),
            "poolA, poolB;poolC\npoolD".to_string(),
        ),
        ("MPM_GITHUB_TOKEN_2".to_string(), "two".to_string()),
        ("MPM_GITHUB_TOKEN_1".to_string(), "one".to_string()),
        ("MPM_GITHUB_TOKEN_10".to_string(), "ten".to_string()),
        ("MPM_GITHUB_TOKEN".to_string(), "single".to_string()),
    ];
    let tokens = github_api_tokens_from_env_entries(&entries);
    assert_eq!(
        tokens,
        vec!["poolA", "poolB", "poolC", "poolD", "one", "two", "ten", "single",]
    );
}

#[test]
fn github_api_tokens_from_env_entries_supports_non_mpm_numbered_tokens() {
    let entries = vec![
        ("GITHUB_TOKEN_2".to_string(), "two".to_string()),
        ("GH_TOKEN_1".to_string(), "one".to_string()),
        ("GH_TOKEN_3".to_string(), "three".to_string()),
        ("GITHUB_TOKEN".to_string(), "fallback".to_string()),
    ];
    let tokens = github_api_tokens_from_env_entries(&entries);
    assert_eq!(tokens, vec!["one", "two", "three", "fallback"]);
}

#[test]
fn github_api_tokens_from_env_entries_deduplicates_across_sources() {
    let entries = vec![
        (
            "MPM_GITHUB_TOKENS".to_string(),
            "same,other,same".to_string(),
        ),
        ("MPM_GITHUB_TOKEN_1".to_string(), "same".to_string()),
        ("GITHUB_TOKEN".to_string(), "other".to_string()),
        ("GH_TOKEN".to_string(), "third".to_string()),
    ];
    let tokens = github_api_tokens_from_env_entries(&entries);
    assert_eq!(tokens, vec!["same", "other", "third"]);
}

#[test]
fn github_api_tokens_from_env_entries_caps_to_max_tokens() {
    let pool = (1..=(GITHUB_API_TOKENS_MAX + 20))
        .map(|idx| format!("token{idx}"))
        .collect::<Vec<_>>()
        .join(",");
    let entries = vec![("MPM_GITHUB_TOKENS".to_string(), pool)];
    let tokens = github_api_tokens_from_env_entries(&entries);
    let expected_last = format!("token{}", GITHUB_API_TOKENS_MAX);
    assert_eq!(tokens.len(), GITHUB_API_TOKENS_MAX);
    assert_eq!(tokens.first().map(String::as_str), Some("token1"));
    assert_eq!(
        tokens.last().map(String::as_str),
        Some(expected_last.as_str())
    );
}

#[test]
fn github_unverified_manual_candidate_is_manual_and_activatable_only_for_transient_outages() {
    let candidate = github_unverified_manual_candidate(
            "example",
            "repo",
            "Example Repo",
            "sha256",
            "sha512",
            "GitHub local identify manual candidate: direct metadata repo hint found, but repository verification is unavailable (rate limited).".to_string(),
        );
    assert_eq!(candidate.source, "github");
    assert_eq!(candidate.project_id, "gh:example/repo");
    assert_eq!(candidate.version_id, "gh_repo_unverified");
    assert_eq!(candidate.confidence, "manual");
    assert_eq!(
        candidate.hashes.get("sha256").map(String::as_str),
        Some("sha256")
    );
    assert_eq!(
        candidate.hashes.get("sha512").map(String::as_str),
        Some("sha512")
    );
    assert!(provider_match_is_auto_activatable(&candidate));

    let non_transient = github_unverified_manual_candidate(
            "example",
            "repo",
            "Example Repo",
            "sha256",
            "sha512",
            "GitHub local identify manual candidate: direct metadata repo hint matched, but no verified release asset matched the local file.".to_string(),
        );
    assert!(!provider_match_is_auto_activatable(&non_transient));
}

#[test]
fn github_provider_activation_rejects_invalid_project_ids() {
    let invalid_match = LocalImportedProviderMatch {
        source: "github".to_string(),
        project_id: "gh:https://meteorclient.com".to_string(),
        version_id: "gh_release:123".to_string(),
        name: "Invalid".to_string(),
        version_number: "1.0.0".to_string(),
        hashes: HashMap::new(),
        confidence: "deterministic".to_string(),
        reason: "invalid".to_string(),
        verification_status: "verified".to_string(),
    };
    assert!(!provider_match_is_auto_activatable(&invalid_match));

    let invalid_candidate = ProviderCandidate {
        source: "github".to_string(),
        project_id: "gh:https://meteorclient.com".to_string(),
        version_id: "gh_release:123".to_string(),
        name: "Invalid".to_string(),
        version_number: "1.0.0".to_string(),
        confidence: Some("deterministic".to_string()),
        reason: Some("invalid".to_string()),
        verification_status: Some("verified".to_string()),
    };
    assert!(!provider_candidate_is_auto_activatable(&invalid_candidate));
}
