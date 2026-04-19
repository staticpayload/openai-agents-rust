use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use openai_agents::sandbox::{Dir, File, LocalDir, Manifest, prepare_sandbox_run};
use openai_agents::{
    ApplyPatchOperation, Model, ModelProvider, ModelRequest, ModelResponse, OutputItem, RunConfig,
    RunContext, RunContextWrapper, Runner, SandboxAgent, SandboxRunConfig, Tool, ToolContext,
    ToolOutput, Usage,
};
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
