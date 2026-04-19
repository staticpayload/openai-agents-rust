use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use openai_agents::extensions::{
    BlaxelSandboxClient, BlaxelSandboxClientOptions, CloudflareSandboxClient,
    CloudflareSandboxClientOptions, DEFAULT_DAYTONA_WORKSPACE_ROOT, DEFAULT_VERCEL_WORKSPACE_ROOT,
    DaytonaSandboxClient, DaytonaSandboxClientOptions, E2BSandboxClient, E2BSandboxClientOptions,
    HostedMountEntry, HostedProviderMountPayload, RunloopSandboxClient,
    RunloopSandboxClientOptions, VercelSandboxClient, VercelSandboxClientOptions,
};
use serde_json::Value;

struct ProviderCase {
    feature: &'static str,
    facade_prefix: &'static str,
    extensions_prefix: &'static str,
}

#[test]
fn hosted_provider_feature_matrix_builds_and_exports_symbols() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");

    let cases = [
        ProviderCase {
            feature: "e2b",
            facade_prefix: "openai_agents::extensions",
            extensions_prefix: "agents_extensions",
        },
        ProviderCase {
            feature: "modal",
            facade_prefix: "openai_agents::extensions",
            extensions_prefix: "agents_extensions",
        },
        ProviderCase {
            feature: "daytona",
            facade_prefix: "openai_agents::extensions",
            extensions_prefix: "agents_extensions",
        },
        ProviderCase {
            feature: "blaxel",
            facade_prefix: "openai_agents::extensions",
            extensions_prefix: "agents_extensions",
        },
        ProviderCase {
            feature: "cloudflare",
            facade_prefix: "openai_agents::extensions",
            extensions_prefix: "agents_extensions",
        },
        ProviderCase {
            feature: "runloop",
            facade_prefix: "openai_agents::extensions",
            extensions_prefix: "agents_extensions",
        },
        ProviderCase {
            feature: "vercel",
            facade_prefix: "openai_agents::extensions",
            extensions_prefix: "agents_extensions",
        },
    ];

    for case in cases {
        let provider = provider_ident(case.feature);
        let crate_dir = create_temp_crate(&workspace_root, case.feature, &provider);
        let manifest_path = crate_dir.join("Cargo.toml");
        let target_dir = workspace_root
            .join("target")
            .join("hosted-provider-feature-matrix")
            .join(case.feature);
        let main_rs = crate_dir.join("src/main.rs");
        let program = sandbox_export_program(
            case.feature,
            &provider,
            case.facade_prefix,
            case.extensions_prefix,
        );

        fs::write(&main_rs, program).expect("main.rs should write");

        let output = Command::new(cargo_bin())
            .arg("check")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(&manifest_path)
            .env("CARGO_TARGET_DIR", &target_dir)
            .output()
            .expect("cargo check should run");

        assert!(
            output.status.success(),
            "feature `{}` failed to compile.\nstdout:\n{}\nstderr:\n{}",
            case.feature,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let _ = fs::remove_dir_all(&crate_dir);
    }
}

