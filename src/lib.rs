use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone, Hash, Eq, PartialEq)]
pub enum AgentTool {
    Gemini,
    Claude,
    Codex,
    OpenCode,
    Mock,
}

impl AgentTool {
    pub fn command_name(&self) -> &str {
        match self {
            AgentTool::Gemini => "gemini",
            AgentTool::Claude => "claude",
            AgentTool::Codex => "codex",
            AgentTool::OpenCode => "opencode",
            AgentTool::Mock => "mock-agent",
        }
    }
}

#[derive(Clone)]
pub struct SessionManager {
    session_ids: Arc<Mutex<HashMap<AgentTool, String>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            session_ids: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn extract_session_id(output: &str) -> Option<String> {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(id) = v.get("session_id").and_then(|v| v.as_str()) {
                return Some(id.to_string());
            }
            if let Some(id) = v.get("sessionId").and_then(|v| v.as_str()) {
                return Some(id.to_string());
            }
        }
        None
    }

    pub fn extract_response(output: &str) -> Option<String> {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(res) = v.get("response").and_then(|v| v.as_str()) {
                return Some(res.to_string());
            }
        }
        None
    }

    pub async fn execute_with_resume<F>(
        &self,
        tool: AgentTool,
        prompt: &str,
        mut on_chunk: F,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut(String) + Send + 'static,
    {
        if tool == AgentTool::Mock {
            on_chunk("Mock: ".into());
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            on_chunk(format!("received your prompt '{}'.", prompt));
            return Ok(());
        }

        let mut session_ids = self.session_ids.lock().await;
        let cmd = tool.command_name();
        let mut current_id = session_ids.get(&tool).cloned();

        if current_id.is_none() {
            let init_prompt = AgentExecutor::build_init_prompt().await;
            let mut seed_cmd = Command::new(cmd);
            seed_cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
            
            match tool {
                AgentTool::Gemini => {
                    seed_cmd.arg("--approval-mode").arg("yolo").arg("--output-format").arg("json").arg("-p").arg(&init_prompt);
                }
                AgentTool::Claude => {
                    seed_cmd.arg("--dangerously-skip-permissions").arg("--output-format").arg("json").arg("--print").arg(&init_prompt);
                }
                _ => { seed_cmd.arg(&init_prompt); }
            }

            let output = seed_cmd.output().await?;
            if !output.status.success() {
                return Err(format!("Seed turn failed: {}", String::from_utf8_lossy(&output.stderr)).into());
            }
            let out_str = String::from_utf8_lossy(&output.stdout);
            if let Some(id) = Self::extract_session_id(&out_str) {
                session_ids.insert(tool.clone(), id.clone());
                current_id = Some(id);
            } else {
                return Err("Failed to extract session_id from seed turn.".into());
            }
        }

        let mut command = Command::new(cmd);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let id = current_id.unwrap();

        match tool {
            AgentTool::Gemini => {
                command.arg("--approval-mode").arg("yolo").arg("--resume").arg(id).arg("-p").arg(prompt);
            }
            AgentTool::Claude => {
                command.arg("--dangerously-skip-permissions").arg("--resume").arg(id).arg("--print").arg(prompt);
            }
            _ => { command.arg(prompt); }
        }

        let mut child = command.spawn().map_err(|e| format!("Failed to spawn {}: {}", cmd, e))?;
        let mut stdout = child.stdout.take().ok_or("Failed to open stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to open stderr")?;
        let mut err_reader = BufReader::new(stderr).lines();

        let mut buffer = [0; 1024];
        loop {
            let n = stdout.read(&mut buffer).await?;
            if n == 0 { break; }
            let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
            on_chunk(chunk);
        }

        let status = child.wait().await?;
        if !status.success() {
            let mut err_msg = String::new();
            while let Ok(Some(line)) = err_reader.next_line().await {
                err_msg.push_str(&line);
                err_msg.push('\n');
            }
            return Err(format!("{} exited with error:\n{}", cmd, err_msg).into());
        }

        Ok(())
    }
}

