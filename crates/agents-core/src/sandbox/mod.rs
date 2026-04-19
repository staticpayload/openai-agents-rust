use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::agent::{Agent, AgentBuilder};
use crate::errors::{AgentsError, Result};
use crate::tool::{FunctionTool, function_tool};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxConcurrencyLimits {
    pub manifest_entries: Option<usize>,
    pub local_dir_files: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxRunConfig {
    pub manifest: Option<Manifest>,
    pub concurrency_limits: SandboxConcurrencyLimits,
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
        self.capabilities = Some(capabilities);
        self
    }

    pub fn build(self) -> SandboxAgent {
        SandboxAgent {
            agent: self.agent_builder.build(),
            default_manifest: self.default_manifest,
            base_instructions: self.base_instructions,
            capabilities: self
                .capabilities
                .unwrap_or_else(SandboxCapability::defaults),
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
pub struct LocalSandboxSession {
    inner: Arc<LocalSandboxSessionInner>,
}

#[derive(Debug)]
struct LocalSandboxSessionInner {
    workspace_root: PathBuf,
    logical_root: String,
    manifest: Manifest,
    runner_owned: bool,
    cleaned: Mutex<bool>,
}

impl LocalSandboxSession {
    pub fn workspace_root(&self) -> PathBuf {
        self.inner.workspace_root.clone()
    }

    pub fn logical_root(&self) -> &str {
        &self.inner.logical_root
    }

    pub fn manifest(&self) -> &Manifest {
        &self.inner.manifest
    }

    pub fn runner_owned(&self) -> bool {
        self.inner.runner_owned
    }

    pub fn cleanup(&self) -> Result<()> {
        let mut cleaned = self.inner.cleaned.lock().expect("sandbox cleanup lock");
        if *cleaned {
            return Ok(());
        }
        if self.inner.runner_owned && self.inner.workspace_root.exists() {
            fs::remove_dir_all(&self.inner.workspace_root)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        }
        *cleaned = true;
        Ok(())
    }

    fn resolve_path(&self, requested: &str) -> Result<PathBuf> {
        let requested_path = Path::new(requested);
        let relative = if requested_path.is_absolute() {
            let logical_root = Path::new(&self.inner.logical_root);
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

        Ok(self.inner.workspace_root.join(normalized))
    }

    fn list_files(&self, requested: &str) -> Result<String> {
        let directory = self.resolve_path(requested)?;
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

    fn read_file(&self, requested: &str) -> Result<String> {
        let path = self.resolve_path(requested)?;
        fs::read_to_string(path).map_err(|error| AgentsError::message(error.to_string()))
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
    let manifest = sandbox_config
        .manifest
        .or_else(|| agent.default_manifest.clone())
        .unwrap_or_default();

    let workspace_root = create_temp_workspace_root()?;
    materialize_manifest(&manifest, &workspace_root)?;
    let session = LocalSandboxSession {
        inner: Arc::new(LocalSandboxSessionInner {
            workspace_root,
            logical_root: manifest.root.clone(),
            manifest: manifest.clone(),
            runner_owned: true,
            cleaned: Mutex::new(false),
        }),
    };
    let instructions = build_instructions(agent, &manifest);
    let tools = default_function_tools(session.clone())?;

    let prepared_agent = agent.agent.clone_with(|prepared| {
        prepared.instructions = Some(instructions);
        prepared.function_tools.extend(tools);
    });

    Ok(PreparedSandboxRun {
        agent: prepared_agent,
        session,
    })
}

fn create_temp_workspace_root() -> Result<PathBuf> {
    let root = std::env::temp_dir().join(format!("openai-agents-sandbox-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).map_err(|error| AgentsError::message(error.to_string()))?;
    Ok(root)
}

fn build_instructions(agent: &SandboxAgent, manifest: &Manifest) -> String {
    let mut parts = Vec::new();
    if let Some(base) = &agent.base_instructions {
        parts.push(base.clone());
    }
    if let Some(instructions) = &agent.agent.instructions {
        parts.push(instructions.clone());
    }
    parts.push(format!(
        "Capabilities: {}",
        agent
            .capabilities
            .iter()
            .map(SandboxCapability::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    parts.push(format!("Workspace layout:\n{}", manifest.describe()));
    parts.join("\n\n")
}

fn default_function_tools(session: LocalSandboxSession) -> Result<Vec<FunctionTool>> {
    #[derive(Deserialize, JsonSchema)]
    struct PathArgs {
        path: String,
    }

    let list_session = session.clone();
    let list_tool = function_tool(
        "sandbox_list_files",
        "List files inside the sandbox workspace",
        move |_ctx, args: PathArgs| {
            let session = list_session.clone();
            async move { session.list_files(&args.path) }
        },
    )?;

    let read_session = session.clone();
    let read_tool = function_tool(
        "sandbox_read_file",
        "Read a UTF-8 text file from the sandbox workspace",
        move |_ctx, args: PathArgs| {
            let session = read_session.clone();
            async move { session.read_file(&args.path) }
        },
    )?;

    let shell_tool = function_tool(
        "sandbox_run_shell",
        "Report the sandbox workspace root for shell usage",
        move |_ctx, _args: PathArgs| {
            let session = session.clone();
            async move { Ok(session.workspace_root().display().to_string()) }
        },
    )?;

    let apply_patch_tool = function_tool(
        "sandbox_apply_patch",
        "Report patch availability for the sandbox workspace",
        move |_ctx, _args: PathArgs| async move {
            Ok::<_, AgentsError>("sandbox patch ready".to_owned())
        },
    )?;

    Ok(vec![list_tool, read_tool, shell_tool, apply_patch_tool])
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

fn copy_local_dir(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).map_err(|error| AgentsError::message(error.to_string()))?;
    for entry in fs::read_dir(source).map_err(|error| AgentsError::message(error.to_string()))? {
        let entry = entry.map_err(|error| AgentsError::message(error.to_string()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| AgentsError::message(error.to_string()))?;
        if file_type.is_dir() {
            copy_local_dir(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| AgentsError::message(error.to_string()))?;
        }
    }
    Ok(())
}
