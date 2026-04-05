use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

#[test]
fn parity_adjacent_docs_do_not_contain_machine_local_absolute_links() {
    let root = workspace_root();
    let docs = [
        "docs/BEHAVIOR_PARITY.md",
        "docs/PORTING_MATRIX.md",
        "docs/PORTING_PI_ID.md",
        "docs/ROOT_EXPORT_PARITY.md",
    ];

    let offenders = docs
        .into_iter()
        .filter_map(|relative| {
            let contents = fs::read_to_string(root.join(relative)).expect("doc contents");
            (contents.contains("/Users/") || contents.contains("\\Users\\"))
                .then_some(relative.to_owned())
        })
        .collect::<Vec<_>>();

    assert!(
        offenders.is_empty(),
        "Parity-adjacent docs contain machine-local absolute links: {}",
        offenders.join(", ")
    );
}

#[test]
fn readme_describes_current_runtime_instead_of_bootstrap_scaffolding() {
    let root = workspace_root();
    let readme = fs::read_to_string(root.join("README.md")).expect("README");

    for stale_phrase in [
        "bootstrap mode",
        "public surface is scaffolded",
        "feature-complete port still needs to be implemented module by module",
        "subsystem scaffolding",
    ] {
        assert!(
            !readme.contains(stale_phrase),
            "README still contains outdated bootstrap/scaffold language: {stale_phrase}"
        );
    }

    for expected in [
        "`crates/agents-core`",
        "`crates/agents-openai`",
        "`crates/agents-realtime`",
        "`crates/agents-voice`",
        "`crates/agents-extensions`",
        "`crates/openai-agents`",
    ] {
        assert!(
            readme.contains(expected),
            "README should describe the shipped workspace crate layout, missing {expected}"
        );
    }
}
