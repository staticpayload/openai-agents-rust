use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use openai_agents::sandbox::{
    Dir, File, LocalDir, LocalSandboxSession, Manifest, prepare_sandbox_run,
};
use openai_agents::{
    ApplyPatchOperation, InputItem, Model, ModelProvider, ModelRequest, ModelResponse, OutputItem,
    RunConfig, RunContext, RunContextWrapper, RunInterruptionKind, Runner, SandboxAgent,
    SandboxCapability, SandboxRunConfig, Tool, ToolContext, ToolOutput, Usage,
};
use serde_json::json;
use tokio::sync::Mutex;

#[derive(Clone, Default)]
struct CapturingSandboxModel {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

#[async_trait]
impl Model for CapturingSandboxModel {
    async fn generate(&self, request: ModelRequest) -> openai_agents::Result<ModelResponse> {
        self.requests.lock().await.push(request.clone());
        Ok(ModelResponse {
            model: request.model.clone(),
            output: vec![OutputItem::Text {
                text: request
                    .input
                    .last()
                    .and_then(|item| item.as_text())
                    .unwrap_or_default()
                    .to_owned(),
            }],
            usage: Usage::default(),
            response_id: Some("sandbox-response".to_owned()),
            request_id: Some("sandbox-request".to_owned()),
        })
    }
}

#[derive(Clone)]
struct CapturingSandboxProvider {
    model: Arc<dyn Model>,
}

impl ModelProvider for CapturingSandboxProvider {
    fn resolve(&self, _model: Option<&str>) -> Arc<dyn Model> {
        self.model.clone()
    }
}

#[derive(Clone, Default)]
struct ApprovalResumeSandboxModel {
    calls: Arc<Mutex<usize>>,
}

#[async_trait]
impl Model for ApprovalResumeSandboxModel {
    async fn generate(&self, request: ModelRequest) -> openai_agents::Result<ModelResponse> {
        let mut calls = self.calls.lock().await;
        *calls += 1;

        match *calls {
            1 => Ok(ModelResponse {
                model: request.model,
                output: vec![OutputItem::ToolCall {
                    call_id: "call-1".to_owned(),
                    tool_name: "sandbox_read_file".to_owned(),
                    arguments: json!({ "path": "/workspace/pre_resume.txt" }),
                    namespace: None,
                }],
                usage: Usage::default(),
                response_id: Some("sandbox-resume-response-1".to_owned()),
                request_id: Some("sandbox-resume-request-1".to_owned()),
            }),
            _ => {
                let resumed_text = request
                    .input
                    .iter()
                    .filter_map(|item| match item {
                        InputItem::Text { text } => Some(text.as_str()),
                        InputItem::Json { value } => value
                            .get("type")
                            .and_then(|kind| (kind == "tool_call_output").then_some(value))
                            .and_then(|value| value.get("output"))
                            .and_then(|output| {
                                output
                                    .get("text")
                                    .and_then(|text| text.as_str())
                                    .or_else(|| output.as_str())
                            }),
                    })
                    .find(|text| text.contains("persisted before approval"))
                    .unwrap_or_else(|| {
                        panic!(
                            "resumed input missing sandbox tool output: {:?}",
                            request.input
                        )
                    })
                    .to_owned();

                Ok(ModelResponse {
                    model: request.model,
                    output: vec![OutputItem::Text {
                        text: format!("resumed:{resumed_text}"),
                    }],
                    usage: Usage::default(),
                    response_id: Some("sandbox-resume-response-2".to_owned()),
                    request_id: Some("sandbox-resume-request-2".to_owned()),
                })
            }
        }
    }
}

fn tool_context(tool_name: &str) -> ToolContext {
    ToolContext::new(
        RunContextWrapper::new(RunContext::default()),
        tool_name,
        &format!("call-{tool_name}"),
        "{}",
    )
}

fn unique_temp_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    fs::canonicalize(std::env::temp_dir())
        .expect("temp dir should canonicalize")
        .join(format!("openai-agents-{label}-{nanos}"))
}

fn sandbox_temp_roots() -> Vec<PathBuf> {
    let mut roots = fs::read_dir(std::env::temp_dir())
        .expect("temp dir should be readable")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("openai-agents-sandbox-"))
        })
        .collect::<Vec<_>>();
    roots.sort();
    roots
}

fn localdir_hook_lock() -> &'static StdMutex<()> {
    static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| StdMutex::new(()))
}

#[tokio::test]
async fn sandbox_agent_preparation_exposes_defaults_and_workspace_context() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingSandboxProvider {
        model: Arc::new(CapturingSandboxModel {
            requests: requests.clone(),
        }),
    };
    let sandbox_agent = SandboxAgent::builder("sandbox")
        .instructions("Inspect the prepared workspace before editing files.")
        .build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(
                Manifest::default().with_entry("README.md", File::from_text("workspace readme\n")),
            ),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };

    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox run prepares");
    Runner::new()
        .with_model_provider(Arc::new(provider))
        .run(&prepared.agent, "summarize the workspace")
        .await
        .expect("prepared sandbox run succeeds");

    let request = requests.lock().await.remove(0);
    let tool_names = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec![
            "sandbox_list_files",
            "sandbox_read_file",
            "sandbox_run_shell",
            "sandbox_apply_patch",
        ]
    );
    let instructions = request.instructions.expect("sandbox instructions");
    assert!(instructions.contains("filesystem"));
    assert!(instructions.contains("shell"));
    assert!(instructions.contains("apply_patch"));
    assert!(instructions.contains("/workspace"));
    assert!(instructions.contains("README.md"));
    assert!(instructions.contains("Inspect the prepared workspace before editing files."));

    prepared.session.cleanup().expect("cleanup succeeds");
}