#[test]
fn hosted_provider_create_resume_and_auth_precedence() {
    let _env_guard = env_lock();

    unsafe {
        std::env::set_var("E2B_API_KEY", "env-e2b-key");
        std::env::remove_var("CLOUDFLARE_SANDBOX_API_KEY");
    }

    let e2b_client = E2BSandboxClient::new(E2BSandboxClientOptions {
        api_key: Some("explicit-e2b-key".to_owned()),
        ..Default::default()
    });
    let e2b_created = e2b_client.create().expect("e2b create should succeed");
    assert_eq!(e2b_created.resolved_auth_source(), "explicit");
    assert_eq!(e2b_created.resolved_auth_value(), "explicit-e2b-key");
    assert_eq!(e2b_created.state().workspace_root, "/workspace");

    let daytona_client = DaytonaSandboxClient::new(DaytonaSandboxClientOptions {
        api_key: Some("daytona-key".to_owned()),
        ..Default::default()
    });
    let daytona_created = daytona_client
        .create()
        .expect("daytona create should succeed");
    assert_eq!(
        daytona_created.state().workspace_root,
        DEFAULT_DAYTONA_WORKSPACE_ROOT
    );
    let daytona_resumed = daytona_client
        .resume(daytona_created.state().clone())
        .expect("daytona resume should succeed");
    assert_eq!(
        daytona_resumed.state().session_id,
        daytona_created.state().session_id
    );
    assert!(daytona_resumed.state().start_state_preserved);

    let vercel_client = VercelSandboxClient::new(VercelSandboxClientOptions {
        token: Some("vercel-token".to_owned()),
        workspace_root: Some("/tmp/custom-root".to_owned()),
        ..Default::default()
    });
    let vercel_created = vercel_client
        .create()
        .expect("vercel create should succeed");
    assert_eq!(vercel_created.state().workspace_root, "/tmp/custom-root");
    let vercel_resumed = vercel_client
        .resume(vercel_created.state().clone())
        .expect("vercel resume should succeed");
    assert_eq!(vercel_resumed.state().workspace_root, "/tmp/custom-root");

    let cloudflare_missing_auth = CloudflareSandboxClient::new(CloudflareSandboxClientOptions {
        workspace_root: Some("/workspace".to_owned()),
        ..Default::default()
    });
    let auth_error = cloudflare_missing_auth
        .create()
        .expect_err("missing cloudflare auth should fail");
    assert!(
        auth_error
            .to_string()
            .contains("CLOUDFLARE_SANDBOX_API_KEY"),
        "unexpected error: {auth_error}"
    );

    let cloudflare_bad_root = CloudflareSandboxClient::new(CloudflareSandboxClientOptions {
        api_key: Some("cloudflare-key".to_owned()),
        workspace_root: Some("/tmp/not-supported".to_owned()),
        ..Default::default()
    });
    let root_error = cloudflare_bad_root
        .create()
        .expect_err("cloudflare should reject a non-/workspace root");
    assert!(
        root_error.to_string().contains("/workspace"),
        "unexpected error: {root_error}"
    );

    unsafe {
        std::env::remove_var("E2B_API_KEY");
        std::env::remove_var("CLOUDFLARE_SANDBOX_API_KEY");
    }
}

#[test]
fn hosted_provider_state_is_secret_safe() {
    let _env_guard = env_lock();

    let client = BlaxelSandboxClient::new(BlaxelSandboxClientOptions {
        token: Some("bl-secret-token".to_owned()),
        client_timeout_s: Some(45),
        workspace_root: Some("/workspace/project".to_owned()),
        base_url: Some("https://sandbox.example.test".to_owned()),
        exposed_ports: vec![3000],
        interactive_pty: true,
        ..Default::default()
    });
    let session = client.create().expect("blaxel create should succeed");
    let payload = client
        .serialize_session_state(session.state())
        .expect("state should serialize");

    let object = payload
        .as_object()
        .expect("serialized state should be an object");
    assert!(!object.contains_key("token"));
    assert!(!object.contains_key("api_key"));
    assert!(!object.contains_key("client_timeout_s"));
    assert_eq!(
        object.get("workspace_root").and_then(Value::as_str),
        Some("/workspace/project")
    );
    assert_eq!(
        object
            .get("exposed_ports")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        object.get("interactive_pty").and_then(Value::as_bool),
        Some(true)
    );

    let restored = client
        .deserialize_session_state(payload)
        .expect("state should deserialize");
    assert_eq!(restored.workspace_root, "/workspace/project");
    assert_eq!(restored.exposed_ports, vec![3000]);
    assert!(restored.interactive_pty);
}

