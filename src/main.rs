use clap::Parser;
use acore::{AgentExecutor, AgentTool};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 実行するプロンプト
    prompt: String,

    /// 使用するツール (gemini, claude, codex, opencode)
    #[arg(short, long, default_value = "gemini")]
    tool: String,

    /// 要約して amem に記録するかどうか
    #[arg(short, long)]
    record: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let tool = match args.tool.as_str() {
        "gemini" => AgentTool::Gemini,
        "claude" => AgentTool::Claude,
        "codex" => AgentTool::Codex,
        "opencode" => AgentTool::OpenCode,
        _ => AgentTool::Gemini,
    };

    // ストリーミング実行（標準出力に出力）
    AgentExecutor::execute_stream(tool.clone(), &args.prompt, |line| {
        println!("{}", line);
    }).await?;

    // 必要に応じて amem に記録
    if args.record {
        // 将来的に実装
    }

    Ok(())
}