#[test]
fn sandbox_fresh_runs_use_isolated_temp_workspaces() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let manifest = Manifest::default().with_entry(
        "notes/todo.txt",
        File::from_text("runner-owned workspace\n"),
    );
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(manifest.clone()),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };

    let prepared_a = prepare_sandbox_run(&sandbox_agent, &run_config).expect("first prep succeeds");
    let prepared_b =
        prepare_sandbox_run(&sandbox_agent, &run_config).expect("second prep succeeds");

    let root_a = prepared_a.session.workspace_root();
    let root_b = prepared_b.session.workspace_root();
    assert!(root_a.is_absolute());
    assert!(root_b.is_absolute());
    assert_ne!(root_a, root_b);
    assert!(prepared_a.session.runner_owned());
    assert!(prepared_b.session.runner_owned());
    assert_eq!(manifest.root, "/workspace");
    assert_ne!(root_a, PathBuf::from(manifest.root.clone()));
    assert!(root_a.join("notes/todo.txt").is_file());
    assert!(root_b.join("notes/todo.txt").is_file());

    prepared_a
        .session
        .cleanup()
        .expect("first cleanup succeeds");
    prepared_b
        .session
        .cleanup()
        .expect("second cleanup succeeds");

    assert!(!root_a.exists());
    assert!(!root_b.exists());
}

#[tokio::test]
async fn sandbox_manifest_entries_are_ready_before_first_tool_call() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let source_root = unique_temp_path("sandbox-localdir-source");
    if source_root.exists() {
        fs::remove_dir_all(&source_root).expect("remove stale source root");
    }
    fs::create_dir_all(source_root.join("nested")).expect("create localdir source");
    fs::write(source_root.join("nested/data.txt"), "copied bytes\n").expect("write source file");

    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(
                Manifest::default()
                    .with_entry("notes.txt", File::from_text("inline bytes\n"))
                    .with_entry(
                        "project",
                        Dir::new().with_entry("src/lib.rs", File::from_text("pub fn demo() {}\n")),
                    )
                    .with_entry("copied", LocalDir::new(&source_root)),
            ),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };

    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox prep succeeds");
    let list_tool = prepared
        .agent
        .find_function_tool("sandbox_list_files", None)
        .expect("list tool is attached");
    let read_tool = prepared
        .agent
        .find_function_tool("sandbox_read_file", None)
        .expect("read tool is attached");

    let root_listing = list_tool
        .invoke(
            tool_context("sandbox_list_files"),
            serde_json::json!({ "path": "/workspace" }),
        )
        .await
        .expect("list tool runs");
    let ToolOutput::Text(root_listing) = root_listing else {
        panic!("list tool should return text output");
    };
    let root_listing = root_listing.text;
    assert!(root_listing.contains("notes.txt"));
    assert!(root_listing.contains("project"));
    assert!(root_listing.contains("copied"));

    let inline_bytes = read_tool
        .invoke(
            tool_context("sandbox_read_file"),
            serde_json::json!({ "path": "/workspace/notes.txt" }),
        )
        .await
        .expect("inline file is readable");
    let ToolOutput::Text(inline_bytes) = inline_bytes else {
        panic!("read tool should return text output");
    };
    let inline_bytes = inline_bytes.text;
    assert_eq!(inline_bytes, "inline bytes\n");

    let nested_bytes = read_tool
        .invoke(
            tool_context("sandbox_read_file"),
            serde_json::json!({ "path": "/workspace/project/src/lib.rs" }),
        )
        .await
        .expect("dir child is readable");
    let ToolOutput::Text(nested_bytes) = nested_bytes else {
        panic!("read tool should return text output");
    };
    let nested_bytes = nested_bytes.text;
    assert_eq!(nested_bytes, "pub fn demo() {}\n");

    let copied_bytes = read_tool
        .invoke(
            tool_context("sandbox_read_file"),
            serde_json::json!({ "path": "/workspace/copied/nested/data.txt" }),
        )
        .await
        .expect("localdir file is readable");
    let ToolOutput::Text(copied_bytes) = copied_bytes else {
        panic!("read tool should return text output");
    };
    let copied_bytes = copied_bytes.text;
    assert_eq!(copied_bytes, "copied bytes\n");

    prepared.session.cleanup().expect("cleanup succeeds");
    fs::remove_dir_all(&source_root).expect("remove source root");
}

#[tokio::test]
async fn sandbox_capability_subsets_limit_attached_tool_bundle() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingSandboxProvider {
        model: Arc::new(CapturingSandboxModel {
            requests: requests.clone(),
        }),
    };
    let sandbox_agent = SandboxAgent::builder("sandbox")
        .instructions("Only use the allowed sandbox capabilities.")
        .capabilities(vec![SandboxCapability::Filesystem])
        .build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(
                Manifest::default().with_entry("README.md", File::from_text("workspace readme\n")),
            ),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };

    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox run prepares");
    Runner::new()
        .with_model_provider(Arc::new(provider))
        .run(&prepared.agent, "summarize the workspace")
        .await
        .expect("prepared sandbox run succeeds");

    let request = requests.lock().await.remove(0);
    let tool_names = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(tool_names, vec!["sandbox_list_files", "sandbox_read_file"]);

    let instructions = request.instructions.expect("sandbox instructions");
    assert!(instructions.contains("filesystem"));
    assert!(!instructions.contains("shell"));
    assert!(!instructions.contains("apply_patch"));

    let list_tool = prepared
        .agent
        .find_function_tool("sandbox_list_files", None)
        .expect("filesystem subset keeps list tool");
    let read_tool = prepared
        .agent
        .find_function_tool("sandbox_read_file", None)
        .expect("filesystem subset keeps read tool");
    assert!(
        prepared
            .agent
            .find_function_tool("sandbox_run_shell", None)
            .is_none(),
        "shell tool should be omitted from the attached bundle"
    );
    assert!(
        prepared
            .agent
            .find_function_tool("sandbox_apply_patch", None)
            .is_none(),
        "apply_patch tool should be omitted from the attached bundle"
    );

    let listing = list_tool
        .invoke(
            tool_context("sandbox_list_files"),
            serde_json::json!({ "path": "/workspace" }),
        )
        .await
        .expect("filesystem subset list tool runs");
    let ToolOutput::Text(listing) = listing else {
        panic!("list tool should return text output");
    };
    assert!(listing.text.contains("README.md"));

    let contents = read_tool
        .invoke(
            tool_context("sandbox_read_file"),
            serde_json::json!({ "path": "/workspace/README.md" }),
        )
        .await
        .expect("filesystem subset read tool runs");
    let ToolOutput::Text(contents) = contents else {
        panic!("read tool should return text output");
    };
    assert_eq!(contents.text, "workspace readme\n");

    prepared.session.cleanup().expect("cleanup succeeds");
}

