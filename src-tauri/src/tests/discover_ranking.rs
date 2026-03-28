use crate::*;
fn make_hit(source: &str, project_id: &str) -> DiscoverSearchHit {
    DiscoverSearchHit {
        source: source.to_string(),
        project_id: project_id.to_string(),
        title: project_id.to_string(),
        description: "".to_string(),
        author: "".to_string(),
        downloads: 0,
        follows: 0,
        icon_url: None,
        categories: vec![],
        versions: vec![],
        date_modified: "".to_string(),
        content_type: "mods".to_string(),
        slug: None,
        external_url: None,
        confidence: None,
        reason: None,
        install_state: None,
        install_summary: None,
    }
}

fn sample_discover_repo() -> GithubRepository {
    GithubRepository {
        full_name: "etianl/Trouser-Streak".to_string(),
        name: "Trouser-Streak".to_string(),
        description: Some("Meteor addon with mods for chunk tracing.".to_string()),
        stargazers_count: 500,
        forks_count: 12,
        archived: false,
        fork: false,
        disabled: false,
        html_url: "https://github.com/etianl/Trouser-Streak".to_string(),
        homepage: None,
        watchers_count: 500,
        open_issues_count: 1,
        pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
        updated_at: Some("2026-03-01T00:00:00Z".to_string()),
        topics: vec![],
        default_branch: "main".to_string(),
        owner: GithubOwner {
            login: "etianl".to_string(),
            owner_type: "User".to_string(),
        },
    }
}

fn sample_non_minecraft_ml_repo() -> GithubRepository {
    GithubRepository {
        full_name: "hwaluskle/tensorflow-generative-model-collections".to_string(),
        name: "tensorflow-generative-model-collections".to_string(),
        description: Some("Collection of generative models in Tensorflow.".to_string()),
        stargazers_count: 3900,
        forks_count: 840,
        archived: false,
        fork: false,
        disabled: false,
        html_url: "https://github.com/hwaluskle/tensorflow-generative-model-collections"
            .to_string(),
        homepage: None,
        watchers_count: 3900,
        open_issues_count: 1,
        pushed_at: Some("2026-03-01T00:00:00Z".to_string()),
        updated_at: Some("2026-03-01T00:00:00Z".to_string()),
        topics: vec![
            "tensorflow".to_string(),
            "model".to_string(),
            "gan".to_string(),
        ],
        default_branch: "main".to_string(),
        owner: GithubOwner {
            login: "hwaluskle".to_string(),
            owner_type: "User".to_string(),
        },
    }
}

#[test]
fn blend_discover_hits_prefers_modrinth_but_keeps_other_provider_visible() {
    let input = vec![
        make_hit("curseforge", "cf_1"),
        make_hit("modrinth", "mr_1"),
        make_hit("curseforge", "cf_2"),
        make_hit("modrinth", "mr_2"),
        make_hit("modrinth", "mr_3"),
    ];
    let blended = blend_discover_hits_prefer_modrinth(input);
    let order = blended
        .iter()
        .map(|hit| hit.project_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(order, vec!["mr_1", "mr_2", "cf_1", "mr_3", "cf_2"]);
}

#[test]
fn blend_discover_hits_passthrough_when_single_provider_present() {
    let input = vec![make_hit("modrinth", "mr_1"), make_hit("modrinth", "mr_2")];
    let blended = blend_discover_hits_prefer_modrinth(input);
    let order = blended
        .iter()
        .map(|hit| hit.project_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(order, vec!["mr_1", "mr_2"]);
}

#[test]
fn github_similarity_score_tolerates_common_typos() {
    let exact = github_name_similarity_score("meteor client", "meteor client");
    let typo = github_name_similarity_score("meteor client", "metor clint");
    assert!(exact >= typo);
    assert!(typo >= 20);
}

#[test]
fn discover_query_variants_include_shortened_form() {
    let variants = discover_query_variants("meteor client hacks");
    assert!(!variants.is_empty());
    assert!(variants.iter().any(|value| value.contains("meteor")));
}

#[test]
fn discover_query_variants_include_repeat_collapsed_typos() {
    let variants = discover_query_variants("sodiuumm modd");
    assert!(variants.iter().any(|value| value.contains("sodium")));
    assert!(variants.iter().any(|value| value.contains("mod")));
}

#[test]
fn sort_discover_hits_prefers_relevance_by_default() {
    let mut hits = vec![
        DiscoverSearchHit {
            project_id: "a".to_string(),
            title: "random utility".to_string(),
            downloads: 9000,
            ..make_hit("modrinth", "a")
        },
        DiscoverSearchHit {
            project_id: "b".to_string(),
            title: "meteor client".to_string(),
            downloads: 100,
            ..make_hit("modrinth", "b")
        },
    ];
    sort_discover_hits(&mut hits, "relevance", Some("metor"));
    assert_eq!(hits.first().map(|hit| hit.project_id.as_str()), Some("b"));
}

#[test]
fn github_signal_gate_rejects_low_similarity_without_minecraft_signal() {
    let repo = sample_discover_repo();
    assert!(!github_repo_passes_signal_gate(
        &repo,
        0,
        12,
        "trouser treaks"
    ));
}

#[test]
fn github_signal_gate_allows_high_similarity_without_minecraft_signal() {
    let repo = sample_discover_repo();
    assert!(github_repo_passes_signal_gate(
        &repo,
        0,
        40,
        "trouser treaks"
    ));
}

#[test]
fn github_signal_gate_allows_positive_minecraft_signal() {
    let repo = sample_discover_repo();
    assert!(github_repo_passes_signal_gate(&repo, 2, 0, "any"));
}

#[test]
fn github_mod_ecosystem_signal_does_not_confuse_model_with_mod() {
    let repo = sample_non_minecraft_ml_repo();
    assert!(github_repo_mod_ecosystem_signal_score(&repo) <= 0);
}

#[test]
fn github_lookup_queries_strip_local_version_noise() {
    let queries =
        github_lookup_queries_for_local_mod("Trouser-Streak-v1.5.8-fabric-1.21.1.jar", None);
    assert!(!queries.is_empty());
    assert!(queries
        .iter()
        .any(|query| query.contains("trouser") && query.contains("streak")));
}

#[test]
fn github_discover_search_queries_prioritize_typo_fallback_without_tokens() {
    let queries = github_discover_search_queries("Trouser Treaks", false);
    assert!(!queries.is_empty());
    assert!(queries
        .iter()
        .any(|q| q.contains("trouser in:name,description")));
    assert!(queries.len() <= GITHUB_UNAUTH_MAX_SEARCH_QUERIES);
}
