use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::agent::{Agent, AgentBuilder};
use crate::editor::{ApplyPatchOperation, ApplyPatchResult};
use crate::errors::{AgentsError, Result};
use crate::tool::{FunctionTool, function_tool};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxConcurrencyLimits {
    pub manifest_entries: Option<usize>,
    pub local_dir_files: Option<usize>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SandboxRunConfig {
    pub manifest: Option<Manifest>,
    pub concurrency_limits: SandboxConcurrencyLimits,
    pub session_state: Option<LocalSandboxSessionState>,
    #[serde(skip, default)]
    pub session: Option<LocalSandboxSession>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxCapability {
    Filesystem,
    Shell,
    ApplyPatch,
}

impl SandboxCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::Shell => "shell",
            Self::ApplyPatch => "apply_patch",
        }
    }

    pub fn defaults() -> Vec<Self> {
        vec![Self::Filesystem, Self::Shell, Self::ApplyPatch]
    }

    fn dedupe(capabilities: Vec<Self>) -> Vec<Self> {
        let mut normalized = Vec::new();
        for capability in capabilities {
            if !normalized.contains(&capability) {
                normalized.push(capability);
            }
        }
        normalized
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SandboxAgent {
    agent: Agent,
    pub default_manifest: Option<Manifest>,
    pub base_instructions: Option<String>,
    pub capabilities: Vec<SandboxCapability>,
}

impl SandboxAgent {
    pub fn builder(name: impl Into<String>) -> SandboxAgentBuilder {
        SandboxAgentBuilder::new(name)
    }

    pub fn into_agent(self) -> Agent {
        self.agent
    }
}

impl std::ops::Deref for SandboxAgent {
    type Target = Agent;

    fn deref(&self) -> &Self::Target {
        &self.agent
    }
}

impl std::ops::DerefMut for SandboxAgent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.agent
    }
}

#[derive(Clone, Debug)]
pub struct SandboxAgentBuilder {
    agent_builder: AgentBuilder,
    default_manifest: Option<Manifest>,
    base_instructions: Option<String>,
    capabilities: Option<Vec<SandboxCapability>>,
}

impl SandboxAgentBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            agent_builder: Agent::builder(name),
            default_manifest: None,
            base_instructions: None,
            capabilities: None,
        }
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.agent_builder = self.agent_builder.instructions(instructions);
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.agent_builder = self.agent_builder.model(model);
        self
    }

    pub fn default_manifest(mut self, manifest: Manifest) -> Self {
        self.default_manifest = Some(manifest);
        self
    }

    pub fn base_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.base_instructions = Some(instructions.into());
        self
    }

    pub fn capabilities(mut self, capabilities: Vec<SandboxCapability>) -> Self {
        self.capabilities = Some(SandboxCapability::dedupe(capabilities));
        self
    }

    pub fn build(self) -> SandboxAgent {
        let capabilities = self
            .capabilities
            .map(SandboxCapability::dedupe)
            .unwrap_or_else(SandboxCapability::defaults);
        SandboxAgent {
            agent: self.agent_builder.build(),
            default_manifest: self.default_manifest,
            base_instructions: self.base_instructions,
            capabilities,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub root: String,
    pub entries: BTreeMap<String, ManifestEntry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            root: "/workspace".to_owned(),
            entries: BTreeMap::new(),
        }
    }
}

impl Manifest {
    pub fn with_entry(mut self, path: impl Into<String>, entry: impl Into<ManifestEntry>) -> Self {
        self.entries.insert(path.into(), entry.into());
        self
    }

    pub fn describe(&self) -> String {
        let mut lines = vec![format!("{} (workspace root)", self.root)];
        for (path, entry) in &self.entries {
            describe_entry(path, entry, 0, &mut lines);
        }
        lines.join("\n")
    }
}