#[test]
fn hosted_provider_capabilities_preserve_ports_and_pty_flags() {
    let _env_guard = env_lock();

    let e2b_client = E2BSandboxClient::new(E2BSandboxClientOptions {
        api_key: Some("e2b-key".to_owned()),
        exposed_ports: vec![3000, 4000],
        interactive_pty: true,
        ..Default::default()
    });
    let e2b_created = e2b_client.create().expect("e2b create should succeed");
    assert_eq!(e2b_created.state().exposed_ports, vec![3000, 4000]);
    assert!(e2b_created.state().interactive_pty);
    assert!(e2b_created.supports_pty());
    let e2b_resumed = e2b_client
        .resume(e2b_created.state().clone())
        .expect("e2b resume should succeed");
    assert_eq!(e2b_resumed.state().exposed_ports, vec![3000, 4000]);
    assert!(e2b_resumed.state().interactive_pty);
    assert!(e2b_resumed.state().start_state_preserved);

    let runloop_client = RunloopSandboxClient::new(RunloopSandboxClientOptions {
        api_key: Some("runloop-key".to_owned()),
        interactive_pty: true,
        ..Default::default()
    });
    let runloop_error = runloop_client
        .create()
        .expect_err("runloop should reject PTY requests");
    assert!(
        runloop_error.to_string().contains("interactive PTY"),
        "unexpected error: {runloop_error}"
    );

    let vercel_client = VercelSandboxClient::new(VercelSandboxClientOptions {
        token: Some("vercel-token".to_owned()),
        exposed_ports: vec![8080],
        ..Default::default()
    });
    let vercel_created = vercel_client
        .create()
        .expect("vercel create should preserve exposed ports");
    assert_eq!(
        vercel_created.state().workspace_root,
        DEFAULT_VERCEL_WORKSPACE_ROOT
    );
    assert_eq!(vercel_created.state().exposed_ports, vec![8080]);
    assert!(!vercel_created.supports_pty());
}

