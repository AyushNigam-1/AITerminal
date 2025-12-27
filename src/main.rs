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

PROTOCOL:
1. You must reply using these prefixes:
   MSG: <Text to show the user>
   CMD: <Linux command to run>
2. You can provide BOTH in one reply (MSG first, then CMD).

CRITICAL RULE - VERIFICATION:
- After you issue a CMD, the system will execute it and return the "COMMAND_OUTPUT" to you.
- You will NOT stop. You MUST read that output.
- If the output shows success, reply with a MSG confirming it to the user.
- If the output shows a silent failure (like 'rm -f' on a missing file) or an error, explain it in a MSG and propose a fix with a new CMD.

Example Flow:
User: "delete file"
AI: CMD: rm file
System: COMMAND_OUTPUT: ...
AI: MSG: File deleted successfully.

Do not assume success until you see the output.
"#.into(),
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

        // 🔁 Reasoning loop
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
            print!("\r\x1b[K");

            let should_continue = handle_ai_reply(&reply, &mut history).await;

            if !should_continue {
                break;
            }
        }
    }
}

async fn handle_ai_reply(reply: &str, history: &mut Vec<Message>) -> bool {
    let mut msg_part = String::new();
    let mut cmd_part = String::new();

    for line in reply.lines() {
        if let Some(rest) = line.trim().strip_prefix("MSG:") {
            if !msg_part.is_empty() {
                msg_part.push('\n');
            }
            msg_part.push_str(rest.trim());
        } else if let Some(rest) = line.trim().strip_prefix("CMD:") {
            cmd_part = rest.trim().to_string();
        } else if !cmd_part.is_empty() {
            // multiline command support (optional, keeping simple)
        } else if !msg_part.is_empty() {
            msg_part.push('\n');
            msg_part.push_str(line.trim());
        }
    }

    // 1. Show the Message
    if !msg_part.is_empty() {
        println!("{} {}", "AI:".bold().green(), msg_part);
    }

    // 2. Handle the Command
    if !cmd_part.is_empty() {
        println!(
            "{} {}",
            "Proposed command:".bold().yellow(),
            cmd_part.cyan()
        );
        print!("{}", "Execute? (y/n): ".bold());
        io::stdout().flush().unwrap();

        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm).unwrap();

        if !confirm.trim().eq_ignore_ascii_case("y") {
            println!("{}", "Cancelled.".dimmed());
            history.push(Message {
                role: "assistant".into(),
                content: reply.to_string(),
            });
            history.push(Message {
                role: "user".into(),
                content: "Cancelled.".into(),
            });
            return false;
        }

        let summary = execute_command_and_capture(&cmd_part);

        // Print the "User View" of the execution
        println!("{}", summary.user_view);

        // Update history
        history.push(Message {
            role: "assistant".into(),
            content: reply.to_string(),
        });
        history.push(Message {
            role: "user".into(), // System feedback
            content: format!("COMMAND_OUTPUT:\n{}", summary.ai_view),
        });

        // 🟢 FIX: Always return true so AI can verify the result
        return true;
    }

    // If only MSG, we are done
    history.push(Message {
        role: "assistant".into(),
        content: reply.to_string(),
    });
    false
}

struct CommandResult {
    exit_code: i32,
    user_view: String,
    ai_view: String,
}

fn execute_command_and_capture(cmd: &str) -> CommandResult {
    println!("{} {}", "Executing:".bold().green(), cmd.bright_white());

    let output = Command::new("sh").arg("-c").arg(cmd).output();

    match output {
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(-1);
            let stdout = truncate(&String::from_utf8_lossy(&out.stdout));
            let stderr = truncate(&String::from_utf8_lossy(&out.stderr));

            // Neutral reporting so we don't lie to the user
            let user_view = if exit_code == 0 {
                format!(
                    "{}\n{}",
                    format!("✔ Process finished (Exit {})", exit_code).green(),
                    stdout
                )
            } else {
                format!(
                    "{}\n{}",
                    format!("✖ Process failed (Exit {})", exit_code).red(),
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
            user_view: format!("{} {}", "✖ System Error:".red(), e),
            ai_view: format!("command: {}\nerror: {}", cmd, e),
        },
    }
}

fn truncate(s: &str) -> String {
    if s.len() > MAX_CAPTURE_LEN {
        format!("{}\n... (truncated)", &s[..MAX_CAPTURE_LEN])
    } else {
        s.to_string()
    }
}
