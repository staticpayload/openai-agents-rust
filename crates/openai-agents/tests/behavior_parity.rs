use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn parse_family_rows(source: &str) -> BTreeMap<String, (String, Vec<String>)> {
    source
        .lines()
        .filter(|line| line.trim_start().starts_with("| `"))
        .filter_map(|line| {
            let columns = line
                .split('|')
                .map(str::trim)
                .filter(|column| !column.is_empty())
                .collect::<Vec<_>>();
            if columns.len() < 4 {
                return None;
            }

            let family = columns[0].trim_matches('`').to_owned();
            let status = columns[1].trim_matches('`').to_owned();
            let coverage_paths = columns[2]
                .split(',')
                .map(str::trim)
                .map(|path| path.trim_matches('`'))
                .filter(|path| !path.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            Some((family, (status, coverage_paths)))
        })
        .collect()
}

#[test]
fn behavior_parity_doc_covers_required_python_families() {
    let root = workspace_root();
    let parity_doc =
        fs::read_to_string(root.join("docs/BEHAVIOR_PARITY.md")).expect("behavior parity doc");
    let families = parse_family_rows(&parity_doc);

    let required = [
        "test_agent_runner",
        "test_agent_runner_streamed",
        "test_agent_runner_sync",
        "test_max_turns",
        "test_openai_conversations_session",
        "memory/test_openai_responses_compaction_session",
        "test_openai_responses",
        "test_openai_chatcompletions",
        "mcp/test_runner_calls_mcp",
        "mcp/test_mcp_server_manager",
        "mcp/test_mcp_resources",
        "realtime/test_runner",
        "realtime/test_session",
        "realtime/test_openai_realtime",
        "voice/test_pipeline",
        "voice/test_workflow",
        "test_responses_websocket_session",
        "js/agents-core/run_and_streaming",
        "js/agents-core/mcp",
        "js/agents-openai/responses_and_sessions",
        "js/agents-realtime/session",
        "js/agents-extensions/realtime_transports",
    ];

    let missing = required
        .iter()
        .filter(|family| !families.contains_key(**family))
        .copied()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "Missing behavior parity families: {}",
        missing.join(", ")
    );
}

#[test]
fn behavior_parity_doc_uses_allowed_statuses_and_existing_paths() {
    let root = workspace_root();
    let parity_doc =
        fs::read_to_string(root.join("docs/BEHAVIOR_PARITY.md")).expect("behavior parity doc");
    let families = parse_family_rows(&parity_doc);
    let allowed_statuses = ["covered", "omitted-with-rationale"];

    let mut invalid_statuses = Vec::new();
    let mut missing_paths = Vec::new();
    let mut partial_families = Vec::new();

    for (family, (status, paths)) in families {
        if status == "partial" {
            partial_families.push(family.clone());
        }
        if !allowed_statuses.contains(&status.as_str()) {
            invalid_statuses.push(format!("{family} -> {status}"));
        }
        if status != "omitted-with-rationale" {
            for path in paths {
                if !root.join(&path).exists() {
                    missing_paths.push(format!("{family} -> {path}"));
                }
            }
        }
    }

    assert!(
        invalid_statuses.is_empty(),
        "Behavior parity doc contains invalid statuses: {}",
        invalid_statuses.join(", ")
    );
    assert!(
        partial_families.is_empty(),
        "Behavior parity doc still contains partial families: {}",
        partial_families.join(", ")
    );
    assert!(
        missing_paths.is_empty(),
        "Behavior parity doc references missing coverage paths: {}",
        missing_paths.join(", ")
    );
}
