use clap::Parser;

mod llm_manager;
mod interpreter;
mod planner;
mod executor;
mod reviewer;
mod agentic_loop;
mod concurrency;
mod ui;
mod logger;

use llm_manager::{LLMManager, LocalProvider};
use agentic_loop::AgenticLoop;

#[derive(Parser)]
#[command(name = "cli_engineer")]
struct Args {
    /// Run without UI output
    #[arg(short, long)]
    headless: bool,
    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
    /// Command or natural language instruction
    #[arg(last = true)]
    command: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    logger::init(args.verbose);
    let mut ui = ui::UIHandler::new(args.headless);
    ui.start()?;
    let llm_manager = LLMManager::new(vec![Box::new(LocalProvider)]);
    let agent = AgenticLoop::new(&llm_manager, 1);
    let input = args.command.join(" ");
    agent.run(&input).await?;
    ui.finish();
    Ok(())
}
