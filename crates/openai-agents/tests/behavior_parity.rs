use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_OMISSION_RATIONALE: &str = "Tracked upstream family; Rust parity is not yet closed for this family in the current runtime audit.";

const SECTION_ORDER: [&str; 11] = [
    "Core Runner",
    "Agent / Tool",
    "Sessions",
    "Model Settings / Providers",
    "OpenAI",
    "MCP",
    "Realtime",
    "Voice",
    "Tracing",
    "Extensions",
    "JS Package Families",
];

#[derive(Debug, Clone)]
struct OverrideRow {
    status: String,
    coverage: Vec<String>,
    notes: String,
}

#[derive(Debug, Clone)]
struct FamilyRow {
    status: String,
    coverage: Vec<String>,
    notes: String,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn parse_family_rows(source: &str) -> BTreeMap<String, FamilyRow> {
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
            let notes = columns[3].to_owned();
            Some((
                family,
                FamilyRow {
                    status,
                    coverage: coverage_paths,
                    notes,
                },
            ))
        })
        .collect()
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn upstream_python_family_list(root: &Path) -> Vec<String> {
    let tests_root = root.join("reference/openai-agents-python/tests");
    let mut files = Vec::new();
    collect_files(&tests_root, &mut files);
    files
        .into_iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("py"))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("test_"))
        })
        .map(|path| {
            path.strip_prefix(&tests_root)
                .expect("python test relative path")
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

fn upstream_python_families(root: &Path) -> BTreeSet<String> {
    upstream_python_family_list(root).into_iter().collect()
}

fn upstream_js_family_list(root: &Path) -> Vec<String> {
    let packages_root = root.join("reference/openai-agents-js/packages");
    let mut files = Vec::new();
    collect_files(&packages_root, &mut files);
    files
        .into_iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("ts"))
        .filter_map(|path| {
            let relative = path.strip_prefix(&packages_root).ok()?;
            let relative_text = relative.to_string_lossy().replace('\\', "/");
            let (package, rest) = relative_text.split_once('/')?;
            let test_prefix = "test/";
            let rest = rest.strip_prefix(test_prefix)?;
            let suffix = ".test.ts";
            let family = rest.strip_suffix(suffix)?;
            Some(format!("js/{package}/{family}"))
        })
        .collect()
}

fn upstream_js_families(root: &Path) -> BTreeSet<String> {
    upstream_js_family_list(root).into_iter().collect()
}

fn load_overrides(root: &Path) -> BTreeMap<String, OverrideRow> {
    let raw: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/behavior_parity_overrides.json"))
            .expect("behavior parity overrides"),
    )
    .expect("parse behavior parity overrides");
    raw.as_object()
        .expect("behavior parity override object")
        .iter()
        .map(|(family, value)| {
            let object = value.as_object().expect("override row");
            let status = object
                .get("status")
                .and_then(|value| value.as_str())
                .expect("override status")
                .to_owned();
            let coverage = object
                .get("coverage")
                .and_then(|value| value.as_array())
                .expect("override coverage")
                .iter()
                .map(|value| value.as_str().expect("coverage path").to_owned())
                .collect::<Vec<_>>();
            let notes = object
                .get("notes")
                .and_then(|value| value.as_str())
                .expect("override notes")
                .to_owned();
            (
                family.to_owned(),
                OverrideRow {
                    status,
                    coverage,
                    notes,
                },
            )
        })
        .collect()
}

