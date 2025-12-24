use colored::*;
use std::io::{self, Write};
use std::process::{Command, Stdio};

mod groq;
use groq::{GroqClient, Message};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env file not found");

    println!("{}", "Welcome to your AI Terminal!".bold().green());
    println!("{}", "Type 'exit' or press Ctrl+C to quit.\n".dimmed());

    let groq = GroqClient::new(
        std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY not set"),
        "openai/gpt-oss-120b",
    );

    let mut history: Vec<Message> = vec![Message {
        role: "system".into(),
        content: r#"
You are an AI-powered terminal assistant.

You MUST reply in exactly ONE of these formats:

CHAT: <plain conversational text>
CMD: <valid Linux shell command>

STRICT RULES:
- Never invent tool names (no list_dir, read_file, FS, etc.)
- Only output real Linux shell commands
- No markdown
- No explanations with CMD
- For opening folders on Ubuntu, use xdg-open
- If unsure, ask using CHAT
"#
        .into(),
    }];

    loop {
        print!("{}", "> ".cyan().bold());
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

        print!("{}", "AI thinking...\r".dimmed());
        io::stdout().flush().unwrap();

        match groq.chat(history.clone()).await {
            Ok(reply) => {
                handle_ai_reply(&reply);
                history.push(Message {
                    role: "assistant".into(),
                    content: reply,
                });
            }
            Err(err) => {
                println!("{} {}", "Error:".bold().red(), err.to_string().dimmed());
            }
        }
    }
}

/// Handle AI output safely
fn handle_ai_reply(reply: &str) {
    if let Some(text) = reply.strip_prefix("CHAT:") {
        println!("{} {}", "AI:".bold().green(), text.trim());
    } else if let Some(cmd) = reply.strip_prefix("CMD:") {
        let cmd = cmd.trim();

        println!("{} {}", "Proposed command:".bold().yellow(), cmd.cyan());

        print!("{}", "Do you want to execute this command? (y/n): ".bold());
        io::stdout().flush().unwrap();

        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm).unwrap();

        if confirm.trim().eq_ignore_ascii_case("y") {
            execute_command(cmd);
        } else {
            println!("{}", "Command cancelled.".dimmed());
        }
    } else {
        println!("{} {}", "Invalid AI reply:".bold().red(), reply.dimmed());
    }
}

/// Execute shell command safely (non-blocking)
fn execute_command(cmd: &str) {
    println!("{} {}", "Executing:".bold().green(), cmd.bright_white());

    let result = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    match result {
        Ok(_) => {
            println!("{}", "✔ Command started successfully.\n".dimmed());
        }
        Err(e) => {
            println!(
                "{} {}",
                "✖ Failed to execute command:".bold().red(),
                e.to_string()
            );
        }
    }
}
