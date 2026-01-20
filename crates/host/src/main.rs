mod agent;
mod mutation_agent;
mod store;

use std::io::{self, BufRead, Write};

use anyhow::Result;

use se_runtime_core::capability_runner::CapabilityRunner;
use se_runtime_core::embedding::MicrosoftFoundryEmbedder;
use se_runtime_core::foundry_client::FoundryClient;

use agent::Agent;
use store::CapabilityStore;

fn main() -> Result<()> {
    let capabilities_root = "capabilities";

    // Initialise services.
    let embedder = MicrosoftFoundryEmbedder::from_env()?;
    let ai_client = FoundryClient::from_env()?;

    // Mutation agent can use a different (coding-focused) model.
    // Falls back to FOUNDRY_CHAT_DEPLOYMENT if FOUNDRY_MUTATION_DEPLOYMENT is not set.
    let mutation_client =
        FoundryClient::from_env_with_deployment_var("FOUNDRY_MUTATION_DEPLOYMENT")
            .or_else(|_| FoundryClient::from_env())?;

    let runner = CapabilityRunner::new(capabilities_root);

    // Load capability store (state).
    let mut store = CapabilityStore::load(capabilities_root, &embedder)?;
    println!("Loaded {} capabilities from registry.", store.len());
    println!("\nSelf-Evolving Agent Runtime");
    println!("Type your task and press Enter. Type 'quit' or 'exit' to stop.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        // Prompt
        print!("> ");
        stdout.flush()?;

        // Read input
        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let task = input.trim();

        // Exit conditions
        if task.is_empty() {
            continue;
        }
        if task.eq_ignore_ascii_case("quit") || task.eq_ignore_ascii_case("exit") {
            println!("Goodbye!");
            break;
        }

        // Find relevant capabilities for this task
        let (caps_summary, nearest) = store.capabilities_summary_for_task(task, &embedder, 5)?;
        println!("\nNearest capabilities:");
        for (id, score) in &nearest {
            println!("  - {id} (score = {score:.3})");
        }

        // Run the agent
        let mut agent = Agent::new(
            &ai_client,
            &mutation_client,
            &mut store,
            &runner,
            &embedder,
            capabilities_root,
        );
        match agent.run_task(task, &caps_summary) {
            Ok(answer) => {
                println!("\n[FINAL ANSWER]");
                println!("{answer}\n");
            }
            Err(e) => {
                println!("\n[ERROR] {e}\n");
            }
        }
    }

    Ok(())
}