fn describe_entry(path: &str, entry: &ManifestEntry, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    match entry {
        ManifestEntry::File(_) => lines.push(format!("{indent}- {path}")),
        ManifestEntry::LocalDir(_) => lines.push(format!("{indent}- {path}/ (copied from host)")),
        ManifestEntry::Dir(dir) => {
            lines.push(format!("{indent}- {path}/"));
            for (child, child_entry) in &dir.entries {
                let child_path = format!("{path}/{child}");
                describe_entry(&child_path, child_entry, depth + 1, lines);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManifestEntry {
    File(File),
    Dir(Dir),
    LocalDir(LocalDir),
}

impl From<File> for ManifestEntry {
    fn from(value: File) -> Self {
        Self::File(value)
    }
}

impl From<Dir> for ManifestEntry {
    fn from(value: Dir) -> Self {
        Self::Dir(value)
    }
}

impl From<LocalDir> for ManifestEntry {
    fn from(value: LocalDir) -> Self {
        Self::LocalDir(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub content: Vec<u8>,
}

impl File {
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            content: text.into().into_bytes(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dir {
    pub entries: BTreeMap<String, ManifestEntry>,
}

impl Dir {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entry(mut self, path: impl Into<String>, entry: impl Into<ManifestEntry>) -> Self {
        self.entries.insert(path.into(), entry.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalDir {
    pub src: PathBuf,
}

impl LocalDir {
    pub fn new(src: impl AsRef<Path>) -> Self {
        Self {
            src: src.as_ref().to_path_buf(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreparedSandboxRun {
    pub agent: Agent,
    pub session: LocalSandboxSession,
}

#[derive(Clone, Debug)]
pub struct AgentSandboxRuntime {
    pub(crate) base_instructions: Option<String>,
    pub(crate) user_instructions: Option<String>,
    pub(crate) capabilities: Vec<SandboxCapability>,
    pub(crate) session: LocalSandboxSession,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSandboxSessionState {
    pub manifest: Manifest,
    pub workspace_root: PathBuf,
    pub workspace_root_owned: bool,
    pub workspace_root_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_fingerprint_version: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub memory_notes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxRunState {
    pub base_instructions: Option<String>,
    pub user_instructions: Option<String>,
    pub capabilities: Vec<SandboxCapability>,
    pub session_state: LocalSandboxSessionState,
}

#[derive(Clone, Debug)]
pub struct LocalSandboxSession {
    inner: Arc<LocalSandboxSessionInner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalShellOutput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl LocalShellOutput {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

#[derive(Clone, Debug)]
pub struct LocalSandboxPtySession {
    inner: Arc<LocalSandboxPtySessionInner>,
}

#[derive(Debug)]
struct LocalSandboxSessionInner {
    workspace_root: PathBuf,
    manifest: Mutex<Manifest>,
    persisted: Mutex<LocalSandboxPersistedState>,
    runner_owned: bool,
    cleaned: Mutex<bool>,
}

#[derive(Clone, Debug, Default)]
struct LocalSandboxPersistedState {
    snapshot_root: Option<PathBuf>,
    snapshot_fingerprint: Option<String>,
    snapshot_fingerprint_version: Option<String>,
    memory_notes: BTreeMap<String, String>,
}

#[derive(Debug)]
struct LocalSandboxPtySessionInner {
    child: Mutex<Child>,
    output: Arc<Mutex<Vec<u8>>>,
    reader: Mutex<Option<thread::JoinHandle<()>>>,
}

impl LocalSandboxSession {
    pub fn create_caller_owned(manifest: Manifest) -> Result<Self> {
        let workspace_root = create_temp_workspace_root()?;
        if let Err(error) = materialize_manifest(&manifest, &workspace_root) {
            let _ = fs::remove_dir_all(&workspace_root);
            return Err(error);
        }

        let session = Self {
            inner: Arc::new(LocalSandboxSessionInner {
                workspace_root,
                manifest: Mutex::new(manifest),
                persisted: Mutex::new(LocalSandboxPersistedState::default()),
                runner_owned: false,
                cleaned: Mutex::new(false),
            }),
        };
        session.refresh_snapshot_state()?;
        Ok(session)
    }

    pub fn workspace_root(&self) -> PathBuf {
        self.inner.workspace_root.clone()
    }

    pub fn logical_root(&self) -> String {
        self.manifest().root
    }

    pub fn manifest(&self) -> Manifest {
        self.inner
            .manifest
            .lock()
            .expect("sandbox manifest lock")
            .clone()
    }

    pub fn runner_owned(&self) -> bool {
        self.inner.runner_owned
    }

    pub fn session_state(&self) -> LocalSandboxSessionState {
        let persisted = self
            .inner
            .persisted
            .lock()
            .expect("sandbox persisted lock")
            .clone();
        LocalSandboxSessionState {
            manifest: self.manifest(),
            workspace_root: self.workspace_root(),
            workspace_root_owned: self.runner_owned(),
            workspace_root_ready: self.inner.workspace_root.is_dir(),
            snapshot_root: persisted.snapshot_root,
            snapshot_fingerprint: persisted.snapshot_fingerprint,
            snapshot_fingerprint_version: persisted.snapshot_fingerprint_version,
            memory_notes: persisted.memory_notes,
        }
    }

    pub fn serialize_session_state(&self) -> Result<Value> {
        self.refresh_snapshot_state()?;
        serde_json::to_value(self.session_state())
            .map_err(|error| AgentsError::message(error.to_string()))
    }

    pub fn deserialize_session_state(payload: Value) -> Result<LocalSandboxSessionState> {
        serde_json::from_value(payload).map_err(|error| AgentsError::message(error.to_string()))
    }

    pub fn resume(state: LocalSandboxSessionState) -> Result<Self> {
        if state.workspace_root_ready {
            if !state.workspace_root.is_dir() {
                if state.workspace_root_owned {
                    fs::create_dir_all(&state.workspace_root)
                        .map_err(|error| AgentsError::message(error.to_string()))?;
                } else {
                    return Err(AgentsError::message(format!(
                        "sandbox workspace `{}` is not available for resume",
                        state.workspace_root.display()
                    )));
                }
            }
        } else {
            fs::create_dir_all(&state.workspace_root)
                .map_err(|error| AgentsError::message(error.to_string()))?;
            if let Err(error) = materialize_manifest(&state.manifest, &state.workspace_root) {
                if state.workspace_root_owned {
                    let _ = fs::remove_dir_all(&state.workspace_root);
                }
                return Err(error);
            }
        }

        let session = Self {
            inner: Arc::new(LocalSandboxSessionInner {
                workspace_root: state.workspace_root,
                manifest: Mutex::new(state.manifest),
                persisted: Mutex::new(LocalSandboxPersistedState {
                    snapshot_root: state.snapshot_root,
                    snapshot_fingerprint: state.snapshot_fingerprint,
                    snapshot_fingerprint_version: state.snapshot_fingerprint_version,
                    memory_notes: state.memory_notes,
                }),
                runner_owned: state.workspace_root_owned,
                cleaned: Mutex::new(false),
            }),
        };

        session.restore_snapshot_if_needed()?;
        Ok(session)
    }

    pub fn cleanup(&self) -> Result<()> {
        let mut cleaned = self.inner.cleaned.lock().expect("sandbox cleanup lock");
        if *cleaned {
            return Ok(());
        }
        if let Some(snapshot_root) = self
            .inner
            .persisted
            .lock()
            .expect("sandbox persisted lock")
            .snapshot_root
            .clone()
        {
            if snapshot_root.exists() {
                fs::remove_dir_all(snapshot_root)
                    .map_err(|error| AgentsError::message(error.to_string()))?;
            }
        }
        if self.inner.runner_owned && self.inner.workspace_root.exists() {
            fs::remove_dir_all(&self.inner.workspace_root)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        }
        *cleaned = true;
        Ok(())
    }

    pub fn resolve_path(&self, requested: &str) -> Result<PathBuf> {
        self.resolve_path_for_access(requested)
    }

    fn resolve_path_for_access(&self, requested: &str) -> Result<PathBuf> {
        if requested.trim().is_empty() {
            return Err(AgentsError::message(
                "path must stay within the sandbox workspace",
            ));
        }

        let requested_path = Path::new(requested);
        let logical_root = self.manifest().root;
        let relative = if requested_path.is_absolute() {
            let logical_root = Path::new(&logical_root);
            requested_path
                .strip_prefix(logical_root)
                .map_err(|_| AgentsError::message("path must stay within the sandbox workspace"))?
                .to_path_buf()
        } else {
            requested_path.to_path_buf()
        };

        let mut normalized = PathBuf::new();
        for component in relative.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => normalized.push(part),
                Component::RootDir => {}
                Component::ParentDir => {
                    return Err(AgentsError::message(
                        "path must stay within the sandbox workspace",
                    ));
                }
                Component::Prefix(_) => {
                    return Err(AgentsError::message(
                        "path must stay within the sandbox workspace",
                    ));
                }
            }
        }

        let candidate = self.inner.workspace_root.join(normalized);
        ensure_path_stays_within_workspace(&self.inner.workspace_root, &candidate)?;
        Ok(candidate)
    }

    pub fn list_files(&self, requested: &str) -> Result<String> {
        let directory = self.resolve_path_for_access(requested)?;
        let entries =
            fs::read_dir(&directory).map_err(|error| AgentsError::message(error.to_string()))?;
        let mut names = entries
            .map(|entry| {
                entry
                    .map(|value| value.file_name().to_string_lossy().to_string())
                    .map_err(|error| AgentsError::message(error.to_string()))
            })
            .collect::<Result<Vec<_>>>()?;
        names.sort();
        Ok(names.join("\n"))
    }

    pub fn read_file(&self, requested: &str) -> Result<String> {
        let path = self.resolve_path_for_access(requested)?;
        fs::read_to_string(path).map_err(|error| AgentsError::message(error.to_string()))
    }

    pub fn write_file(&self, requested: &str, content: impl AsRef<[u8]>) -> Result<()> {
        let path = self.resolve_path_for_access(requested)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| AgentsError::message(error.to_string()))?;
        }
        fs::write(path, content).map_err(|error| AgentsError::message(error.to_string()))?;
        self.refresh_snapshot_state()
    }

    pub fn apply_patch(&self, operation: ApplyPatchOperation) -> Result<ApplyPatchResult> {
        self.write_file(&operation.path, operation.replacement.as_bytes())?;
        Ok(ApplyPatchResult {
            updated: true,
            path: operation.path,
        })
    }

    pub fn run_shell(&self, command: &str) -> Result<LocalShellOutput> {
        validate_shell_command(&self.logical_root(), command)?;

        let env_vars = sandbox_command_env(&self.inner.workspace_root);
        let shell_command = workspace_shell_command(&self.inner.workspace_root, command);
        let output = confined_shell_command(&shell_command, &self.inner.workspace_root, &env_vars)?
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| AgentsError::message(error.to_string()))?;

        let shell_output = LocalShellOutput {
            command: command.to_owned(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or_default(),
        };
        if shell_output.success() {
            self.refresh_snapshot_state()?;
        }
        Ok(shell_output)
    }

    pub fn open_pty(&self, command: &str) -> Result<LocalSandboxPtySession> {
        validate_shell_command(&self.logical_root(), command)?;

        let env_vars = sandbox_command_env(&self.inner.workspace_root);
        let shell_command = workspace_shell_command(&self.inner.workspace_root, command);
        let mut child =
            confined_pty_command(&shell_command, &self.inner.workspace_root, &env_vars)?
                .current_dir("/")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| AgentsError::message(error.to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentsError::message("failed to capture PTY stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentsError::message("failed to capture PTY stderr"))?;
        let output = Arc::new(Mutex::new(Vec::new()));
        let output_for_stdout = output.clone();
        let output_for_stderr = output.clone();
        let reader = thread::spawn(move || {
            read_process_output(stdout, output_for_stdout);
            read_process_output(stderr, output_for_stderr);
        });

        Ok(LocalSandboxPtySession {
            inner: Arc::new(LocalSandboxPtySessionInner {
                child: Mutex::new(child),
                output,
                reader: Mutex::new(Some(reader)),
            }),
        })
    }

    fn apply_live_manifest_update(&self, processed_manifest: Manifest) -> Result<()> {
        let current_manifest = self.manifest();
        if processed_manifest == current_manifest {
            return Ok(());
        }

        validate_running_live_session_manifest_update(&current_manifest, &processed_manifest)?;
        let entries_to_apply = diff_live_session_entries(
            &current_manifest.entries,
            &processed_manifest.entries,
            Path::new(""),
        )?;

        for (rel_path, entry) in &entries_to_apply {
            materialize_entry(entry, &self.inner.workspace_root, rel_path)?;
        }

        *self.inner.manifest.lock().expect("sandbox manifest lock") = processed_manifest;
        self.refresh_snapshot_state()?;
        Ok(())
    }

    pub fn write_memory_note(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<()> {
        self.inner
            .persisted
            .lock()
            .expect("sandbox persisted lock")
            .memory_notes
            .insert(key.into(), value.into());
        Ok(())
    }

    pub fn read_memory_note(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .inner
            .persisted
            .lock()
            .expect("sandbox persisted lock")
            .memory_notes
            .get(key)
            .cloned())
    }

    fn refresh_snapshot_state(&self) -> Result<()> {
        let (fingerprint, snapshot_root) = snapshot_workspace(&self.inner.workspace_root, self)?;
        let mut persisted = self.inner.persisted.lock().expect("sandbox persisted lock");
        persisted.snapshot_root = Some(snapshot_root);
        persisted.snapshot_fingerprint = Some(fingerprint);
        persisted.snapshot_fingerprint_version = Some(WORKSPACE_FINGERPRINT_VERSION.to_owned());
        Ok(())
    }

    fn restore_snapshot_if_needed(&self) -> Result<()> {
        let persisted = self
            .inner
            .persisted
            .lock()
            .expect("sandbox persisted lock")
            .clone();
        let Some(snapshot_root) = persisted.snapshot_root else {
            return Ok(());
        };
        let Some(snapshot_fingerprint) = persisted.snapshot_fingerprint else {
            return Ok(());
        };
        if !self.inner.workspace_root.exists() {
            fs::create_dir_all(&self.inner.workspace_root)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        }
        let live_fingerprint = fingerprint_workspace(&self.inner.workspace_root)?;
        if live_fingerprint == snapshot_fingerprint {
            return Ok(());
        }

        clear_directory(&self.inner.workspace_root)?;
        copy_directory_contents(&snapshot_root, &self.inner.workspace_root)?;
        Ok(())
    }
}

impl LocalSandboxPtySession {
    pub fn write_stdin(&self, input: impl AsRef<[u8]>) -> Result<()> {
        let mut child = self.inner.child.lock().expect("sandbox pty child lock");
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| AgentsError::message("PTY stdin is closed"))?;
        stdin
            .write_all(input.as_ref())
            .and_then(|_| stdin.flush())
            .map_err(|error| AgentsError::message(error.to_string()))
    }

    pub fn read_output(&self) -> String {
        let output = self.inner.output.lock().expect("sandbox pty output lock");
        String::from_utf8_lossy(&output).into_owned()
    }

    pub fn wait_for_output(&self, needle: &str, timeout: Duration) -> Result<String> {
        let deadline = Instant::now() + timeout;
        loop {
            let output = self.read_output();
            if output.contains(needle) {
                return Ok(output);
            }
            if Instant::now() >= deadline {
                return Err(AgentsError::message(format!(
                    "timed out waiting for PTY output containing `{needle}`"
                )));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn wait(&self) -> Result<i32> {
        let status = {
            let mut child = self.inner.child.lock().expect("sandbox pty child lock");
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.flush();
            }
            child
                .wait()
                .map_err(|error| AgentsError::message(error.to_string()))?
        };

        if let Some(reader) = self
            .inner
            .reader
            .lock()
            .expect("sandbox pty reader lock")
            .take()
        {
            let _ = reader.join();
        }

        Ok(status.code().unwrap_or_default())
    }
}

impl Drop for LocalSandboxSessionInner {
    fn drop(&mut self) {
        if self.runner_owned {
            let _ = fs::remove_dir_all(&self.workspace_root);
        }
    }
}

pub fn prepare_sandbox_run(
    agent: &SandboxAgent,
    run_config: &crate::run_config::RunConfig,
) -> Result<PreparedSandboxRun> {
    let sandbox_config = run_config.sandbox.clone().unwrap_or_default();
    let session = if let Some(session) = sandbox_config.session {
        if let Some(manifest) = sandbox_config.manifest {
            session.apply_live_manifest_update(manifest)?;
        }
        session
    } else if let Some(session_state) = sandbox_config.session_state {
        LocalSandboxSession::resume(session_state)?
    } else {
        let manifest = sandbox_config
            .manifest
            .or_else(|| agent.default_manifest.clone())
            .unwrap_or_default();
        let workspace_root = create_temp_workspace_root()?;
        if let Err(error) = materialize_manifest(&manifest, &workspace_root) {
            let _ = fs::remove_dir_all(&workspace_root);
            return Err(error);
        }
        LocalSandboxSession {
            inner: Arc::new(LocalSandboxSessionInner {
                workspace_root,
                manifest: Mutex::new(manifest),
                persisted: Mutex::new(LocalSandboxPersistedState::default()),
                runner_owned: true,
                cleaned: Mutex::new(false),
            }),
        }
    };
    let manifest = session.manifest();
    let instructions = build_instructions(agent, &manifest);
    let tools = default_function_tools(session.clone(), &agent.capabilities)?;

    let prepared_agent = agent.agent.clone_with(|prepared| {
        prepared.instructions = Some(instructions);
        prepared.function_tools.extend(tools);
        prepared.sandbox_runtime = Some(AgentSandboxRuntime {
            base_instructions: agent.base_instructions.clone(),
            user_instructions: agent.agent.instructions.clone(),
            capabilities: agent.capabilities.clone(),
            session: session.clone(),
        });
    });

    Ok(PreparedSandboxRun {
        agent: prepared_agent,
        session,
    })
}

impl AgentSandboxRuntime {
    pub fn snapshot(&self) -> Option<SandboxRunState> {
        if !self.session.runner_owned() {
            return None;
        }
        let _ = self.session.refresh_snapshot_state();
        Some(SandboxRunState {
            base_instructions: self.base_instructions.clone(),
            user_instructions: self.user_instructions.clone(),
            capabilities: self.capabilities.clone(),
            session_state: self.session.session_state(),
        })
    }
}

pub(crate) fn restore_agent_from_run_state(
    agent: &Agent,
    state: Option<&SandboxRunState>,
) -> Result<Agent> {
    let Some(state) = state else {
        return Ok(agent.clone());
    };

    let session = LocalSandboxSession::resume(state.session_state.clone())?;
    let manifest = session.manifest();
    let instructions = build_instructions_from_parts(
        state.base_instructions.as_deref(),
        state.user_instructions.as_deref(),
        &state.capabilities,
        &manifest,
    );
    let tools = default_function_tools(session.clone(), &state.capabilities)?;

    Ok(agent.clone_with(|prepared| {
        prepared.instructions = Some(instructions);
        prepared.function_tools.retain(|tool| {
            !matches!(
                tool.definition.name.as_str(),
                "sandbox_list_files"
                    | "sandbox_read_file"
                    | "sandbox_run_shell"
                    | "sandbox_apply_patch"
            )
        });
        prepared.function_tools.extend(tools);
        prepared.sandbox_runtime = Some(AgentSandboxRuntime {
            base_instructions: state.base_instructions.clone(),
            user_instructions: state.user_instructions.clone(),
            capabilities: state.capabilities.clone(),
            session,
        });
    }))
}

fn create_temp_workspace_root() -> Result<PathBuf> {
    let root = std::env::temp_dir().join(format!("openai-agents-sandbox-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).map_err(|error| AgentsError::message(error.to_string()))?;
    Ok(root)
}

fn build_instructions(agent: &SandboxAgent, manifest: &Manifest) -> String {
    build_instructions_from_parts(
        agent.base_instructions.as_deref(),
        agent.agent.instructions.as_deref(),
        &agent.capabilities,
        manifest,
    )
}

fn build_instructions_from_parts(
    base_instructions: Option<&str>,
    user_instructions: Option<&str>,
    capabilities: &[SandboxCapability],
    manifest: &Manifest,
) -> String {
    let mut parts = Vec::new();
    if let Some(base) = base_instructions {
        parts.push(base.to_owned());
    }
    if let Some(instructions) = user_instructions {
        parts.push(instructions.to_owned());
    }
    parts.push(format!(
        "Capabilities: {}",
        capabilities
            .iter()
            .map(SandboxCapability::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    parts.push(format!("Workspace layout:\n{}", manifest.describe()));
    parts.join("\n\n")
}

fn default_function_tools(
    session: LocalSandboxSession,
    capabilities: &[SandboxCapability],
) -> Result<Vec<FunctionTool>> {
    #[derive(Deserialize, JsonSchema)]
    struct PathArgs {
        path: String,
    }

    #[derive(Deserialize, JsonSchema)]
    struct PatchArgs {
        path: String,
        replacement: String,
    }

    #[derive(Deserialize, JsonSchema)]
    struct ShellArgs {
        command: String,
    }

    let mut tools = Vec::new();

    for capability in capabilities {
        match capability {
            SandboxCapability::Filesystem => {
                let list_session = session.clone();
                tools.push(function_tool(
                    "sandbox_list_files",
                    "List files inside the sandbox workspace",
                    move |_ctx, args: PathArgs| {
                        let session = list_session.clone();
                        async move { session.list_files(&args.path) }
                    },
                )?);

                let read_session = session.clone();
                tools.push(function_tool(
                    "sandbox_read_file",
                    "Read a UTF-8 text file from the sandbox workspace",
                    move |_ctx, args: PathArgs| {
                        let session = read_session.clone();
                        async move { session.read_file(&args.path) }
                    },
                )?);
            }
            SandboxCapability::Shell => {
                let shell_session = session.clone();
                tools.push(function_tool(
                    "sandbox_run_shell",
                    "Run a shell command inside the sandbox workspace and return its exit code, stdout, and stderr",
                    move |_ctx, args: ShellArgs| {
                        let session = shell_session.clone();
                        async move {
                            let output = session.run_shell(&args.command)?;
                            Ok(format!(
                                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                                output.exit_code, output.stdout, output.stderr
                            ))
                        }
                    },
                )?);
            }
            SandboxCapability::ApplyPatch => {
                let patch_session = session.clone();
                tools.push(function_tool(
                    "sandbox_apply_patch",
                    "Replace a sandbox workspace file with patched contents",
                    move |_ctx, args: PatchArgs| {
                        let session = patch_session.clone();
                        async move {
                            session
                                .apply_patch(ApplyPatchOperation {
                                    path: args.path.clone(),
                                    replacement: args.replacement,
                                })
                                .map(|result| format!("patched {}", result.path))
                        }
                    },
                )?);
            }
        }
    }

    Ok(tools)
}

fn materialize_manifest(manifest: &Manifest, workspace_root: &Path) -> Result<()> {
    for (path, entry) in &manifest.entries {
        materialize_entry(entry, workspace_root, Path::new(path))?;
    }
    Ok(())
}

fn materialize_entry(
    entry: &ManifestEntry,
    workspace_root: &Path,
    relative_path: &Path,
) -> Result<()> {
    let destination = workspace_root.join(relative_path);
    match entry {
        ManifestEntry::File(file) => {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| AgentsError::message(error.to_string()))?;
            }
            fs::write(destination, &file.content)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        }
        ManifestEntry::Dir(dir) => {
            fs::create_dir_all(&destination)
                .map_err(|error| AgentsError::message(error.to_string()))?;
            for (child, child_entry) in &dir.entries {
                materialize_entry(child_entry, workspace_root, &relative_path.join(child))?;
            }
        }
        ManifestEntry::LocalDir(local_dir) => {
            copy_local_dir(&local_dir.src, &destination)?;
        }
    }
    Ok(())
}

fn validate_running_live_session_manifest_update(
    current_manifest: &Manifest,
    processed_manifest: &Manifest,
) -> Result<()> {
    if processed_manifest.root != current_manifest.root {
        return Err(AgentsError::message(
            "Running injected sandbox sessions do not support capability changes to `manifest.root`; use a fresh session or a session_state resume flow.",
        ));
    }
    Ok(())
}

fn diff_live_session_entries(
    current_entries: &BTreeMap<String, ManifestEntry>,
    processed_entries: &BTreeMap<String, ManifestEntry>,
    parent_rel: &Path,
) -> Result<Vec<(PathBuf, ManifestEntry)>> {
    let removed = current_entries
        .keys()
        .filter(|key| !processed_entries.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    if !removed.is_empty() {
        let removed_paths = removed
            .iter()
            .map(|rel| parent_rel.join(rel).display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(AgentsError::message(format!(
            "Running injected sandbox sessions do not support removing manifest entries: {removed_paths}."
        )));
    }

    let mut entries_to_apply = Vec::new();
    for (rel_name, processed_entry) in processed_entries {
        let rel_path = parent_rel.join(rel_name);
        let Some(current_entry) = current_entries.get(rel_name) else {
            entries_to_apply.push((rel_path, processed_entry.clone()));
            continue;
        };

        if let Some(delta_entry) =
            diff_live_session_entry(&rel_path, current_entry, processed_entry)?
        {
            entries_to_apply.push((rel_path, delta_entry));
        }
    }

    Ok(entries_to_apply)
}

fn diff_live_session_entry(
    rel_path: &Path,
    current_entry: &ManifestEntry,
    processed_entry: &ManifestEntry,
) -> Result<Option<ManifestEntry>> {
    if current_entry == processed_entry {
        return Ok(None);
    }

    if std::mem::discriminant(current_entry) != std::mem::discriminant(processed_entry) {
        return Err(AgentsError::message(format!(
            "Running injected sandbox sessions do not support replacing manifest entry types at {}; use a fresh session or a session_state resume flow.",
            rel_path.display()
        )));
    }

    match (current_entry, processed_entry) {
        (ManifestEntry::Dir(current_dir), ManifestEntry::Dir(processed_dir)) => {
            let changed_children = diff_live_session_entries(
                &current_dir.entries,
                &processed_dir.entries,
                Path::new(""),
            )?;
            if changed_children.is_empty() {
                return Ok(None);
            }

            let mut entries = BTreeMap::new();
            for (child_path, child_entry) in changed_children {
                entries.insert(child_path.display().to_string(), child_entry);
            }
            Ok(Some(ManifestEntry::Dir(Dir { entries })))
        }
        _ => Ok(Some(processed_entry.clone())),
    }
}

fn copy_local_dir(source: &Path, destination: &Path) -> Result<()> {
    validate_local_dir_source_root(source)?;
    let source_snapshot = snapshot_local_dir_source(source)?;
    let parent = destination
        .parent()
        .ok_or_else(|| AgentsError::message("local dir destination must have a parent"))?;
    fs::create_dir_all(parent).map_err(|error| AgentsError::message(error.to_string()))?;

    let staging_destination = parent.join(format!(".localdir-stage-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging_destination)
        .map_err(|error| AgentsError::message(error.to_string()))?;

    let copy_result = copy_local_dir_contents(source, &staging_destination, &source_snapshot);
    if copy_result.is_err() {
        let _ = fs::remove_dir_all(&staging_destination);
        return copy_result;
    }

    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| AgentsError::message(error.to_string()))?;
    }

    fs::rename(&staging_destination, destination)
        .map_err(|error| AgentsError::message(error.to_string()))
}

fn copy_local_dir_contents(
    source: &Path,
    destination: &Path,
    root_snapshot: &LocalDirSourceSnapshot,
) -> Result<()> {
    maybe_wait_on_local_dir_test_hook(source)?;

    for entry in fs::read_dir(source).map_err(|error| AgentsError::message(error.to_string()))? {
        let entry = entry.map_err(|error| AgentsError::message(error.to_string()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let entry_snapshot = snapshot_local_dir_source(&source_path)?;
        match entry_snapshot.kind {
            LocalDirEntryKind::Dir => {
                fs::create_dir_all(&destination_path)
                    .map_err(|error| AgentsError::message(error.to_string()))?;
                copy_local_dir_contents(&source_path, &destination_path, root_snapshot)?;
            }
            LocalDirEntryKind::File => {
                fs::copy(&source_path, &destination_path)
                    .map_err(|error| AgentsError::message(error.to_string()))?;
            }
        }
        ensure_local_dir_source_unchanged(&entry_snapshot)?;
        ensure_local_dir_source_unchanged(root_snapshot)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalDirEntryKind {
    File,
    Dir,
}

fn validate_local_dir_source_root(source: &Path) -> Result<()> {
    if !source.exists() {
        return Err(AgentsError::message(format!(
            "local dir source does not exist: {}",
            source.display()
        )));
    }
    snapshot_local_dir_source(source).map(|_| ())
}

#[derive(Clone, Debug)]
struct LocalDirSourceSnapshot {
    path: PathBuf,
    canonical_path: PathBuf,
    kind: LocalDirEntryKind,
    #[cfg(unix)]
    device_id: u64,
    #[cfg(unix)]
    inode: u64,
    size: u64,
    modified: Option<std::time::SystemTime>,
}

fn snapshot_local_dir_source(path: &Path) -> Result<LocalDirSourceSnapshot> {
    ensure_no_symlinked_ancestors(path)?;
    let metadata = ensure_not_symlink(path, "local dir source")?;
    let kind = if metadata.is_dir() {
        LocalDirEntryKind::Dir
    } else if metadata.is_file() {
        LocalDirEntryKind::File
    } else {
        return Err(AgentsError::message(format!(
            "local dir source must contain only regular files and directories: {}",
            path.display()
        )));
    };

    Ok(LocalDirSourceSnapshot {
        path: path.to_path_buf(),
        canonical_path: fs::canonicalize(path)
            .map_err(|error| AgentsError::message(error.to_string()))?,
        kind,
        #[cfg(unix)]
        device_id: std::os::unix::fs::MetadataExt::dev(&metadata),
        #[cfg(unix)]
        inode: std::os::unix::fs::MetadataExt::ino(&metadata),
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn ensure_local_dir_source_unchanged(snapshot: &LocalDirSourceSnapshot) -> Result<()> {
    let current = snapshot_local_dir_source(&snapshot.path)?;
    let unchanged = snapshot.kind == current.kind
        && snapshot.canonical_path == current.canonical_path
        && snapshot.size == current.size
        && snapshot.modified == current.modified
        && {
            #[cfg(unix)]
            {
                snapshot.device_id == current.device_id && snapshot.inode == current.inode
            }
            #[cfg(not(unix))]
            {
                true
            }
        };
    if unchanged {
        Ok(())
    } else {
        Err(AgentsError::message(format!(
            "local dir source changed during copy: {}",
            snapshot.path.display()
        )))
    }
}

fn ensure_not_symlink(path: &Path, context: &str) -> Result<fs::Metadata> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| AgentsError::message(error.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(AgentsError::message(format!(
            "{context} cannot be a symlink: {}",
            path.display()
        )));
    }
    Ok(metadata)
}

fn ensure_no_symlinked_ancestors(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AgentsError::message(format!(
                    "local dir source cannot use a symlinked ancestor: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(error) => return Err(AgentsError::message(error.to_string())),
        }
    }
    Ok(())
}

fn maybe_wait_on_local_dir_test_hook(source: &Path) -> Result<()> {
    let Some(raw) = std::env::var_os("OPENAI_AGENTS_SANDBOX_LOCALDIR_TEST_HOOK") else {
        return Ok(());
    };

    let raw = raw
        .into_string()
        .map_err(|_| AgentsError::message("local dir test hook must be valid UTF-8"))?;
    let mut parts = raw.splitn(3, '|');
    let expected_source = parts.next().unwrap_or_default();
    let trigger_path = parts.next().unwrap_or_default();
    let release_path = parts.next().unwrap_or_default();
    if expected_source.is_empty() || trigger_path.is_empty() || release_path.is_empty() {
        return Ok(());
    }

    if source != Path::new(expected_source) {
        return Ok(());
    }

    fs::write(trigger_path, b"ready").map_err(|error| AgentsError::message(error.to_string()))?;
    let release_path = Path::new(release_path);
    let started = Instant::now();
    while !release_path.exists() {
        if started.elapsed() > Duration::from_secs(5) {
            return Err(AgentsError::message(format!(
                "timed out waiting for local dir test hook release: {}",
                release_path.display()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn ensure_path_stays_within_workspace(workspace_root: &Path, candidate: &Path) -> Result<()> {
    let workspace_real = fs::canonicalize(workspace_root)
        .map_err(|error| AgentsError::message(error.to_string()))?;

    if !candidate.starts_with(workspace_root) {
        return Err(AgentsError::message(
            "path must stay within the sandbox workspace",
        ));
    }

    let relative = candidate
        .strip_prefix(workspace_root)
        .map_err(|_| AgentsError::message("path must stay within the sandbox workspace"))?;

    let mut current = workspace_root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    let resolved = fs::canonicalize(&current)
                        .map_err(|error| AgentsError::message(error.to_string()))?;
                    if !resolved.starts_with(&workspace_real) {
                        return Err(AgentsError::message(
                            "path must stay within the sandbox workspace",
                        ));
                    }
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(error) => return Err(AgentsError::message(error.to_string())),
        }
    }

    if candidate.exists() {
        let resolved =
            fs::canonicalize(candidate).map_err(|error| AgentsError::message(error.to_string()))?;
        if !resolved.starts_with(&workspace_real) {
            return Err(AgentsError::message(
                "path must stay within the sandbox workspace",
            ));
        }
    }

    Ok(())
}

fn validate_shell_command(logical_root: &str, command: &str) -> Result<()> {
    let tokens = shell_like_split(command)?;
    let logical_root = Path::new(logical_root);

    for token in tokens {
        if token == ".."
            || token.starts_with("../")
            || token.contains("/../")
            || token.ends_with("/..")
        {
            return Err(AgentsError::message(
                "shell command must stay within the sandbox workspace",
            ));
        }

        if token.starts_with('/') && !Path::new(&token).starts_with(logical_root) {
            return Err(AgentsError::message(
                "shell command must stay within the sandbox workspace",
            ));
        }
    }

    Ok(())
}

fn sandbox_command_env(workspace_root: &Path) -> BTreeMap<String, String> {
    let mut env_vars: BTreeMap<String, String> = env::vars().collect();
    env_vars.insert(
        "HOME".to_owned(),
        workspace_root.as_os_str().to_string_lossy().into_owned(),
    );
    env_vars
}

fn workspace_shell_command(workspace_root: &Path, command: &str) -> String {
    format!(
        "cd {} && {}",
        shell_single_quote(workspace_root.as_os_str().to_string_lossy().as_ref()),
        command
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn confined_shell_command(
    command: &str,
    workspace_root: &Path,
    env_vars: &BTreeMap<String, String>,
) -> Result<Command> {
    let mut process = if cfg!(target_os = "macos") {
        let sandbox_exec = darwin_sandbox_exec()?;
        let profile = darwin_exec_profile(workspace_root, env_vars, Path::new("/bin/sh"));
        let mut command_builder = Command::new(sandbox_exec);
        command_builder
            .arg("-p")
            .arg(profile)
            .arg("/bin/sh")
            .arg("-lc")
            .arg(command);
        command_builder
    } else {
        let mut command_builder = Command::new("/bin/sh");
        command_builder.arg("-lc").arg(command);
        command_builder
    };
    process.envs(env_vars);
    Ok(process)
}

fn confined_pty_command(
    command: &str,
    workspace_root: &Path,
    env_vars: &BTreeMap<String, String>,
) -> Result<Command> {
    let mut process = Command::new("/usr/bin/script");
    process.arg("-q").arg("/dev/null");

    if cfg!(target_os = "macos") {
        let sandbox_exec = darwin_sandbox_exec()?;
        let profile = darwin_exec_profile(workspace_root, env_vars, Path::new("/bin/sh"));
        process
            .arg(sandbox_exec)
            .arg("-p")
            .arg(profile)
            .arg("/bin/sh")
            .arg("-lc")
            .arg(command);
    } else {
        process.arg("/bin/sh").arg("-lc").arg(command);
    }

    process.envs(env_vars);
    Ok(process)
}

fn darwin_sandbox_exec() -> Result<&'static str> {
    let sandbox_exec = "/usr/bin/sandbox-exec";
    if Path::new(sandbox_exec).exists() {
        Ok(sandbox_exec)
    } else {
        Err(AgentsError::message(
            "unix-local sandbox confinement requires /usr/bin/sandbox-exec on macOS",
        ))
    }
}

fn darwin_exec_profile(
    workspace_root: &Path,
    env_vars: &BTreeMap<String, String>,
    executable: &Path,
) -> String {
    let mut extra_read_paths = darwin_additional_read_paths(env_vars, executable);
    extra_read_paths.sort();
    extra_read_paths.dedup();

    let denied_paths = [
        "/Users",
        "/Volumes",
        "/Applications",
        "/Library",
        "/opt",
        "/etc",
        "/private/etc",
        "/tmp",
        "/private/tmp",
        "/private",
        "/var",
        "/usr",
    ];

    let mut rules = vec!["(version 1)".to_owned(), "(allow default)".to_owned()];

    for path in denied_paths {
        rules.push(format!(
            "(deny file-read-data (subpath {}))",
            sandbox_profile_literal(path)
        ));
        rules.push(format!(
            "(deny file-write* (subpath {}))",
            sandbox_profile_literal(path)
        ));
    }

    let mut workspace_paths = vec![workspace_root.to_path_buf()];
    if let Ok(canonical_workspace_root) = fs::canonicalize(workspace_root) {
        if canonical_workspace_root != workspace_root {
            workspace_paths.push(canonical_workspace_root);
        }
    }

    for workspace_path in workspace_paths {
        rules.push(format!(
            "(allow file-read-data file-read-metadata (subpath {}))",
            sandbox_profile_literal(&workspace_path)
        ));
        rules.push(format!(
            "(allow file-write* (subpath {}))",
            sandbox_profile_literal(&workspace_path)
        ));
    }

    for path in extra_read_paths {
        rules.push(format!(
            "(allow file-read-data file-read-metadata (subpath {}))",
            sandbox_profile_literal(path)
        ));
    }

    rules.extend([
        "(allow file-read-data file-read-metadata (subpath \"/usr/bin\"))".to_owned(),
        "(allow file-read-data file-read-metadata (subpath \"/usr/lib\"))".to_owned(),
        "(allow file-read-data file-read-metadata (subpath \"/bin\"))".to_owned(),
        "(allow file-read-data file-read-metadata (subpath \"/System\"))".to_owned(),
        "(allow file-read-data file-read-metadata (literal \"/private/var/select/sh\"))".to_owned(),
        "(allow file-write* (literal \"/dev/null\"))".to_owned(),
    ]);

    rules.join("\n")
}

fn sandbox_profile_literal(path: impl AsRef<Path>) -> String {
    format!(
        "\"{}\"",
        path.as_ref()
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}

fn darwin_additional_read_paths(
    env_vars: &BTreeMap<String, String>,
    executable: &Path,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(parent) = executable.parent() {
        paths.push(parent.to_path_buf());
    }

    if let Some(path_var) = env_vars.get("PATH") {
        for entry in path_var.split(':').filter(|entry| !entry.is_empty()) {
            paths.extend(darwin_allowable_read_roots(Path::new(entry)));
        }
    }

    paths
}

fn darwin_allowable_read_roots(path: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let expanded = expand_tilde(path);

    if expanded.is_dir() {
        candidates.push(expanded.clone());
    } else if let Some(parent) = expanded.parent() {
        candidates.push(parent.to_path_buf());
    }

    let resolved = fs::canonicalize(&expanded).unwrap_or(expanded.clone());
    if resolved.is_dir() {
        candidates.push(resolved.clone());
    } else if let Some(parent) = resolved.parent() {
        candidates.push(parent.to_path_buf());
    }

    let resolved_text = resolved.as_os_str().to_string_lossy();
    if resolved_text == "/opt/homebrew" || resolved_text.starts_with("/opt/homebrew/") {
        candidates.push(PathBuf::from("/opt/homebrew"));
    }
    if resolved_text == "/usr/local" || resolved_text.starts_with("/usr/local/") {
        candidates.push(PathBuf::from("/usr/local"));
    }
    if resolved_text == "/Library/Frameworks" || resolved_text.starts_with("/Library/Frameworks/") {
        candidates.push(PathBuf::from("/Library/Frameworks"));
    }

    candidates
}

fn expand_tilde(path: &Path) -> PathBuf {
    let raw = path.as_os_str().to_string_lossy();
    if raw == "~" {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    path.to_path_buf()
}

fn shell_like_split(command: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            Some('"') => {
                if ch == '"' {
                    quote = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
            }
            _ => match ch {
                '\'' | '"' => quote = Some(ch),
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(ch),
            },
        }
    }

    if quote.is_some() {
        return Err(AgentsError::message(
            "shell command contains an unterminated quote",
        ));
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

fn read_process_output<R>(mut reader: R, output: Arc<Mutex<Vec<u8>>>)
where
    R: Read,
{
    let mut buffer = [0_u8; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                output
                    .lock()
                    .expect("sandbox pty output lock")
                    .extend_from_slice(&buffer[..read]);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

const WORKSPACE_FINGERPRINT_VERSION: &str = "workspace_sha256_v1";

fn snapshot_workspace(
    workspace_root: &Path,
    session: &LocalSandboxSession,
) -> Result<(String, PathBuf)> {
    let fingerprint = fingerprint_workspace(workspace_root)?;
    let previous_snapshot_root = session
        .inner
        .persisted
        .lock()
        .expect("sandbox persisted lock")
        .snapshot_root
        .clone();
    let snapshot_root = previous_snapshot_root.unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "openai-agents-sandbox-snapshot-{}",
            uuid::Uuid::new_v4()
        ))
    });

    if snapshot_root.exists() {
        clear_directory(&snapshot_root)?;
    } else {
        fs::create_dir_all(&snapshot_root)
            .map_err(|error| AgentsError::message(error.to_string()))?;
    }
    copy_directory_contents(workspace_root, &snapshot_root)?;
    Ok((fingerprint, snapshot_root))
}

fn fingerprint_workspace(workspace_root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    fingerprint_path(workspace_root, workspace_root, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn fingerprint_path(root: &Path, path: &Path, hasher: &mut Sha256) -> Result<()> {
    let mut entries = fs::read_dir(path)
        .map_err(|error| AgentsError::message(error.to_string()))?
        .map(|entry| entry.map_err(|error| AgentsError::message(error.to_string())))
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let entry_path = entry.path();
        let relative = entry_path
            .strip_prefix(root)
            .map_err(|error| AgentsError::message(error.to_string()))?;
        hasher.update(relative.to_string_lossy().as_bytes());
        let metadata = fs::symlink_metadata(&entry_path)
            .map_err(|error| AgentsError::message(error.to_string()))?;
        if metadata.is_dir() {
            hasher.update(b"dir");
            fingerprint_path(root, &entry_path, hasher)?;
        } else if metadata.is_file() {
            hasher.update(b"file");
            hasher.update(
                fs::read(&entry_path).map_err(|error| AgentsError::message(error.to_string()))?,
            );
        } else if metadata.file_type().is_symlink() {
            hasher.update(b"symlink");
            hasher.update(
                fs::read_link(&entry_path)
                    .map_err(|error| AgentsError::message(error.to_string()))?
                    .to_string_lossy()
                    .as_bytes(),
            );
        }
    }
    Ok(())
}

fn clear_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|error| AgentsError::message(error.to_string()))?;
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|error| AgentsError::message(error.to_string()))? {
        let entry = entry.map_err(|error| AgentsError::message(error.to_string()))?;
        let entry_path = entry.path();
        if entry
            .file_type()
            .map_err(|error| AgentsError::message(error.to_string()))?
            .is_dir()
        {
            fs::remove_dir_all(&entry_path)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        } else {
            fs::remove_file(&entry_path)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        }
    }
    Ok(())
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).map_err(|error| AgentsError::message(error.to_string()))?;
    for entry in fs::read_dir(source).map_err(|error| AgentsError::message(error.to_string()))? {
        let entry = entry.map_err(|error| AgentsError::message(error.to_string()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| AgentsError::message(error.to_string()))?;
        if file_type.is_dir() {
            copy_directory_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| AgentsError::message(error.to_string()))?;
            }
            fs::copy(&source_path, &destination_path)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        } else if file_type.is_symlink() {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| AgentsError::message(error.to_string()))?;
            }
            std::os::unix::fs::symlink(
                fs::read_link(&source_path)
                    .map_err(|error| AgentsError::message(error.to_string()))?,
                &destination_path,
            )
            .map_err(|error| AgentsError::message(error.to_string()))?;
        }
    }
    Ok(())
}

impl Drop for LocalSandboxPtySessionInner {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Ok(mut reader) = self.reader.lock() {
            if let Some(handle) = reader.take() {
                let _ = handle.join();
            }
        }
    }
}
