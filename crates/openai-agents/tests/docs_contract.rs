use std::fs;
use std::path::{Path, PathBuf};

fn contains_machine_local_absolute_path(contents: &str) -> bool {
    const UNIX_ROOTS: &[&str] = &[
        "/Users/",
        "/home/",
        "/root/",
        "/workspace/",
        "/workspaces/",
        "/github/workspace/",
        "/__w/",
    ];
    const WINDOWS_PATH_PREFIXES: &[&str] = &["Users", "a", "workspace", "workspaces"];

    if UNIX_ROOTS.iter().any(|root| contents.contains(root)) || contents.contains("\\Users\\") {
        return true;
    }

    contents.char_indices().any(|(idx, ch)| {
        if !ch.is_ascii_alphabetic() {
            return false;
        }

        let tail = &contents[idx..];
        let mut chars = tail.chars();
        let _drive = chars.next();
        matches!(chars.next(), Some(':'))
            && matches!(chars.next(), Some('\\' | '/'))
            && WINDOWS_PATH_PREFIXES.iter().any(|prefix| {
                chars
                    .as_str()
                    .strip_prefix(prefix)
                    .is_some_and(|tail| tail.starts_with(['\\', '/']))
            })
    })
}

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
            contains_machine_local_absolute_path(&contents).then_some(relative.to_owned())
        })
        .collect::<Vec<_>>();

    assert!(
        offenders.is_empty(),
        "Parity-adjacent docs contain machine-local absolute links: {}",
        offenders.join(", ")
    );
}

#[test]
fn portability_detector_rejects_non_macos_machine_local_absolute_paths() {
    for absolute_path in [
        "/home/alice/openai-agents-rust/docs/ROOT_EXPORT_PARITY.md",
        "/workspace/openai-agents-rust/docs/BEHAVIOR_PARITY.md",
        "/github/workspace/docs/PORTING_MATRIX.md",
        "/__w/openai-agents-rust/openai-agents-rust/docs/PORTING_PI_ID.md",
        "C:\\Users\\alice\\openai-agents-rust\\docs\\ROOT_EXPORT_PARITY.md",
        "D:\\a\\openai-agents-rust\\openai-agents-rust\\docs\\BEHAVIOR_PARITY.md",
        "C:/Users/alice/openai-agents-rust/docs/ROOT_EXPORT_PARITY.md",
        "D:/a/openai-agents-rust/openai-agents-rust/docs/BEHAVIOR_PARITY.md",
    ] {
        assert!(
            contains_machine_local_absolute_path(absolute_path),
            "expected portability detector to reject machine-local absolute path: {absolute_path}"
        );
    }

    for valid_relative_link in [
        "docs/ROOT_EXPORT_PARITY.md",
        "./docs/BEHAVIOR_PARITY.md",
        "../docs/PORTING_MATRIX.md",
        "[root exports](docs/ROOT_EXPORT_PARITY.md)",
        "reference/openai-agents-python/src/agents/__init__.py",
    ] {
        assert!(
            !contains_machine_local_absolute_path(valid_relative_link),
            "portability detector should not reject valid repo-relative path: {valid_relative_link}"
        );
    }
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
        "`run`",
        "`run_streamed`",
        "`run_with_session`",
        "`OpenAIProvider`",
        "`OpenAIResponsesModel`",
        "`OpenAIChatCompletionsModel`",
        "`realtime`",
        "`voice`",
        "`extensions`",
    ] {
        assert!(
            readme.contains(expected),
            "README should describe the shipped crate layout and facade surface, missing {expected}"
        );
    }
}