#[test]
fn hosted_provider_mount_strategies_match_upstream_payloads() {
    let e2b_mount: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "s3_mount",
        "bucket": "code-bucket",
        "access_key_id": "ak",
        "secret_access_key": "sk",
        "session_token": "session",
        "prefix": "repo/src",
        "region": "us-west-2",
        "mount_path": "/workspace/data",
        "mount_strategy": { "type": "e2b_cloud_bucket" }
    }))
    .expect("e2b mount should parse");
    let e2b_payload = serde_json::to_value(
        e2b_mount
            .resolve_provider_payload()
            .expect("e2b payload should resolve"),
    )
    .expect("e2b payload should serialize");
    assert_eq!(
        e2b_payload,
        serde_json::json!({
            "provider": "e2b",
            "config": {
                "provider": "s3",
                "strategy": "e2b_cloud_bucket",
                "bucket": "code-bucket",
                "remote_path": "code-bucket/repo/src",
                "mount_path": "/workspace/data",
                "endpoint_url": null,
                "region": "us-west-2",
                "credentials": {
                    "access_key_id": "ak",
                    "secret_access_key": "sk"
                },
                "session_token": "session",
                "service_account_credentials": null,
                "access_token": null,
                "read_only": true
            }
        })
    );

    let modal_mount: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "gcs_mount",
        "bucket": "analytics",
        "access_id": "gcs-access",
        "secret_access_key": "gcs-secret",
        "prefix": "exports/2026",
        "read_only": false,
        "mount_strategy": { "type": "modal_cloud_bucket" }
    }))
    .expect("modal mount should parse");
    let modal_payload = serde_json::to_value(
        modal_mount
            .resolve_provider_payload()
            .expect("modal payload should resolve"),
    )
    .expect("modal payload should serialize");
    assert_eq!(
        modal_payload,
        serde_json::json!({
            "provider": "modal",
            "config": {
                "bucket_name": "analytics",
                "bucket_endpoint_url": "https://storage.googleapis.com",
                "key_prefix": "exports/2026",
                "credentials": {
                    "GOOGLE_ACCESS_KEY_ID": "gcs-access",
                    "GOOGLE_ACCESS_KEY_SECRET": "gcs-secret"
                },
                "secret_name": null,
                "secret_environment_name": null,
                "read_only": false
            }
        })
    );

    let cloudflare_mount: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "r2_mount",
        "bucket": "static-assets",
        "account_id": "acct-123",
        "access_key_id": "r2-ak",
        "secret_access_key": "r2-sk",
        "mount_strategy": { "type": "cloudflare_bucket_mount" }
    }))
    .expect("cloudflare mount should parse");
    let cloudflare_payload = serde_json::to_value(
        cloudflare_mount
            .resolve_provider_payload()
            .expect("cloudflare payload should resolve"),
    )
    .expect("cloudflare payload should serialize");
    assert_eq!(
        cloudflare_payload,
        serde_json::json!({
            "provider": "cloudflare",
            "config": {
                "bucket_name": "static-assets",
                "bucket_endpoint_url": "https://acct-123.r2.cloudflarestorage.com",
                "provider": "r2",
                "key_prefix": null,
                "credentials": {
                    "access_key_id": "r2-ak",
                    "secret_access_key": "r2-sk"
                },
                "read_only": true
            }
        })
    );

    let blaxel_mount: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "gcs_mount",
        "bucket": "docs",
        "service_account_credentials": "{\"client_email\":\"bot@example.com\"}",
        "mount_path": "/workspace/docs",
        "mount_strategy": { "type": "blaxel_cloud_bucket" }
    }))
    .expect("blaxel mount should parse");
    let blaxel_payload = serde_json::to_value(
        blaxel_mount
            .resolve_provider_payload()
            .expect("blaxel payload should resolve"),
    )
    .expect("blaxel payload should serialize");
    assert_eq!(
        blaxel_payload,
        serde_json::json!({
            "provider": "blaxel",
            "config": {
                "provider": "gcs",
                "bucket": "docs",
                "mount_path": "/workspace/docs",
                "read_only": true,
                "access_key_id": null,
                "secret_access_key": null,
                "session_token": null,
                "region": null,
                "endpoint_url": null,
                "prefix": null,
                "service_account_key": "{\"client_email\":\"bot@example.com\"}"
            }
        })
    );

    let daytona_mount: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "gcs_mount",
        "bucket": "analytics",
        "access_token": "ya29.token",
        "prefix": "daily",
        "mount_strategy": { "type": "daytona_cloud_bucket" }
    }))
    .expect("daytona mount should parse");
    let runloop_mount: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "s3_mount",
        "bucket": "logs",
        "endpoint_url": "https://s3.example.test",
        "mount_strategy": { "type": "runloop_cloud_bucket" }
    }))
    .expect("runloop mount should parse");
    let daytona_payload = daytona_mount
        .resolve_provider_payload()
        .expect("daytona payload should resolve");
    let runloop_payload = runloop_mount
        .resolve_provider_payload()
        .expect("runloop payload should resolve");
    assert!(matches!(
        daytona_payload,
        HostedProviderMountPayload::Daytona { .. }
    ));
    assert!(matches!(
        runloop_payload,
        HostedProviderMountPayload::Runloop { .. }
    ));

    let invalid_modal: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "s3_mount",
        "bucket": "bucket",
        "access_key_id": "ak",
        "secret_access_key": "sk",
        "mount_strategy": {
            "type": "modal_cloud_bucket",
            "secret_name": "named-secret"
        }
    }))
    .expect("invalid modal mount should still parse");
    let invalid_modal_error = invalid_modal
        .resolve_provider_payload()
        .expect_err("modal should reject mixed inline credentials and secret_name");
    assert!(
        invalid_modal_error
            .to_string()
            .contains("do not support both inline credentials and secret_name")
    );

    let invalid_cloudflare: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "s3_mount",
        "bucket": "bucket",
        "access_key_id": "ak",
        "secret_access_key": "sk",
        "session_token": "session",
        "mount_strategy": { "type": "cloudflare_bucket_mount" }
    }))
    .expect("invalid cloudflare mount should still parse");
    let invalid_cloudflare_error = invalid_cloudflare
        .resolve_provider_payload()
        .expect_err("cloudflare should reject s3 session tokens");
    assert!(
        invalid_cloudflare_error
            .to_string()
            .contains("do not support s3 session_token credentials")
    );

    let invalid_r2: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "r2_mount",
        "bucket": "bucket",
        "account_id": "acct",
        "access_key_id": "only-access",
        "mount_strategy": { "type": "runloop_cloud_bucket" }
    }))
    .expect("invalid r2 mount should still parse");
    let invalid_r2_error = invalid_r2
        .resolve_provider_payload()
        .expect_err("r2 should reject incomplete credential pairs");
    assert!(
        invalid_r2_error
            .to_string()
            .contains("require both access_key_id and secret_access_key")
    );

    let invalid_gcs: HostedMountEntry = serde_json::from_value(serde_json::json!({
        "type": "gcs_mount",
        "bucket": "bucket",
        "service_account_credentials": "{\"client_email\":\"bot@example.com\"}",
        "mount_strategy": { "type": "cloudflare_bucket_mount" }
    }))
    .expect("invalid gcs mount should still parse");
    let invalid_gcs_error = invalid_gcs
        .resolve_provider_payload()
        .expect_err("cloudflare should reject native gcs auth");
    assert!(
        invalid_gcs_error
            .to_string()
            .contains("gcs cloudflare bucket mounts require access_id and secret_access_key")
    );
}

