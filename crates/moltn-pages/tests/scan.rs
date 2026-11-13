//! Scan du dossier des pages et génération TypeScript.

use moltn_pages::build::scan_dir;

fn pages(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    dir
}

#[test]
fn all_conventions_and_ignored_files() {
    let dir = pages(&[
        "index.vue",
        "about.vue",
        "users/index.vue",
        "users/[id].vue",
        "tabs/[[tab]].vue",
        "docs/[...slug].vue",
        "(admin)/stats.vue",
        "_components/Card.vue",
        "users/_Avatar.vue",
        "README.md",
        "react/page.tsx",
        "svelte/page.svelte",
        "jsx/page.jsx",
    ]);
    let found: Vec<(String, Vec<String>)> = scan_dir(dir.path())
        .unwrap()
        .into_iter()
        .map(|p| (p.component, p.routes))
        .collect();
    let expected: Vec<(String, Vec<String>)> = [
        ("(admin)/stats", vec!["/stats"]),
        ("about", vec!["/about"]),
        ("docs/[...slug]", vec!["/docs/{*slug}"]),
        ("index", vec!["/"]),
        ("jsx/page", vec!["/jsx/page"]),
        ("react/page", vec!["/react/page"]),
        ("svelte/page", vec!["/svelte/page"]),
        ("tabs/[[tab]]", vec!["/tabs", "/tabs/{tab}"]),
        ("users/[id]", vec!["/users/{id}"]),
        ("users/index", vec!["/users"]),
    ]
    .into_iter()
    .map(|(c, r)| (c.to_string(), r.into_iter().map(String::from).collect()))
    .collect();
    assert_eq!(found, expected);
}

#[test]
fn conflicts_are_all_listed_with_both_files() {
    let dir = pages(&[
        "about.vue",
        "about/index.vue",
        "users/[id].vue",
        "users/[slug].tsx",
        "(a)/x.vue",
        "(b)/x.vue",
        "opt/index.vue",
        "opt/[[tab]].vue",
    ]);
    let err = scan_dir(dir.path()).unwrap_err();
    let message = err.to_string();
    assert_eq!(err.problems.len(), 4, "{message}");
    assert!(
        message.contains("about.vue and about/index.vue produce the same route /about"),
        "{message}"
    );
    assert!(
        message.contains("users/[id].vue and users/[slug].tsx"),
        "{message}"
    );
    assert!(message.contains("(a)/x.vue and (b)/x.vue"), "{message}");
    assert!(
        message.contains("opt/[[tab]].vue and opt/index.vue")
            || message.contains("opt/index.vue and opt/[[tab]].vue"),
        "{message}"
    );
}

#[test]
fn invalid_segments_are_reported_per_file() {
    let dir = pages(&["[[tab]]/x.vue", "ok.vue", "[a-b].vue"]);
    let err = scan_dir(dir.path()).unwrap_err();
    assert_eq!(err.problems.len(), 2, "{err}");
    assert!(
        err.to_string()
            .contains(r#"[[tab]]/x.vue: "[[tab]]" is only allowed as the last segment"#),
        "{err}"
    );
}
