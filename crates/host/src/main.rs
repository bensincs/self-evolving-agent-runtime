mod coding_agent;
mod log;
mod runtime_agent;
mod store;

use std::io::{self, BufRead, Write};

use anyhow::Result;

use se_runtime_core::capability_runner::CapabilityRunner;
use se_runtime_core::embedding::MicrosoftFoundryEmbedder;
use se_runtime_core::foundry_client::FoundryClient;

use runtime_agent::RuntimeAgent;
use store::CapabilityStore;

fn main() -> Result<()> {
    let capabilities_root = "capabilities";

    // Initialize services
    let embedder = MicrosoftFoundryEmbedder::from_env()?;
    let client = FoundryClient::from_env()?;
    let runner = CapabilityRunner::new(capabilities_root)?;

    // Load capability store
    let mut store = CapabilityStore::load(capabilities_root, &embedder)?;
    println!("Loaded {} capabilities.", store.len());
    println!("\nSelf-Evolving Agent Runtime");
    println!("Type your task and press Enter. Type 'quit' to exit.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let task = input.trim();

        if task.is_empty() {
            continue;
        }
        if task.eq_ignore_ascii_case("quit") || task.eq_ignore_ascii_case("exit") {
            println!("Goodbye!");
            break;
        }

        // Find relevant capabilities
        let (caps_summary, nearest) = store.capabilities_summary_for_task(task, &embedder, 2)?;
        println!("\nNearest capabilities:");
        for (id, score) in &nearest {
            println!("  - {id} (score = {score:.3})");
        }

        // Run agent
        let mut agent = RuntimeAgent::new(
            &client,
            &mut store,
            &runner,
            &embedder,
            capabilities_root,
        );

        match agent.run_task(task, &caps_summary) {
            Ok(answer) => println!("\n{answer}\n"),
            Err(e) => println!("\n[ERROR] {e}\n"),
        }
    }

    Ok(())
}