fn create_temp_crate(workspace_root: &Path, feature: &str, provider: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should move forward")
        .as_nanos();
    let crate_dir = workspace_root
        .join("target")
        .join("tmp")
        .join(format!("hosted-provider-{feature}-{unique}"));

    fs::create_dir_all(crate_dir.join("src")).expect("temp crate src dir should exist");

    let cargo_toml = format!(
        r#"[package]
name = "hosted-provider-{feature}"
version = "0.0.0"
edition = "2024"

[workspace]

[dependencies]
openai_agents = {{ package = "openai-agents-rs", path = "{openai_agents}", default-features = false, features = ["{feature}"] }}
agents_extensions = {{ package = "openai-agents-extensions-rs", path = "{agents_extensions}", default-features = false, features = ["{feature}"] }}
"#,
        feature = feature,
        openai_agents = display_path(&workspace_root.join("crates/openai-agents")),
        agents_extensions = display_path(&workspace_root.join("crates/agents-extensions")),
    );

    fs::write(crate_dir.join("Cargo.toml"), cargo_toml).expect("Cargo.toml should write");
    fs::write(crate_dir.join("src/main.rs"), format!("// {provider}\n"))
        .expect("placeholder main.rs should write");

    crate_dir
}

fn sandbox_export_program(
    feature: &str,
    provider: &str,
    facade_prefix: &str,
    extensions_prefix: &str,
) -> String {
    let client = format!("{provider}SandboxClient");
    let options = format!("{provider}SandboxClientOptions");
    let session = format!("{provider}SandboxSession");
    let state = format!("{provider}SandboxSessionState");

    format!(
        r#"use {facade_prefix}::sandbox::{{{client}, {options}, {session}, {state}}};
use {facade_prefix}::{{{client} as FacadeRootClient, {options} as FacadeRootOptions, {session} as FacadeRootSession, {state} as FacadeRootState}};
use {extensions_prefix}::sandbox::{{{client} as ExtensionsClient, {options} as ExtensionsOptions, {session} as ExtensionsSession, {state} as ExtensionsState}};
use {extensions_prefix}::{{{client} as ExtensionsRootClient, {options} as ExtensionsRootOptions, {session} as ExtensionsRootSession, {state} as ExtensionsRootState}};

fn main() {{
    let options = {options}::default();
    let state = {state}::default();
    let client = {client}::new(options.clone());
    let session = {session}::new(state.clone());

    let _facade_root_client: FacadeRootClient = client.clone();
    let _facade_root_options: FacadeRootOptions = options.clone();
    let _facade_root_session: FacadeRootSession = session.clone();
    let _facade_root_state: FacadeRootState = state.clone();

    let _extensions_client: ExtensionsClient = client.clone();
    let _extensions_options: ExtensionsOptions = options.clone();
    let _extensions_session: ExtensionsSession = session.clone();
    let _extensions_state: ExtensionsState = state.clone();

    let _extensions_root_client: ExtensionsRootClient = client;
    let _extensions_root_options: ExtensionsRootOptions = options;
    let _extensions_root_session: ExtensionsRootSession = session;
    let _extensions_root_state: ExtensionsRootState = state;

    let _ = "{feature}";
}}
"#,
        facade_prefix = facade_prefix,
        extensions_prefix = extensions_prefix,
        client = client,
        options = options,
        session = session,
        state = state,
        feature = feature,
    )
}

fn provider_ident(feature: &str) -> String {
    match feature {
        "e2b" => "E2B".to_owned(),
        other => {
            let mut chars = other.chars();
            let first = chars.next().expect("feature should not be empty");
            format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
        }
    }
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock should not be poisoned")
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}