fn section_for_family(family: &str) -> &'static str {
    if family.starts_with("js/") {
        return "JS Package Families";
    }
    if family.starts_with("extensions/")
        || matches!(family, "test_extension_filters" | "test_visualization")
    {
        return "Extensions";
    }
    if family.starts_with("voice/") {
        return "Voice";
    }
    if family.starts_with("realtime/") {
        return "Realtime";
    }
    if family.starts_with("mcp/") {
        return "MCP";
    }
    if family.starts_with("models/") || family.starts_with("model_settings/") {
        return "Model Settings / Providers";
    }
    if family.starts_with("tracing/")
        || family.starts_with("test_trace")
        || family.starts_with("test_tracing")
    {
        return "Tracing";
    }
    if family.starts_with("memory/")
        || family.starts_with("test_session")
        || family.starts_with("fastapi/")
    {
        return "Sessions";
    }
    if family.starts_with("test_openai")
        || family.starts_with("test_responses")
        || family.starts_with("test_server_conversation_tracker")
    {
        return "OpenAI";
    }
    if [
        "test_agent",
        "test_function",
        "test_tool",
        "test_handoff",
        "test_apply",
        "test_shell",
        "test_computer",
        "test_output_tool",
        "test_local_shell_tool",
        "test_visualization",
    ]
    .into_iter()
    .any(|prefix| family.starts_with(prefix))
    {
        return "Agent / Tool";
    }
    "Core Runner"
}

fn omission_rationale_for_family(family: &str) -> &'static str {
    match section_for_family(family) {
        "Core Runner" => {
            "Runner parity for this upstream family has not landed in the shared Rust runtime yet; keep it omitted until equivalent run semantics and executable tests exist."
        }
        "Agent / Tool" => {
            "Agent/tool parity for this upstream family is still missing a concrete Rust runtime surface and matching executable tests."
        }
        "Sessions" => {
            "Session parity for this upstream family is not wired through the Rust runtime yet, so it stays omitted until the session behavior and tests land."
        }
        "Model Settings / Providers" => {
            "Model-settings/provider parity for this upstream family is still open in Rust and needs an executable runtime contract before it can be covered."
        }
        "OpenAI" => {
            "OpenAI-specific parity for this upstream family remains open; leave it omitted until the corresponding provider/runtime behavior and tests ship."
        }
        "MCP" => {
            "MCP parity for this upstream family is still incomplete in the Rust runtime, so it remains omitted pending executable coverage."
        }
        "Realtime" => {
            "Realtime parity for this upstream family is not fully implemented in Rust yet; keep it omitted until the runtime path and tests exist."
        }
        "Voice" => {
            "Voice parity for this upstream family is still missing from the Rust runtime, so it remains omitted until executable coverage lands."
        }
        "Tracing" => {
            "Tracing parity for this upstream family has not been ported into the Rust runtime and test surface yet."
        }
        "Extensions" => {
            "Extension parity for this upstream family is still unimplemented or unverified in Rust, so the row stays omitted for now."
        }
        "JS Package Families" => {
            "This JS package-shape family still lacks an equivalent Rust facade/runtime contract with executable coverage, so it remains omitted."
        }
        _ => unreachable!("unknown section"),
    }
}

fn render_expected_behavior_parity_doc(root: &Path) -> String {
    let overrides = load_overrides(root);
    let tracked_families = upstream_python_family_list(root)
        .into_iter()
        .chain(upstream_js_family_list(root))
        .collect::<Vec<_>>();

    let mut rows = BTreeMap::<&str, Vec<(String, OverrideRow)>>::new();
    for family in &tracked_families {
        let row = overrides
            .get(family)
            .cloned()
            .unwrap_or_else(|| OverrideRow {
                status: "omitted-with-rationale".to_owned(),
                coverage: vec!["n/a".to_owned()],
                notes: omission_rationale_for_family(family).to_owned(),
            });
        rows.entry(section_for_family(family))
            .or_default()
            .push((family.clone(), row));
    }

    let mut output = vec![
        "# Behavior Parity".to_owned(),
        String::new(),
        "This document is generated from the pinned Python and JS test trees plus".to_owned(),
        "`docs/behavior_parity_overrides.json`.".to_owned(),
        String::new(),
        "Allowed statuses:".to_owned(),
        String::new(),
        "- `covered`: there is Rust coverage for the family and the runtime surface is materially present"
            .to_owned(),
        "- `omitted-with-rationale`: intentionally not closed yet or environment-specific; the omission is explicit"
            .to_owned(),
        String::new(),
        format!("Tracked upstream families: `{}`", tracked_families.len()),
        String::new(),
    ];

    for section in SECTION_ORDER {
        output.push(format!("### {section}"));
        output.push(String::new());
        output.push("| Family | Status | Rust coverage | Notes |".to_owned());
        output.push("| --- | --- | --- | --- |".to_owned());
        for (family, row) in rows.get(section).into_iter().flatten() {
            let coverage = row
                .coverage
                .iter()
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join(", ");
            output.push(format!(
                "| `{family}` | `{}` | {coverage} | {} |",
                row.status, row.notes
            ));
        }
        output.push(String::new());
    }

    format!("{}\n", output.join("\n").trim_end())
}

