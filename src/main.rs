use colored::*;
use std::io::{self, Write};
use std::process::Command;

mod groq;
use groq::{GroqClient, Message};

const MAX_CAPTURE_LEN: usize = 800;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env file not found");

    println!("{}", "Welcome to your AI Terminal!".bold().green());
    println!("{}", "Type 'exit' or press Ctrl+C to quit.\n".dimmed());

    let groq_client = GroqClient::new(
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
- Only real Linux shell commands
- No tool names, no pseudo-code
- No markdown
- No explanations with CMD
- Prefer safe, common Ubuntu commands
- If a command fails, analyze the result and suggest a fix
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

        // 🔁 Reasoning loop (NO recursion)
        loop {
            print!("{}", "AI thinking...\r".dimmed());
            io::stdout().flush().unwrap();

            let reply = match groq_client.chat(history.clone()).await {
                Ok(r) => r,
                Err(err) => {
                    println!("{} {}", "Error:".bold().red(), err.to_string().dimmed());
                    break;
                }
            };

            let should_continue = handle_ai_reply(&reply, &mut history, &groq_client).await;

            if !should_continue {
                break;
            }
        }
    }
}

/// Handle AI output
/// Returns true if AI should continue reasoning
async fn handle_ai_reply(
    reply: &str,
    history: &mut Vec<Message>,
    _groq_client: &GroqClient,
) -> bool {
    if let Some(text) = reply.strip_prefix("CHAT:") {
        println!("{} {}", "AI:".bold().green(), text.trim());

        history.push(Message {
            role: "assistant".into(),
            content: reply.to_string(),
        });

        return false;
    }

    if let Some(cmd) = reply.strip_prefix("CMD:") {
        let cmd = cmd.trim();

        println!("{} {}", "Proposed command:".bold().yellow(), cmd.cyan());

        print!("{}", "Execute this command? (y/n): ".bold());
        io::stdout().flush().unwrap();

        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm).unwrap();

        if !confirm.trim().eq_ignore_ascii_case("y") {
            println!("{}", "Command cancelled.".dimmed());
            return false;
        }

        let summary = execute_command_and_capture(cmd);
        println!("{}", summary.user_view);

        history.push(Message {
            role: "assistant".into(),
            content: format!("COMMAND_EXECUTION_RESULT:\n{}", summary.ai_view),
        });

        // 🔁 Ask AI to reason again ONLY on failure
        if summary.exit_code != 0 {
            history.push(Message {
                role: "user".into(),
                content:
                    "The previous command failed. Analyze the error and suggest a fix or next step."
                        .into(),
            });
            return true; // continue loop
        }

        return false;
    }

    println!("{} {}", "Invalid AI reply:".bold().red(), reply.dimmed());

    false
}

/// Execution result
struct CommandResult {
    exit_code: i32,
    user_view: String,
    ai_view: String,
}

/// Execute command and capture output
fn execute_command_and_capture(cmd: &str) -> CommandResult {
    println!("{} {}", "Executing:".bold().green(), cmd.bright_white());

    let output = Command::new("sh").arg("-c").arg(cmd).output();

    match output {
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(-1);

            let stdout = truncate(&String::from_utf8_lossy(&out.stdout));
            let stderr = truncate(&String::from_utf8_lossy(&out.stderr));

            let user_view = if exit_code == 0 {
                format!("{}\n{}", "✔ Command succeeded.".green().bold(), stdout)
            } else {
                format!(
                    "{} (exit code {})\n{}",
                    "✖ Command failed.".red().bold(),
                    exit_code,
                    stderr
                )
            };

            let ai_view = format!(
                "command: {}\nexit_code: {}\nstdout:\n{}\nstderr:\n{}",
                cmd, exit_code, stdout, stderr
            );

            CommandResult {
                exit_code,
                user_view,
                ai_view,
            }
        }
        Err(e) => CommandResult {
            exit_code: -1,
            user_view: format!("{} {}", "✖ Failed to execute command:".bold().red(), e),
            ai_view: format!("command: {}\nexecution_error: {}", cmd, e),
        },
    }
}

/// Truncate long output
fn truncate(s: &str) -> String {
    if s.len() > MAX_CAPTURE_LEN {
        format!("{}\n... (truncated)", &s[..MAX_CAPTURE_LEN])
    } else {
        s.to_string()
    }
}
