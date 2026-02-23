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
    /// テスト用のモックツール
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
        // Mock ツールの処理
        if tool == AgentTool::Mock {
            on_chunk("Mock: ".into());
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            on_chunk(format!("received your prompt '{}'.", prompt));
            return Ok(());
        }

        let mut session_ids = self.session_ids.lock().await;
        let cmd = tool.command_name();
        let mut command = Command::new(cmd);
        
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let is_resume = session_ids.contains_key(&tool);

        if let Some(id) = session_ids.get(&tool) {
            match tool {
                AgentTool::Gemini => {
                    command.arg("--approval-mode").arg("yolo").arg("--resume").arg(id).arg("-p").arg(prompt);
                }
                AgentTool::Claude => {
                    command.arg("--dangerously-skip-permissions").arg("--resume").arg(id).arg("--print").arg(prompt);
                }
                _ => { command.arg(prompt); }
            }
        } else {
            let context = AgentExecutor::fetch_context().await;
            let bootstrap = format!("{}\n\n依頼: {}", context, prompt);
            
            match tool {
                AgentTool::Gemini => {
                    command.arg("--approval-mode").arg("yolo").arg("--output-format").arg("json").arg("-p").arg(bootstrap);
                }
                AgentTool::Claude => {
                    command.arg("--dangerously-skip-permissions").arg("--output-format").arg("json").arg("--print").arg(bootstrap);
                }
                _ => { command.arg(bootstrap); }
            }
        }

        let mut child = command.spawn().map_err(|e| format!("Failed to spawn {}: {}", cmd, e))?;
        let mut stdout = child.stdout.take().ok_or("Failed to open stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to open stderr")?;
        let mut err_reader = BufReader::new(stderr).lines();

        let mut full_output = String::new();
        let mut buffer = [0; 1024];

        if is_resume {
            loop {
                let n = stdout.read(&mut buffer).await?;
                if n == 0 { break; }
                let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                on_chunk(chunk);
            }
        } else {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            while reader.read_line(&mut line).await? > 0 {
                full_output.push_str(&line);
                line.clear();
            }
        }

        let status = child.wait().await?;
        if status.success() {
            if !is_resume {
                if let Some(id) = Self::extract_session_id(&full_output) {
                    session_ids.insert(tool, id);
                }
                if let Some(clean_res) = Self::extract_response(&full_output) {
                    on_chunk(clean_res);
                } else {
                    on_chunk(full_output);
                }
            }
        } else {
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

    pub async fn fetch_context() -> String {
        if !Self::has_amem().await { return "".to_string(); }
        let output = match Command::new("amem").arg("today").arg("--json").output().await {
            Ok(o) => o,
            Err(_) => return "".to_string(),
        };
        if !output.status.success() { return "".to_string(); }
        let today: serde_json::Value = match serde_json::from_slice(&output.stdout) {
            Ok(t) => t,
            Err(_) => return "".to_string(),
        };
        format!("### プロフィール\n{}\n### 最近の活動\n{}", 
            today["owner_profile"].as_str().unwrap_or(""), 
            today["activity"].as_str().unwrap_or(""))
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