pub struct AgentExecutor;

impl AgentExecutor {
    pub async fn has_amem() -> bool {
        Command::new("amem")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// amem の記憶から Snapshot 文字列を取得します
    pub async fn fetch_context() -> String {
        let mut context = String::new();
        if !Self::has_amem().await { return context; }

        let output = match Command::new("amem").arg("today").arg("--json").output().await {
            Ok(o) => o,
            Err(_) => return context,
        };
        if !output.status.success() { return context; }
        
        let today: serde_json::Value = match serde_json::from_slice(&output.stdout) {
            Ok(t) => t,
            Err(_) => return context,
        };

        if let Some(profile) = today["owner_profile"].as_str() {
            context.push_str("## Owner Profile\n");
            context.push_str(profile);
            context.push('\n');
        }
        if let Some(soul) = today["agent_soul"].as_str() {
            context.push_str("\n## Agent Soul\n");
            context.push_str(soul);
            context.push('\n');
        }
        if let Some(activity) = today["activity"].as_str() {
            context.push_str("\n## Recent Activities\n");
            context.push_str(activity);
            context.push('\n');
        }
        if let Some(memories) = today["agent_memories"].as_str() {
            context.push_str("\n## Important Memories (P0)\n");
            context.push_str(memories);
            context.push('\n');
        }
        context
    }

    /// amem の記憶から初期化用プロンプトを構築します
    pub async fn build_init_prompt() -> String {
        let context = Self::fetch_context().await;
        if context.is_empty() {
            return String::from("Load this amem snapshot for the next interactive session and reply exactly `MEMORY_READY`.\n\n(amem context is empty or unavailable)");
        }
        format!("Load this amem snapshot for the next interactive session and reply exactly `MEMORY_READY`.\n\n{}", context)
    }

    pub async fn execute_stream<F>(
        tool: AgentTool,
        prompt: &str,
        mut on_chunk: F,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut(String) + Send + 'static,
    {
        if tool == AgentTool::Mock {
            on_chunk("Mock stream: pong".into());
            return Ok(());
        }

        let mut child = Command::new(tool.command_name())
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdout = child.stdout.take().ok_or("Failed to open stdout")?;
        let mut buffer = [0; 1024];
        
        loop {
            let n = stdout.read(&mut buffer).await?;
            if n == 0 { break; }
            let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
            on_chunk(chunk);
        }

        let _ = child.wait().await?;
        Ok(())
    }

    pub async fn summarize_and_record(
        tool: AgentTool,
        transcript: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if tool == AgentTool::Mock { return Ok(()); }
        if transcript.is_empty() || !Self::has_amem().await { return Ok(()); }
        let output = Command::new(tool.command_name())
            .arg(format!("対話内容をAgentの活動ログとして1行で要約せよ：\n{}", transcript))
            .output().await?;
        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !line.is_empty() {
            let _ = Command::new("amem").arg("keep").arg(line).arg("--kind").arg("activity").arg("--source").arg("yuiclaw").status().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    #[test]
    fn test_extract_session_id() {
        let json_output = r#"{"session_id": "test-uuid-1234", "status": "ok"}"#;
        assert_eq!(SessionManager::extract_session_id(json_output), Some("test-uuid-1234".to_string()));
    }

    #[test]
    fn test_extract_response() {
        let json_output = r#"{"session_id": "abc", "response": "Hello, world!"}"#;
        assert_eq!(SessionManager::extract_response(json_output), Some("Hello, world!".to_string()));
    }

    #[tokio::test]
    async fn test_execute_stream_chunks() {
        let received = Arc::new(StdMutex::new(String::new()));
        let received_clone = Arc::clone(&received);
        let _ = AgentExecutor::execute_stream(AgentTool::Mock, "test", move |chunk| {
            received_clone.lock().unwrap().push_str(&chunk);
        }).await;
        assert_eq!(*received.lock().unwrap(), "Mock stream: pong");
    }
}