#[tokio::test]
async fn sandbox_duplicate_capability_entries_do_not_create_duplicate_runtime_tool_names() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingSandboxProvider {
        model: Arc::new(CapturingSandboxModel {
            requests: requests.clone(),
        }),
    };
    let sandbox_agent = SandboxAgent::builder("sandbox")
        .instructions("Only use the allowed sandbox capabilities.")
        .capabilities(vec![
            SandboxCapability::Filesystem,
            SandboxCapability::Filesystem,
            SandboxCapability::ApplyPatch,
            SandboxCapability::ApplyPatch,
        ])
        .build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(
                Manifest::default().with_entry("README.md", File::from_text("workspace readme\n")),
            ),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };

    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox run prepares");
    Runner::new()
        .with_model_provider(Arc::new(provider))
        .run(&prepared.agent, "summarize the workspace")
        .await
        .expect("prepared sandbox run succeeds");

    let request = requests.lock().await.remove(0);
    let tool_names = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec![
            "sandbox_list_files",
            "sandbox_read_file",
            "sandbox_apply_patch"
        ]
    );

    let instructions = request.instructions.expect("sandbox instructions");
    assert_eq!(instructions.matches("filesystem").count(), 1);
    assert_eq!(instructions.matches("apply_patch").count(), 1);
    assert!(!instructions.contains("shell"));

    let list_tool = prepared
        .agent
        .find_function_tool("sandbox_list_files", None)
        .expect("filesystem subset keeps list tool");
    let read_tool = prepared
        .agent
        .find_function_tool("sandbox_read_file", None)
        .expect("filesystem subset keeps read tool");
    let patch_tool = prepared
        .agent
        .find_function_tool("sandbox_apply_patch", None)
        .expect("apply_patch subset keeps patch tool");

    let listing = list_tool
        .invoke(
            tool_context("sandbox_list_files"),
            serde_json::json!({ "path": "/workspace" }),
        )
        .await
        .expect("deduped list tool runs");
    let ToolOutput::Text(listing) = listing else {
        panic!("list tool should return text output");
    };
    assert!(listing.text.contains("README.md"));

    let contents = read_tool
        .invoke(
            tool_context("sandbox_read_file"),
            serde_json::json!({ "path": "/workspace/README.md" }),
        )
        .await
        .expect("deduped read tool runs");
    let ToolOutput::Text(contents) = contents else {
        panic!("read tool should return text output");
    };
    assert_eq!(contents.text, "workspace readme\n");

    let patched = patch_tool
        .invoke(
            tool_context("sandbox_apply_patch"),
            serde_json::json!({
                "path": "/workspace/README.md",
                "replacement": "patched workspace readme\n"
            }),
        )
        .await
        .expect("deduped patch tool runs");
    let ToolOutput::Text(patched) = patched else {
        panic!("patch tool should return text output");
    };
    assert_eq!(patched.text, "patched /workspace/README.md");
    assert_eq!(
        prepared
            .session
            .read_file("/workspace/README.md")
            .expect("patched README remains readable"),
        "patched workspace readme\n"
    );

    prepared.session.cleanup().expect("cleanup succeeds");
}

#[test]
fn localdir_staging_rejects_symlinked_ancestor_sources() {
    let _guard = localdir_hook_lock().lock().expect("hook lock");
    let source_root = unique_temp_path("sandbox-localdir-symlink-root");
    let real_parent = source_root.join("real-parent");
    let nested = real_parent.join("nested");
    fs::create_dir_all(&nested).expect("create source tree");
    fs::write(real_parent.join("plain.txt"), "plain\n").expect("write plain file");
    fs::write(nested.join("real.txt"), "real\n").expect("write nested file");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_parent, source_root.join("linked-parent"))
        .expect("create ancestor symlink");

    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let linked_source = source_root.join("linked-parent");
    let manifest = Manifest::default().with_entry("copied", LocalDir::new(&linked_source));
    let before_temp_roots = sandbox_temp_roots();
    let error = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                manifest: Some(manifest),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect_err("symlinked localdir source should be rejected");
    let after_temp_roots = sandbox_temp_roots();

    let message = error.to_string();
    assert!(message.contains("symlink"), "unexpected error: {message}");
    assert!(
        message.contains("linked-parent"),
        "unexpected error: {message}"
    );
    assert_eq!(
        before_temp_roots, after_temp_roots,
        "failed staging should not leave sandbox temp roots behind"
    );

    let stable_root = unique_temp_path("sandbox-localdir-stable-root");
    fs::create_dir_all(stable_root.join("src")).expect("create stable source tree");
    fs::write(stable_root.join("src/lib.rs"), "pub fn ok() {}\n").expect("write stable file");

    let prepared = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                manifest: Some(
                    Manifest::default().with_entry("copied", LocalDir::new(&stable_root)),
                ),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("stable real directory should stage successfully");

    let copied = prepared
        .session
        .read_file("/workspace/copied/src/lib.rs")
        .expect("staged file should be readable");
    assert_eq!(copied, "pub fn ok() {}\n");

    let workspace_root = prepared.session.workspace_root();
    assert!(workspace_root.join("copied/src/lib.rs").is_file());

    prepared.session.cleanup().expect("cleanup succeeds");
    assert!(
        !workspace_root.exists(),
        "runner-owned workspace should be removed"
    );

    fs::remove_dir_all(&source_root).expect("remove source root");
    fs::remove_dir_all(&stable_root).expect("remove stable source root");
}

