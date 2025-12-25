use colored::*;
use std::io::{self, Write};
use std::process::{Command, Stdio};

mod command_policy;
mod groq; // 👈 THIS WAS MISSING
mod utils;
use groq::{GroqClient, Message};
// mod command_policy;
use command_policy::{CommandResult, CommandRisk, classify_command};
use utils::confirm_yes;

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
        let risk = classify_command(cmd);

        println!("{} {}", "Proposed command:".bold().yellow(), cmd.cyan());

        match risk {
            CommandRisk::Safe => {
                if confirm_yes("Execute this command? (y/n): ") {
                    execute_command(cmd);
                }
            }

            CommandRisk::Caution => {
                println!(
                    "{}",
                    "⚠ This command modifies files or processes."
                        .bold()
                        .yellow()
                );
                if confirm_yes("Are you sure? (y/n): ") {
                    execute_command(cmd);
                }
            }

            CommandRisk::Dangerous => {
                println!("{}", "🔥 DANGEROUS COMMAND DETECTED!".bold().red());
                println!("{}", "To confirm, type the FULL command again:".bold());

                let mut typed = String::new();
                io::stdin().read_line(&mut typed).unwrap();

                if typed.trim() == cmd {
                    execute_command(cmd);
                } else {
                    println!("{}", "Command cancelled.".bold().yellow());
                }
            }
        }
    } else {
        println!("{} {}", "Invalid AI reply:".bold().red(), reply.dimmed());
    }
}

/// Execute shell command safely (non-blocking)
fn execute_command(cmd: &str) -> CommandResult {
    println!("{} {}", "Executing:".bold().green(), cmd.bright_white());

    let output = Command::new("sh").arg("-c").arg(cmd).output();

    match output {
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(-1);

            let stdout = String::from_utf8_lossy(&out.stdout)
                .chars()
                .take(800)
                .collect::<String>();

            let stderr_raw = String::from_utf8_lossy(&out.stderr).to_string();

            let stderr = if stderr_raw.trim().is_empty() {
                "(no stderr output)".to_string()
            } else {
                stderr_raw.chars().take(800).collect()
            };

            let success = out.status.success();

            if success {
                println!("{}", "✔ Command succeeded".green().bold());
            } else {
                println!(
                    "{} exit code {}",
                    "✖ Command failed".red().bold(),
                    exit_code
                );
            }

            CommandResult {
                success,
                exit_code,
                stdout,
                stderr,
            }
        }
        Err(e) => {
            println!("{} {}", "✖ Failed to execute command:".bold().red(), e);

            CommandResult {
                success: false,
                exit_code: -1,
                stdout: "".into(),
                stderr: e.to_string(),
            }
        }
    }
}
