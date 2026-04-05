use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn parse_python_root_exports(source: &str) -> BTreeSet<String> {
    let start = source.find("__all__ = [").expect("python __all__ start");
    let tail = &source[start..];
    let end = tail.find(']').expect("python __all__ end");
    let body = &tail[..end];

    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with('"') {
                return None;
            }
            line.split('"').nth(1).map(str::to_owned)
        })
        .collect()
}

fn parse_aliases(source: &str) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    let mut in_aliases = false;

    for line in source.lines() {
        let trimmed = line.trim();
        match trimmed {
            "## Aliased" => {
                in_aliases = true;
                continue;
            }
            "## Intentional Rust-First Omissions" => break,
            _ => {}
        }

        if !in_aliases || !trimmed.starts_with("- `") {
            continue;
        }

        let Some((left, right)) = trimmed.split_once(" -> ") else {
            continue;
        };
        let python_name = left.trim_start_matches("- `").trim_end_matches('`');
        let rust_name = right.trim().trim_matches('`');
        aliases.insert(python_name.to_owned(), rust_name.to_owned());
    }

    aliases
}

fn parse_omissions(source: &str) -> BTreeSet<String> {
    let mut omissions = BTreeSet::new();
    let mut in_omissions = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed == "## Intentional Rust-First Omissions" {
            in_omissions = true;
            continue;
        }
        if !in_omissions || !trimmed.starts_with("- `") {
            continue;
        }

        if let Some(name) = trimmed
            .trim_start_matches("- `")
            .split('`')
            .next()
            .filter(|name| !name.is_empty())
        {
            omissions.insert(name.to_owned());
        }
    }

    omissions
}

#[test]
fn python_root_exports_are_surfaced_or_documented() {
    let root = workspace_root();
    let python_init =
        fs::read_to_string(root.join("reference/openai-agents-python/src/agents/__init__.py"))
            .expect("python __init__.py");
    let facade = fs::read_to_string(root.join("crates/openai-agents/src/lib.rs"))
        .expect("rust facade lib.rs");
    let parity_doc =
        fs::read_to_string(root.join("docs/ROOT_EXPORT_PARITY.md")).expect("root parity doc");

    let exports = parse_python_root_exports(&python_init);
    let aliases = parse_aliases(&parity_doc);
    let omissions = parse_omissions(&parity_doc);

    let missing: Vec<String> = exports
        .into_iter()
        .filter(|name| {
            !(facade.contains(name)
                || aliases
                    .get(name)
                    .is_some_and(|alias_target| facade.contains(alias_target))
                || omissions.contains(name))
        })
        .map(|name| name.to_owned())
        .collect();

    assert!(
        missing.is_empty(),
        "Undocumented root export parity gaps: {}",
        missing.join(", ")
    );
}

#[test]
fn documented_aliases_and_omissions_stay_live() {
    let root = workspace_root();
    let python_init =
        fs::read_to_string(root.join("reference/openai-agents-python/src/agents/__init__.py"))
            .expect("python __init__.py");
    let facade = fs::read_to_string(root.join("crates/openai-agents/src/lib.rs"))
        .expect("rust facade lib.rs");
    let parity_doc =
        fs::read_to_string(root.join("docs/ROOT_EXPORT_PARITY.md")).expect("root parity doc");

    let exports = parse_python_root_exports(&python_init);
    let aliases = parse_aliases(&parity_doc);
    let omissions = parse_omissions(&parity_doc);

    let stale_aliases = aliases
        .iter()
        .filter_map(|(python_name, rust_name)| {
            (!exports.contains(python_name) || !facade.contains(rust_name))
                .then_some(format!("{python_name} -> {rust_name}"))
        })
        .collect::<Vec<_>>();
    let stale_omissions = omissions
        .iter()
        .filter(|name| !exports.contains(*name))
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        stale_aliases.is_empty(),
        "Root export parity aliases drifted from live Python exports or Rust facade exports: {}",
        stale_aliases.join(", ")
    );
    assert!(
        stale_omissions.is_empty(),
        "Root export parity omissions refer to non-existent Python exports: {}",
        stale_omissions.join(", ")
    );
}

#[test]
fn facade_exposes_documented_namespace_modules() {
    let root = workspace_root();
    let facade = fs::read_to_string(root.join("crates/openai-agents/src/lib.rs"))
        .expect("rust facade lib.rs");

    for namespace in ["realtime", "voice", "extensions"] {
        assert!(
            facade.contains(&format!("pub mod {namespace} {{")),
            "Facade is missing documented `{namespace}` namespace export"
        );
    }
}

#[test]
fn root_export_doc_describes_cross_crate_facade_surface() {
    let root = workspace_root();
    let facade = fs::read_to_string(root.join("crates/openai-agents/src/lib.rs"))
        .expect("rust facade lib.rs");
    let parity_doc =
        fs::read_to_string(root.join("docs/ROOT_EXPORT_PARITY.md")).expect("root parity doc");

    for surface in [
        "### Facade cross-crate surface",
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
            parity_doc.contains(surface),
            "Root export parity doc should describe facade surface item {surface}"
        );
    }

    for export in [
        "run,",
        "run_streamed,",
        "run_with_session,",
        "OpenAIProvider",
        "OpenAIResponsesModel",
        "OpenAIChatCompletionsModel",
    ] {
        assert!(
            facade.contains(export),
            "Facade is missing documented cross-crate export {export}"
        );
    }
}