#[test]
fn localdir_staging_rejects_live_source_swaps() {
    let _guard = localdir_hook_lock().lock().expect("hook lock");
    let source_root = unique_temp_path("localdir-swap-root");
    fs::create_dir_all(&source_root).expect("create source root");
    for index in 0..8 {
        fs::write(
            source_root.join(format!("file-{index}.txt")),
            "stable bytes\n".repeat(32_768),
        )
        .expect("write source file");
    }

    let swapped_root = unique_temp_path("localdir-swapped-root");
    fs::create_dir_all(&swapped_root).expect("create swapped root");
    fs::write(swapped_root.join("alt.txt"), "swapped bytes\n").expect("write swapped file");

    let trigger_path = unique_temp_path("localdir-hook-trigger");
    let release_path = unique_temp_path("localdir-hook-release");
    let hook_value = format!(
        "{}|{}|{}",
        source_root.display(),
        trigger_path.display(),
        release_path.display()
    );
    let before_temp_roots = sandbox_temp_roots();
    // SAFETY: the test serializes access to the process environment with `localdir_hook_lock`.
    unsafe {
        std::env::set_var("OPENAI_AGENTS_SANDBOX_LOCALDIR_TEST_HOOK", &hook_value);
    }

    let source_root_for_thread = source_root.clone();
    let handle = thread::spawn(move || {
        let sandbox_agent = SandboxAgent::builder("sandbox").build();
        let manifest =
            Manifest::default().with_entry("copied", LocalDir::new(&source_root_for_thread));
        prepare_sandbox_run(
            &sandbox_agent,
            &RunConfig {
                sandbox: Some(SandboxRunConfig {
                    manifest: Some(manifest),
                    ..SandboxRunConfig::default()
                }),
                ..RunConfig::default()
            },
        )
    });

    for _ in 0..200 {
        if trigger_path.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(trigger_path.exists(), "copy should reach the hook");

    let backup_root = unique_temp_path("localdir-swap-backup");
    fs::rename(&source_root, &backup_root).expect("move source root out of the way");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&swapped_root, &source_root).expect("swap source root to symlink");
    fs::write(&release_path, "release\n").expect("release hook");

    let error = handle
        .join()
        .expect("copy thread should join")
        .expect_err("swapped source should be rejected");

    // SAFETY: the test serializes access to the process environment with `localdir_hook_lock`.
    unsafe {
        std::env::remove_var("OPENAI_AGENTS_SANDBOX_LOCALDIR_TEST_HOOK");
    }

    let after_temp_roots = sandbox_temp_roots();
    let message = error.to_string();
    assert!(
        message.contains("changed during copy") || message.contains("symlink"),
        "unexpected error: {message}"
    );
    assert_eq!(
        before_temp_roots, after_temp_roots,
        "failed staging should not leave sandbox temp roots behind"
    );

    #[cfg(unix)]
    fs::remove_file(&source_root).expect("remove swapped symlink");
    fs::remove_file(&trigger_path).expect("remove trigger");
    fs::remove_file(&release_path).expect("remove release");
    fs::remove_dir_all(&backup_root).expect("remove original source root");
    fs::remove_dir_all(&swapped_root).expect("remove swapped root");
}

#[test]
fn sandbox_paths_reject_workspace_escape() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(Manifest::default().with_entry("notes.txt", File::from_text("hello\n"))),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };
    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox prep succeeds");

    let inside_path = "/workspace/notes.txt";
    assert_eq!(
        prepared
            .session
            .read_file(inside_path)
            .expect("inside reads should succeed"),
        "hello\n"
    );

    prepared
        .session
        .write_file("/workspace/generated.txt", "generated\n")
        .expect("inside writes should succeed");
    assert_eq!(
        prepared
            .session
            .read_file("generated.txt")
            .expect("relative inside reads should succeed"),
        "generated\n"
    );

    prepared
        .session
        .apply_patch(ApplyPatchOperation {
            path: "/workspace/generated.txt".to_owned(),
            replacement: "patched\n".to_owned(),
        })
        .expect("inside patch should succeed");
    assert_eq!(
        prepared
            .session
            .read_file("/workspace/generated.txt")
            .expect("patched file should be readable"),
        "patched\n"
    );

    let host_outside = unique_temp_path("sandbox-host-outside");
    fs::write(&host_outside, "host-secret\n").expect("write host file");

    #[cfg(unix)]
    std::os::unix::fs::symlink(
        &host_outside,
        prepared.session.workspace_root().join("escape-link"),
    )
    .expect("create workspace escape symlink");

    for escaped in [
        "../outside.txt",
        "/tmp/outside.txt",
        "/workspace/escape-link",
    ] {
        let error = prepared
            .session
            .read_file(escaped)
            .expect_err("escape read should fail");
        assert!(
            error
                .to_string()
                .contains("path must stay within the sandbox workspace"),
            "unexpected error: {error}"
        );
    }

    let write_error = prepared
        .session
        .write_file("/workspace/escape-link", "mutated\n")
        .expect_err("write through escape symlink should fail");
    assert!(
        write_error
            .to_string()
            .contains("path must stay within the sandbox workspace"),
        "unexpected error: {write_error}"
    );
    assert_eq!(
        fs::read_to_string(&host_outside).expect("host file should remain readable"),
        "host-secret\n"
    );

    let patch_error = prepared
        .session
        .apply_patch(ApplyPatchOperation {
            path: "/workspace/escape-link".to_owned(),
            replacement: "patched-host\n".to_owned(),
        })
        .expect_err("patch through escape symlink should fail");
    assert!(
        patch_error
            .to_string()
            .contains("path must stay within the sandbox workspace"),
        "unexpected error: {patch_error}"
    );
    assert_eq!(
        fs::read_to_string(&host_outside).expect("host file should remain unchanged"),
        "host-secret\n"
    );

    prepared.session.cleanup().expect("cleanup succeeds");
    fs::remove_file(&host_outside).expect("remove host file");
}

