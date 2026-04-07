use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use crate::model::{ThreadChatTree, ThreadChatTreeNode};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub struct CodexAppServer {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: AtomicU64,
}

impl CodexAppServer {
    pub async fn spawn(codex_home: &Path) -> Result<Arc<Self>, String> {
        let mut command = build_codex_command(codex_home);
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let mut child = command.spawn().map_err(|err| err.to_string())?;
        let stdin = child.stdin.take().ok_or("missing stdin")?;
        let stdout = child.stdout.take().ok_or("missing stdout")?;
        let stderr = child.stderr.take().ok_or("missing stderr")?;

        let client = Arc::new(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        });

        let client_stdout = Arc::clone(&client);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
                    continue;
                };
                if let Some(id) = value.get("id").and_then(Value::as_u64) {
                    if let Some(tx) = client_stdout.pending.lock().await.remove(&id) {
                        let _ = tx.send(value);
                    }
                }
            }
            client_stdout.pending.lock().await.clear();
        });

        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(_line)) = lines.next_line().await {}
        });

        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&self) -> Result<(), String> {
        self.send_request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "codex_chat_tree_overlay",
                    "title": "Codex Chat Tree Overlay",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        )
        .await?;
        self.send_notification("initialized", None).await
    }

    async fn write_message(&self, value: Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().await;
        let mut line = serde_json::to_string(&value).map_err(|err| err.to_string())?;
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|err| err.to_string())
    }

    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<(), String> {
        let value = if let Some(params) = params {
            json!({ "method": method, "params": params })
        } else {
            json!({ "method": method })
        };
        self.write_message(value).await
    }

    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        if let Err(err) = self
            .write_message(json!({ "id": id, "method": method, "params": params }))
            .await
        {
            self.pending.lock().await.remove(&id);
            return Err(err);
        }
        let response = timeout(REQUEST_TIMEOUT, rx)
            .await
            .map_err(|_| "request timed out".to_string())?
            .map_err(|_| "request canceled".to_string())?;
        if let Some(error) = response.get("error") {
            return Err(format!("app-server error for {method}: {error}"));
        }
        Ok(response)
    }

    pub async fn is_alive(&self) -> bool {
        let mut child = self.child.lock().await;
        matches!(child.try_wait(), Ok(None))
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let mut child = self.child.lock().await;
        match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => child.kill().await.map_err(|err| err.to_string()),
            Err(err) => Err(err.to_string()),
        }
    }

    pub async fn resume_thread(&self, thread_id: &str) -> Result<(), String> {
        self.send_request("thread/resume", json!({ "threadId": thread_id }))
            .await
            .map(|_| ())
    }

    pub async fn read_chat_tree(&self, thread_id: &str) -> Result<ThreadChatTree, String> {
        let response = self
            .send_request("thread/chatTree/read", json!({ "threadId": thread_id }))
            .await?;
        parse_chat_tree(&response)
    }

    pub async fn set_current_node(&self, thread_id: &str, node_id: &str) -> Result<String, String> {
        let response = self
            .send_request(
                "thread/chatTree/current/set",
                json!({
                    "threadId": thread_id,
                    "nodeId": node_id
                }),
            )
            .await?;
        response
            .get("result")
            .and_then(|value| {
                value
                    .get("currentNodeId")
                    .or_else(|| value.get("current_node_id"))
            })
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .ok_or_else(|| "missing currentNodeId in set-current response".to_string())
    }
}

fn parse_chat_tree(response: &Value) -> Result<ThreadChatTree, String> {
    let chat_tree = response
        .get("result")
        .and_then(|value| value.get("chatTree").or_else(|| value.get("chat_tree")))
        .ok_or("missing chatTree result")?;

    let current_node_id = chat_tree
        .get("currentNodeId")
        .or_else(|| chat_tree.get("current_node_id"))
        .and_then(Value::as_str)
        .map(|value| value.to_string());

    let nodes = chat_tree
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let node_id = entry
                .get("nodeId")
                .or_else(|| entry.get("node_id"))
                .and_then(Value::as_str)?
                .to_string();
            Some(ThreadChatTreeNode {
                node_id,
                parent_node_id: entry
                    .get("parentNodeId")
                    .or_else(|| entry.get("parent_node_id"))
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                summary: entry
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                turn_id: entry
                    .get("turnId")
                    .or_else(|| entry.get("turn_id"))
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                order: entry.get("order").and_then(Value::as_u64).unwrap_or(0) as u32,
            })
        })
        .collect();

    Ok(ThreadChatTree {
        current_node_id,
        nodes,
    })
}

fn build_codex_command(codex_home: &Path) -> Command {
    let mut command = Command::new(resolve_codex_bin());
    command.arg("app-server");
    command.current_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    command.env("CODEX_HOME", codex_home);
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW.0);
    }
    if let Some(path_env) = build_codex_path_env() {
        command.env("PATH", path_env);
    }
    command
}

fn build_codex_path_env() -> Option<String> {
    let mut paths: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();

    let mut extras: Vec<PathBuf> = Vec::new();
    if let Ok(app_data) = env::var("APPDATA") {
        extras.push(Path::new(&app_data).join("npm"));
    }
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        extras.push(
            Path::new(&local_app_data)
                .join("Microsoft")
                .join("WindowsApps"),
        );
    }
    if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
        let home_path = Path::new(&home);
        extras.push(home_path.join(".cargo").join("bin"));
        extras.push(home_path.join("scoop").join("shims"));
    }
    if let Ok(program_data) = env::var("PROGRAMDATA") {
        extras.push(Path::new(&program_data).join("chocolatey").join("bin"));
    }

    for extra in extras {
        if !paths.iter().any(|candidate| candidate == &extra) {
            paths.push(extra);
        }
    }

    env::join_paths(paths)
        .ok()
        .map(|value| value.to_string_lossy().to_string())
}

fn resolve_codex_bin() -> PathBuf {
    if let Ok(explicit) = env::var("CODEX_CLI_PATH") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(explicit) = env::var("CODEX_CHAT_TREE_BIN") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(explicit) = env::var("CODEX_BIN") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let chat_tree_debug = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("codex")
        .join("codex-rs")
        .join("target")
        .join("debug")
        .join("codex.exe");
    if chat_tree_debug.exists() {
        return chat_tree_debug;
    }

    let repo_bundle = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("resources")
        .join("bin")
        .join("win32-x64")
        .join("codex.exe");
    if repo_bundle.exists() {
        return repo_bundle;
    }

    PathBuf::from("codex")
}