fn is_executable_validation_surface(path: &Path) -> bool {
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return false;
    }

    let source = fs::read_to_string(path).unwrap_or_default();
    source.contains("#[test]") || source.contains("#[tokio::test]") || source.contains("mod tests")
}

#[test]
fn behavior_parity_doc_covers_upstream_family_inventory() {
    let root = workspace_root();
    let parity_doc =
        fs::read_to_string(root.join("docs/BEHAVIOR_PARITY.md")).expect("behavior parity doc");
    let families = parse_family_rows(&parity_doc);
    let documented = families.keys().cloned().collect::<BTreeSet<_>>();
    let expected = upstream_python_families(&root)
        .into_iter()
        .chain(upstream_js_families(&root))
        .collect::<BTreeSet<_>>();

    let missing = expected
        .difference(&documented)
        .cloned()
        .collect::<Vec<_>>();
    let unexpected = documented
        .difference(&expected)
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "Missing behavior parity families: {}",
        missing.join(", ")
    );
    assert!(
        unexpected.is_empty(),
        "Behavior parity doc contains unexpected families: {}",
        unexpected.join(", ")
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

    for (family, row) in families {
        let status = row.status;
        let paths = row.coverage;
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

#[test]
fn behavior_parity_doc_matches_generator_inputs() {
    let root = workspace_root();
    let actual =
        fs::read_to_string(root.join("docs/BEHAVIOR_PARITY.md")).expect("behavior parity doc");
    let expected = render_expected_behavior_parity_doc(&root);

    assert_eq!(
        actual, expected,
        "docs/BEHAVIOR_PARITY.md is out of sync with scripts/generate_behavior_parity.py or docs/behavior_parity_overrides.json"
    );
}

#[test]
fn behavior_parity_overrides_only_target_live_families() {
    let root = workspace_root();
    let live_families = upstream_python_families(&root)
        .into_iter()
        .chain(upstream_js_families(&root))
        .collect::<BTreeSet<_>>();
    let stale_overrides = load_overrides(&root)
        .into_keys()
        .filter(|family| !live_families.contains(family))
        .collect::<Vec<_>>();

    assert!(
        stale_overrides.is_empty(),
        "Behavior parity overrides contain stale upstream families: {}",
        stale_overrides.join(", ")
    );
}

#[test]
fn covered_behavior_parity_rows_point_to_executable_validation_surfaces() {
    let root = workspace_root();
    let parity_doc =
        fs::read_to_string(root.join("docs/BEHAVIOR_PARITY.md")).expect("behavior parity doc");
    let families = parse_family_rows(&parity_doc);
    let non_executable = families
        .into_iter()
        .filter(|(_, row)| row.status == "covered")
        .filter_map(|(family, row)| {
            let has_executable_path = row
                .coverage
                .iter()
                .map(|relative| root.join(relative))
                .any(|path| is_executable_validation_surface(&path));
            (!has_executable_path).then_some(family)
        })
        .collect::<Vec<_>>();

    assert!(
        non_executable.is_empty(),
        "Covered behavior parity rows must reference at least one executable Rust validation surface: {}",
        non_executable.join(", ")
    );
}

#[test]
fn omitted_behavior_parity_rows_have_specific_rationales() {
    let root = workspace_root();
    let parity_doc =
        fs::read_to_string(root.join("docs/BEHAVIOR_PARITY.md")).expect("behavior parity doc");
    let placeholder_rows = parse_family_rows(&parity_doc)
        .into_iter()
        .filter(|(_, row)| row.status == "omitted-with-rationale")
        .filter_map(|(family, row)| {
            let placeholder = row.notes == DEFAULT_OMISSION_RATIONALE
                || row
                    .notes
                    .contains("not yet closed for this family in the current runtime audit")
                || row.notes.contains("Tracked upstream family");
            placeholder.then_some(family)
        })
        .collect::<Vec<_>>();

    assert!(
        placeholder_rows.is_empty(),
        "Omitted behavior parity rows still use placeholder rationales: {}",
        placeholder_rows.join(", ")
    );
}

#[test]
fn final_release_gate_rows_capture_latest_coverage_and_accepted_omissions() {
    let root = workspace_root();
    let parity_doc =
        fs::read_to_string(root.join("docs/BEHAVIOR_PARITY.md")).expect("behavior parity doc");
    let families = parse_family_rows(&parity_doc);

    for (family, coverage_path) in [
        (
            "test_call_model_input_filter",
            "crates/openai-agents/tests/runner_semantics.rs",
        ),
        (
            "test_call_model_input_filter_unit",
            "crates/agents-core/src/run.rs",
        ),
        ("voice/test_input", "crates/agents-voice/src/input.rs"),
        (
            "voice/test_openai_stt",
            "crates/agents-voice/src/models/openai_stt.rs",
        ),
        (
            "voice/test_openai_tts",
            "crates/agents-voice/src/models/openai_tts.rs",
        ),
        (
            "mcp/test_tool_filtering",
            "crates/agents-core/src/mcp/util.rs",
        ),
        (
            "realtime/test_session_payload_and_formats",
            "crates/agents-realtime/src/openai_realtime.rs",
        ),
        (
            "test_openai_chatcompletions_stream",
            "crates/agents-openai/src/models/chatcmpl_stream_handler.rs",
        ),
    ] {
        let row = families
            .get(family)
            .unwrap_or_else(|| panic!("missing behavior parity row for {family}"));
        assert_eq!(row.status, "covered", "{family} should be covered");
        assert!(
            row.coverage.iter().any(|path| path == coverage_path),
            "{family} should point at executable coverage path {coverage_path}, got {:?}",
            row.coverage
        );
    }

    let session_row = families
        .get("test_session")
        .expect("missing behavior parity row for test_session");
    assert_eq!(
        session_row.status, "omitted-with-rationale",
        "test_session should remain an explicit accepted omission"
    );
    for required_phrase in [
        "session_input_callback",
        "duplicate empty JSON object",
        "in-band marker",
        "public API change",
    ] {
        assert!(
            session_row.notes.contains(required_phrase),
            "test_session omission rationale should mention `{required_phrase}`, got: {}",
            session_row.notes
        );
    }

    let tracker_row = families
        .get("test_server_conversation_tracker")
        .expect("missing behavior parity row for test_server_conversation_tracker");
    assert_eq!(
        tracker_row.status, "omitted-with-rationale",
        "test_server_conversation_tracker should remain an explicit accepted omission"
    );
    for required_phrase in [
        "call_model_input_filter",
        "drops/reorders siblings",
        "multiple fresh replacement items",
        "ModelInputData",
        "provenance-aware API channel",
    ] {
        assert!(
            tracker_row.notes.contains(required_phrase),
            "test_server_conversation_tracker omission rationale should mention `{required_phrase}`, got: {}",
            tracker_row.notes
        );
    }
}