#[test]
fn sandbox_local_shell_starts_in_workspace_and_blocks_escape() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let host_outside = unique_temp_path("sandbox-shell-outside");
    if host_outside.exists() {
        fs::remove_file(&host_outside).expect("remove stale host outside file");
    }

    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(Manifest::default().with_entry("notes.txt", File::from_text("hello\n"))),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };
    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox prep succeeds");

    let pwd = prepared
        .session
        .run_shell("pwd")
        .expect("pwd should succeed");
    assert_eq!(pwd.exit_code, 0);
    let reported_pwd =
        fs::canonicalize(pwd.stdout.trim()).expect("reported pwd should canonicalize");
    let workspace_root =
        fs::canonicalize(prepared.session.workspace_root()).expect("workspace root canonicalizes");
    assert_eq!(reported_pwd, workspace_root);

    let write_inside = prepared
        .session
        .run_shell("printf 'shell-write\\n' > shell-created.txt && cat shell-created.txt")
        .expect("inside write should succeed");
    assert_eq!(write_inside.exit_code, 0);
    assert_eq!(write_inside.stdout, "shell-write\n");
    assert_eq!(
        prepared
            .session
            .read_file("/workspace/shell-created.txt")
            .expect("shell-created file is readable"),
        "shell-write\n"
    );

    let missing = prepared
        .session
        .run_shell("missing-sandbox-command")
        .expect("missing command should return shell status");
    assert_eq!(missing.exit_code, 127);
    assert!(missing.stderr.contains("missing-sandbox-command"));

    let denied_script = prepared.session.workspace_root().join("denied.sh");
    fs::write(&denied_script, "#!/bin/sh\necho denied\n").expect("write denied script");
    let denied = prepared
        .session
        .run_shell("./denied.sh")
        .expect("permission denied should surface shell status");
    assert_eq!(denied.exit_code, 126);
    assert!(denied.stderr.contains("Permission denied"));

    let escape = prepared
        .session
        .run_shell(&format!("printf 'escape\\n' > {}", host_outside.display()))
        .expect_err("outside write should be blocked");
    assert!(
        escape
            .to_string()
            .contains("shell command must stay within the sandbox workspace"),
        "unexpected error: {escape}"
    );
    assert!(
        !host_outside.exists(),
        "outside path should not be created by sandbox shell"
    );

    prepared.session.cleanup().expect("cleanup succeeds");
}

#[test]
fn sandbox_local_shell_blocks_interpreter_and_expansion_escapes() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let host_outside = unique_temp_path("sandbox-shell-interpreter-escape");
    if host_outside.exists() {
        fs::remove_file(&host_outside).expect("remove stale host outside file");
    }

    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(Manifest::default()),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };
    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox prep succeeds");

    let interpreter = prepared
        .session
        .run_shell(&format!(
            "perl -e \"open my \\$fh, '>', q[{}] or die \\$!; print {{\\$fh}} q[escape]; close \\$fh;\"",
            host_outside.display()
        ))
        .expect("interpreter command should run under confinement");
    assert_ne!(interpreter.exit_code, 0);
    assert!(
        interpreter.stderr.contains("Operation not permitted")
            || interpreter.stderr.contains("Permission denied")
            || interpreter.stderr.contains("denied"),
        "unexpected interpreter stderr: {}",
        interpreter.stderr
    );
    assert!(
        !host_outside.exists(),
        "outside path should not be created through interpreter escape"
    );

    let expansion = prepared
        .session
        .run_shell(&format!(
            "target='{}'; printf 'escape\\n' > \"$target\"",
            host_outside.display()
        ))
        .expect("expansion command should run under confinement");
    assert_ne!(expansion.exit_code, 0);
    assert!(
        !host_outside.exists(),
        "outside path should not be created through shell expansion escape"
    );

    let nested = prepared
        .session
        .run_shell(&format!(
            "sh -lc \"printf 'escape\\\\n' > {}\"",
            host_outside.display()
        ))
        .expect("nested shell command should run under confinement");
    assert_ne!(nested.exit_code, 0);
    assert!(
        !host_outside.exists(),
        "outside path should not be created through nested shell escape"
    );

    prepared.session.cleanup().expect("cleanup succeeds");
}

#[tokio::test]
async fn injected_live_sessions_persist_changes_across_runs() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = CapturingSandboxProvider {
        model: Arc::new(CapturingSandboxModel {
            requests: requests.clone(),
        }),
    };
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let live_session = LocalSandboxSession::create_caller_owned(
        Manifest::default().with_entry("notes.txt", File::from_text("caller-owned workspace\n")),
    )
    .expect("caller-owned live session should initialize");
    let workspace_root = live_session.workspace_root();
    assert!(!live_session.runner_owned());

    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            session: Some(live_session.clone()),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };

    let first = prepare_sandbox_run(&sandbox_agent, &run_config).expect("first run prepares");
    Runner::new()
        .with_model_provider(Arc::new(provider.clone()))
        .run(&first.agent, "first run")
        .await
        .expect("first injected-session run succeeds");
    first
        .session
        .write_file("/workspace/persisted.txt", "written once\n")
        .expect("workspace edit should succeed");
    assert_eq!(
        first
            .session
            .read_file("/workspace/persisted.txt")
            .expect("persisted file should be readable after first run"),
        "written once\n"
    );
    first
        .session
        .cleanup()
        .expect("caller-owned cleanup should be a no-op");
    assert!(
        workspace_root.exists(),
        "caller-owned session workspace should remain after runner cleanup"
    );

    let second = prepare_sandbox_run(&sandbox_agent, &run_config).expect("second run prepares");
    Runner::new()
        .with_model_provider(Arc::new(provider))
        .run(&second.agent, "second run")
        .await
        .expect("second injected-session run succeeds");

    assert_eq!(second.session.workspace_root(), workspace_root);
    assert!(!second.session.runner_owned());

    let read_tool = second
        .agent
        .find_function_tool("sandbox_read_file", None)
        .expect("read tool is attached");
    let persisted = read_tool
        .invoke(
            tool_context("sandbox_read_file"),
            serde_json::json!({ "path": "/workspace/persisted.txt" }),
        )
        .await
        .expect("persisted file should remain readable on the next run");
    let ToolOutput::Text(persisted) = persisted else {
        panic!("read tool should return text output");
    };
    assert_eq!(persisted.text, "written once\n");

    assert_eq!(
        requests.lock().await.len(),
        2,
        "both runs should reach the model without recreating the workspace"
    );

    second
        .session
        .cleanup()
        .expect("caller-owned cleanup should remain a no-op");
    assert!(
        workspace_root.exists(),
        "caller-owned session should remain caller-managed after the second run"
    );

    fs::remove_dir_all(&workspace_root).expect("caller cleans up injected live session");
}

