use clap::Parser;
use acore::{AgentExecutor, AgentProvider};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 実行するプロンプト
    prompt: String,

    /// 使用するプロバイダー (gemini, claude, codex, opencode)
    #[arg(short, long, default_value = "gemini")]
    provider: String,

    /// 要約して amem に記録するかどうか
    #[arg(short, long)]
    record: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let provider = match args.provider.as_str() {
        "gemini" => AgentProvider::Gemini,
        "claude" => AgentProvider::Claude,
        "codex" => AgentProvider::Codex,
        "opencode" => AgentProvider::OpenCode,
        _ => AgentProvider::Gemini,
    };

    // ストリーミング実行（標準出力に出力）
    AgentExecutor::execute_stream(provider.clone(), &args.prompt, |line| {
        println!("{}", line);
    }).await?;

    // 必要に応じて amem に記録
    if args.record {
        // 将来的に実装
    }

    Ok(())
}
