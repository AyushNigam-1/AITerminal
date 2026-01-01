use colored::*;
use std::{
    env,
    io::{self, Write},
};

mod cmd;
mod groq;
mod handler;
mod sys;

use groq::{GroqClient, Message};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env file not found");

    let mut current_dir = env::current_dir().expect("Failed to get cwd");
    let (has_display, wayland, x11) = sys::detect_display();
    let system_info = sys::gather_info(&current_dir, has_display, wayland, x11);

    println!("{}", "Welcome to your AI Terminal!".bold().green());
    println!(
        "{} {}\n",
        "Working directory:".dimmed(),
        current_dir.display().to_string().cyan()
    );

    let groq_client = GroqClient::new(
        env::var("GROQ_API_KEY").expect("GROQ_API_KEY not set"),
        "openai/gpt-oss-120b",
    );

    let mut history: Vec<Message> = vec![Message {
        role: "system".into(),
        content: format!(
            r#"
            You are an AI-powered terminal assistant.
            
            SYSTEM INFORMATION:
            {}

            PROTOCOL:
            MSG: <text>
            CMD: <linux command>
            
            RULES:
            - After CMD execution, you will receive COMMAND_OUTPUT.
            - You MUST verify the output before claiming success.
            "#,
            system_info
        ),
    }];

    loop {
        print!("{} ", format!("{} >", current_dir.display()).cyan().bold());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("{}", "Goodbye!".bold().yellow());
            break;
        }

        if input.is_empty() {
            continue;
        }

        history.push(Message {
            role: "user".into(),
            content: input.to_string(),
        });

        loop {
            print!("{}", "AI thinking...\r".dimmed());
            io::stdout().flush().unwrap();

            let reply = match groq_client.chat(history.clone()).await {
                Ok(r) => {
                    // ADD THIS DEBUG LINE:
                    println!("[DEBUG] Received response!");
                    r
                }
                Err(err) => {
                    println!("{} {}", "Error:".red(), err);
                    break;
                }
            };
            print!("\r\x1b[K");

            let should_continue = handler::handle_reply(
                &reply,
                &mut history,
                &mut current_dir,
                has_display,
                &groq_client,
            )
            .await;

            if !should_continue {
                break;
            }
        }
    }
}