#[test]
fn running_sessions_reject_unsupported_manifest_mutations() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let live_session = LocalSandboxSession::create_caller_owned(
        Manifest::default().with_entry("existing.txt", File::from_text("existing\n")),
    )
    .expect("caller-owned live session should initialize");
    let workspace_root = live_session.workspace_root();

    let updated_manifest = live_session
        .manifest()
        .with_entry("added.txt", File::from_text("added\n"));
    let updated = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session: Some(live_session.clone()),
                manifest: Some(updated_manifest.clone()),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("compatible manifest additions should apply");
    assert_eq!(
        updated
            .session
            .read_file("/workspace/added.txt")
            .expect("added manifest file should materialize"),
        "added\n"
    );
    assert_eq!(updated.session.manifest(), updated_manifest);

    let invalid_root_manifest = Manifest {
        root: "/other-workspace".to_owned(),
        ..updated.session.manifest()
    }
    .with_entry("root-change.txt", File::from_text("blocked\n"));
    let before_root_change = live_session.manifest();
    let root_change = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session: Some(live_session.clone()),
                manifest: Some(invalid_root_manifest),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect_err("running session should reject root mutation");
    assert!(root_change.to_string().contains("`manifest.root`"));
    assert_eq!(live_session.manifest(), before_root_change);
    assert!(
        !workspace_root.join("root-change.txt").exists(),
        "unsupported root mutation should not partially materialize new files"
    );

    let removal_manifest = Manifest::default().with_entry("added.txt", File::from_text("added\n"));
    let before_removal = live_session.manifest();
    let removal = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session: Some(live_session.clone()),
                manifest: Some(
                    removal_manifest
                        .with_entry("should-not-appear.txt", File::from_text("blocked\n")),
                ),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect_err("running session should reject entry removal");
    assert!(removal.to_string().contains("removing manifest entries"));
    assert_eq!(live_session.manifest(), before_removal);
    assert!(
        !workspace_root.join("should-not-appear.txt").exists(),
        "rejected removals should not partially apply other entry changes"
    );

    let replacement_manifest = Manifest::default()
        .with_entry(
            "existing.txt",
            Dir::new().with_entry("nested.txt", File::from_text("blocked\n")),
        )
        .with_entry("added.txt", File::from_text("added\n"))
        .with_entry("replacement-side-effect.txt", File::from_text("blocked\n"));
    let before_replacement = live_session.manifest();
    let replacement = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session: Some(live_session.clone()),
                manifest: Some(replacement_manifest),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect_err("running session should reject entry type replacement");
    assert!(
        replacement
            .to_string()
            .contains("replacing manifest entry types")
    );
    assert_eq!(live_session.manifest(), before_replacement);
    assert!(
        !workspace_root.join("replacement-side-effect.txt").exists(),
        "rejected type replacements should not partially apply other entry changes"
    );

    fs::remove_dir_all(workspace_root).expect("caller cleans up injected live session");
}

#[tokio::test]
async fn caller_owned_sessions_are_not_serialized_into_run_state() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let provider = Arc::new(CapturingSandboxProvider {
        model: Arc::new(ApprovalResumeSandboxModel::default()),
    });
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let live_session = LocalSandboxSession::create_caller_owned(
        Manifest::default().with_entry("notes.txt", File::from_text("caller-owned workspace\n")),
    )
    .expect("caller-owned live session should initialize");
    let workspace_root = live_session.workspace_root();
    assert!(!live_session.runner_owned());

    let prepared = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session: Some(live_session.clone()),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("sandbox prep succeeds");

    let approval_gated_agent = prepared.agent.clone_with(|agent| {
        let read_tool = agent
            .find_function_tool("sandbox_read_file", None)
            .expect("sandbox read tool is attached")
            .clone()
            .with_needs_approval(true);
        agent
            .function_tools
            .retain(|tool| tool.definition.name != "sandbox_read_file");
        agent.function_tools.push(read_tool);
    });

    let result = Runner::new()
        .with_model_provider(provider)
        .run(
            &approval_gated_agent,
            "serialize caller-owned sandbox state",
        )
        .await
        .expect("caller-owned run should interrupt cleanly");

    assert!(result.final_output.is_none());
    assert!(matches!(
        result
            .interruptions
            .first()
            .and_then(|step| step.kind.clone()),
        Some(RunInterruptionKind::ToolApproval)
    ));

    let durable_state = result
        .durable_state()
        .expect("interrupted run should expose durable state");
    assert!(
        durable_state.sandbox.is_none(),
        "caller-owned injected sessions must be omitted from durable run state"
    );

    let serialized_state =
        serde_json::to_value(durable_state).expect("run state should serialize cleanly");
    assert!(
        serialized_state.get("sandbox").is_none(),
        "serialized run state should omit caller-owned sandbox payloads"
    );
    assert!(
        workspace_root.exists(),
        "caller-owned workspace should remain caller-managed after interruption"
    );

    prepared
        .session
        .cleanup()
        .expect("caller-owned cleanup should be a no-op");
    assert!(
        workspace_root.exists(),
        "runner cleanup should not delete caller-owned workspace roots"
    );
    fs::remove_dir_all(workspace_root).expect("caller cleans up injected live session");
}

async fn sandbox_runstate_resume_restores_workspace_after_teardown_impl() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let provider = Arc::new(CapturingSandboxProvider {
        model: Arc::new(ApprovalResumeSandboxModel::default()),
    });
    let sandbox_agent = SandboxAgent::builder("sandbox")
        .instructions("Resume the same workspace after approval.")
        .build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(Manifest::default()),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };
    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox prep succeeds");
    prepared
        .session
        .write_file("/workspace/pre_resume.txt", "persisted before approval\n")
        .expect("pre-interruption file write should succeed");

    let approval_gated_agent = prepared.agent.clone_with(|agent| {
        let read_tool = agent
            .find_function_tool("sandbox_read_file", None)
            .expect("sandbox read tool is attached")
            .clone()
            .with_needs_approval(true);
        agent
            .function_tools
            .retain(|tool| tool.definition.name != "sandbox_read_file");
        agent.function_tools.push(read_tool);
    });

    let initial = Runner::new()
        .with_model_provider(provider.clone())
        .run(&approval_gated_agent, "resume after approval")
        .await
        .expect("initial sandbox run should interrupt cleanly");

    assert!(initial.final_output.is_none());
    assert!(matches!(
        initial
            .interruptions
            .first()
            .and_then(|step| step.kind.clone()),
        Some(RunInterruptionKind::ToolApproval)
    ));

    let mut state = initial
        .durable_state()
        .cloned()
        .expect("interrupted run should expose durable state");
    let serialized_state =
        serde_json::to_value(&state).expect("sandbox run state should serialize with resume data");
    let sandbox_payload = serialized_state
        .get("sandbox")
        .and_then(|value| value.get("session_state"))
        .cloned()
        .expect("run state should capture sandbox session state");
    assert_eq!(
        sandbox_payload
            .get("workspace_root_owned")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        sandbox_payload
            .get("workspace_root_ready")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    let snapshot_root = PathBuf::from(
        sandbox_payload
            .get("snapshot_root")
            .and_then(|value| value.as_str())
            .expect("run state should capture a durable snapshot root"),
    );
    assert!(
        snapshot_root.is_dir(),
        "runner-owned interrupted state should keep a durable snapshot outside the workspace"
    );
    let original_workspace_root = prepared.session.workspace_root();
    fs::remove_dir_all(&original_workspace_root)
        .expect("test should tear down original runner-owned workspace before resume");
    assert!(
        !original_workspace_root.exists(),
        "original runner-owned workspace should be removed before resume"
    );

    state.approve_for_tool(
        "call-1",
        Some("sandbox_read_file".to_owned()),
        Some("approved".to_owned()),
    );

    let resumed = Runner::new()
        .with_model_provider(provider)
        .resume(&state)
        .await
        .expect("resumed sandbox run should succeed");

    assert_eq!(
        resumed.final_output.as_deref(),
        Some("resumed:persisted before approval\n")
    );
    assert_eq!(
        resumed
            .durable_state()
            .and_then(|state| state.sandbox.as_ref())
            .map(|state| state.session_state.workspace_root.clone()),
        Some(original_workspace_root.clone())
    );
    assert_eq!(
        resumed
            .durable_state()
            .and_then(|state| state.sandbox.as_ref())
            .and_then(|state| state.session_state.snapshot_root.clone()),
        Some(snapshot_root)
    );
    assert!(
        original_workspace_root.exists(),
        "resume should recreate the runner-owned workspace root from durable state"
    );
}

fn explicit_session_state_roundtrip_resumes_same_workspace_after_teardown_impl() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let initial = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                manifest: Some(Manifest::default()),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("initial sandbox prep succeeds");
    initial
        .session
        .write_file("/workspace/roundtrip.txt", "session-state roundtrip\n")
        .expect("session-state file write should succeed");

    let serialized = initial
        .session
        .serialize_session_state()
        .expect("sandbox session state should serialize");
    let original_workspace_root = initial.session.workspace_root();
    let restored_state = LocalSandboxSession::deserialize_session_state(serialized)
        .expect("sandbox session state should deserialize");
    let snapshot_root = restored_state
        .snapshot_root
        .clone()
        .expect("serialized state should include a durable snapshot root");
    assert!(
        snapshot_root.is_dir(),
        "serialized session state should keep a durable snapshot outside the workspace"
    );
    fs::remove_dir_all(&original_workspace_root)
        .expect("test should tear down original runner-owned workspace before explicit resume");
    assert!(
        !original_workspace_root.exists(),
        "original runner-owned workspace should be gone before explicit resume"
    );
    let restored = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session_state: Some(restored_state),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("session-state resume should prepare");

    assert_eq!(restored.session.workspace_root(), original_workspace_root);
    assert_eq!(
        restored
            .session
            .read_file("/workspace/roundtrip.txt")
            .expect("resumed workspace should keep prior file"),
        "session-state roundtrip\n"
    );

    restored
        .session
        .write_file("/workspace/extended.txt", "still same workspace\n")
        .expect("resumed session should keep extending the same workspace");
    assert!(
        restored.session.workspace_root().exists(),
        "explicit session_state resume should recreate the workspace root after teardown"
    );
    assert_eq!(
        restored
            .session
            .read_file("/workspace/extended.txt")
            .expect("resumed session should read newly extended content"),
        "still same workspace\n"
    );
}

#[tokio::test]
async fn sandbox_runstate_resume_restores_workspace_after_teardown() {
    sandbox_runstate_resume_restores_workspace_after_teardown_impl().await;
}

#[test]
fn explicit_session_state_roundtrip_resumes_same_workspace_after_teardown() {
    explicit_session_state_roundtrip_resumes_same_workspace_after_teardown_impl();
}

#[tokio::test]
async fn sandbox_runstate_resume_restores_workspace_after_approval() {
    sandbox_runstate_resume_restores_workspace_after_teardown_impl().await;
}

#[test]
fn explicit_session_state_roundtrip_resumes_same_workspace() {
    explicit_session_state_roundtrip_resumes_same_workspace_after_teardown_impl();
}

#[test]
fn local_snapshot_restore_corrects_workspace_drift() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let initial = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                manifest: Some(Manifest::default()),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("initial sandbox prep succeeds");
    initial
        .session
        .write_file("/workspace/snapshot.txt", "persisted snapshot contents\n")
        .expect("snapshot file write should succeed");

    let serialized = initial
        .session
        .serialize_session_state()
        .expect("sandbox session state should serialize");
    assert_eq!(
        serialized
            .get("snapshot_fingerprint_version")
            .and_then(|value| value.as_str()),
        Some("workspace_sha256_v1")
    );
    assert!(
        serialized
            .get("snapshot_fingerprint")
            .and_then(|value| value.as_str())
            .is_some(),
        "serialized state should record a workspace fingerprint"
    );

    fs::write(
        initial.session.workspace_root().join("snapshot.txt"),
        "drifted workspace contents\n",
    )
    .expect("drifted file write should succeed");
    fs::write(
        initial.session.workspace_root().join("unexpected.txt"),
        "should be removed on restore\n",
    )
    .expect("unexpected file write should succeed");

    let restored_state = LocalSandboxSession::deserialize_session_state(serialized)
        .expect("sandbox session state should deserialize");
    let resumed = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session_state: Some(restored_state),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("session-state resume should prepare");

    assert_eq!(
        resumed
            .session
            .read_file("/workspace/snapshot.txt")
            .expect("restored workspace should keep persisted file"),
        "persisted snapshot contents\n"
    );
    assert!(
        !resumed
            .session
            .workspace_root()
            .join("unexpected.txt")
            .exists(),
        "restore should remove files introduced by drift"
    );
}

#[test]
fn sandbox_memory_persists_notes_across_resumed_sessions() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let initial = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                manifest: Some(Manifest::default()),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("initial sandbox prep succeeds");
    initial
        .session
        .write_memory_note("summary", "remember the sandbox session")
        .expect("memory note write should succeed");

    let serialized = initial
        .session
        .serialize_session_state()
        .expect("sandbox session state should serialize");
    let restored_state = LocalSandboxSession::deserialize_session_state(serialized)
        .expect("sandbox session state should deserialize");
    let resumed = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session_state: Some(restored_state),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("session-state resume should prepare");

    assert_eq!(
        resumed
            .session
            .read_memory_note("summary")
            .expect("memory note lookup should succeed")
            .as_deref(),
        Some("remember the sandbox session")
    );

    resumed
        .session
        .write_memory_note("summary", "updated remembered state")
        .expect("memory note update should succeed");
    assert_eq!(
        resumed
            .session
            .read_memory_note("summary")
            .expect("updated memory note lookup should succeed")
            .as_deref(),
        Some("updated remembered state")
    );

    let resumed_state = LocalSandboxSession::deserialize_session_state(
        resumed
            .session
            .serialize_session_state()
            .expect("updated sandbox session state should serialize"),
    )
    .expect("updated sandbox session state should deserialize");
    let resumed_again = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                session_state: Some(resumed_state),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("updated session-state resume should prepare");
    assert_eq!(
        resumed_again
            .session
            .read_memory_note("summary")
            .expect("updated memory note should persist across resume")
            .as_deref(),
        Some("updated remembered state")
    );

    let unrelated = prepare_sandbox_run(
        &sandbox_agent,
        &RunConfig {
            sandbox: Some(SandboxRunConfig {
                manifest: Some(Manifest::default()),
                ..SandboxRunConfig::default()
            }),
            ..RunConfig::default()
        },
    )
    .expect("fresh sandbox prep succeeds");
    assert_eq!(
        unrelated
            .session
            .read_memory_note("summary")
            .expect("fresh sandbox note lookup should succeed"),
        None
    );
}

#[test]
fn unix_local_pty_accepts_stdin_and_surfaces_output() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(Manifest::default()),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };
    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox prep succeeds");

    let pty = prepared
        .session
        .open_pty("pwd; read line; printf 'echo:%s\\n' \"$line\"")
        .expect("pty should start");
    let initial_output = pty
        .wait_for_output(
            &prepared.session.workspace_root().display().to_string(),
            Duration::from_secs(2),
        )
        .expect("pty should emit workspace path");
    assert!(initial_output.contains(&prepared.session.workspace_root().display().to_string()));

    pty.write_stdin("rust-pty\n")
        .expect("stdin write should succeed");
    let echoed = pty
        .wait_for_output("echo:rust-pty", Duration::from_secs(2))
        .expect("pty should surface echoed input");
    assert!(echoed.contains("echo:rust-pty"));

    let exit_code = pty.wait().expect("pty should exit cleanly");
    assert_eq!(exit_code, 0);

    prepared.session.cleanup().expect("cleanup succeeds");
}

#[test]
fn unix_local_pty_blocks_interpreter_and_expansion_escapes() {
    let _guard = localdir_hook_lock().lock().expect("sandbox test lock");
    let sandbox_agent = SandboxAgent::builder("sandbox").build();
    let host_outside = unique_temp_path("sandbox-pty-outside");
    if host_outside.exists() {
        fs::remove_file(&host_outside).expect("remove stale host outside file");
    }

    let run_config = RunConfig {
        sandbox: Some(SandboxRunConfig {
            manifest: Some(Manifest::default()),
            ..SandboxRunConfig::default()
        }),
        ..RunConfig::default()
    };
    let prepared = prepare_sandbox_run(&sandbox_agent, &run_config).expect("sandbox prep succeeds");

    let pty = prepared
        .session
        .open_pty(&format!(
            "pwd; perl -e \"open my \\$fh, '>', q[{}] or die \\$!; print {{\\$fh}} q[escape]; close \\$fh;\"; target='{}'; printf 'escape\\n' > \"$target\"",
            host_outside.display(),
            host_outside.display()
        ))
        .expect("pty should start");
    let output = pty
        .wait_for_output(
            &prepared.session.workspace_root().display().to_string(),
            Duration::from_secs(2),
        )
        .expect("pty should emit workspace path");
    assert!(output.contains(&prepared.session.workspace_root().display().to_string()));

    let exit_code = pty.wait().expect("pty should exit after blocked escapes");
    assert_ne!(exit_code, 0);
    assert!(
        !host_outside.exists(),
        "outside path should not be created through PTY escape attempts"
    );

    prepared.session.cleanup().expect("cleanup succeeds");
}
