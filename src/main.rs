use colored::*;
use std::{
    env,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

mod groq;
use groq::{GroqClient, Message};

const MAX_CAPTURE_LEN: usize = 800;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env file not found");

    let mut current_dir = env::current_dir().expect("Failed to get cwd");

    // 🔒 Real capability detection
    let has_display = env::var("DISPLAY").is_ok() || env::var("WAYLAND_DISPLAY").is_ok();
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let is_x11 = std::env::var("DISPLAY").is_ok() && !is_wayland;

    let system_info = gather_system_info(&current_dir, has_display, is_wayland, is_x11);

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
                - Use:
                MSG: <text>
                CMD: <linux command>
                - You may include BOTH (MSG first, then CMD).

                CRITICAL RULES:
                - The system enforces capabilities. Do NOT assume success.
                - Screenshots are IMPOSSIBLE if display=false.
                - After CMD execution, you will receive COMMAND_OUTPUT.
                - You must verify success from the output.
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
                Ok(r) => r,
                Err(err) => {
                    println!("{} {}", "Error:".red(), err);
                    break;
                }
            };
            print!("\r\x1b[K");

            let should_continue =
                handle_ai_reply(&reply, &mut history, &mut current_dir, has_display).await;

            if !should_continue {
                break;
            }
        }
    }
}

async fn handle_ai_reply(
    reply: &str,
    history: &mut Vec<Message>,
    current_dir: &mut PathBuf,
    has_display: bool,
) -> bool {
    let mut msg = String::new();
    let mut cmd = String::new();

    for line in reply.lines() {
        if let Some(rest) = line.strip_prefix("MSG:") {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str(rest.trim());
        } else if let Some(rest) = line.strip_prefix("CMD:") {
            cmd = rest.trim().to_string();
        }
    }

    if !msg.is_empty() {
        println!("{} {}", "AI:".bold().green(), msg);
    }

    if cmd.is_empty() {
        history.push(Message {
            role: "assistant".into(),
            content: reply.to_string(),
        });
        return false;
    }

    // 🚫 HARD BLOCK: screenshots without display
    if !has_display && is_screenshot_command(&cmd) {
        println!(
            "{}",
            "Screenshots are not possible: no graphical display available."
                .red()
                .bold()
        );

        history.push(Message {
            role: "assistant".into(),
            content: "Screenshot blocked: no graphical display.".into(),
        });

        return false;
    }

    // cd interception (real shell behavior)
    if let Some(path) = cmd.strip_prefix("cd").map(|s| s.trim()) {
        let target = resolve_cd_target(path, current_dir);
        if target.exists() && target.is_dir() {
            *current_dir = target;
            println!(
                "{} {}",
                "Directory changed to".green(),
                current_dir.display()
            );
            history.push(Message {
                role: "assistant".into(),
                content: format!("Changed directory to {}", current_dir.display()),
            });
        } else {
            println!("{} {}", "cd failed:".red(), target.display());
            history.push(Message {
                role: "user".into(),
                content: format!("cd failed: {}", target.display()),
            });
        }
        return false;
    }

    println!("{} {}", "Proposed command:".bold().yellow(), cmd.cyan());
    print!("{}", "Execute? (y/n): ".bold());
    io::stdout().flush().unwrap();

    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm).unwrap();

    if !confirm.trim().eq_ignore_ascii_case("y") {
        println!("{}", "Cancelled.".dimmed());
        return false;
    }

    let result = execute_command_and_capture(&cmd, current_dir);
    println!("{}", result.user_view);

    history.push(Message {
        role: "assistant".into(),
        content: reply.to_string(),
    });
    history.push(Message {
        role: "user".into(),
        content: format!("COMMAND_OUTPUT:\n{}", result.ai_view),
    });

    // Always let AI verify output
    true
}

struct CommandResult {
    exit_code: i32,
    user_view: String,
    ai_view: String,
}

fn execute_command_and_capture(cmd: &str, dir: &PathBuf) -> CommandResult {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .output();

    match output {
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            let stdout = truncate(&String::from_utf8_lossy(&out.stdout));
            let stderr = truncate(&String::from_utf8_lossy(&out.stderr));

            CommandResult {
                exit_code: code,
                user_view: if code == 0 {
                    format!("{}\n{}", "✔ Success".green(), stdout)
                } else {
                    format!("{}\n{}", "✖ Failed".red(), stderr)
                },
                ai_view: format!(
                    "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                    code, stdout, stderr
                ),
            }
        }
        Err(e) => CommandResult {
            exit_code: -1,
            user_view: format!("{} {}", "✖ Error:".red(), e),
            ai_view: e.to_string(),
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

fn is_screenshot_command(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    c.contains("scrot")
        || c.contains("gnome-screenshot")
        || c.contains("import")
        || c.contains("screencapture")
}

fn resolve_cd_target(path: &str, cwd: &PathBuf) -> PathBuf {
    if path.is_empty() || path == "~" {
        dirs_next::home_dir().unwrap_or_else(|| cwd.clone())
    } else {
        let p = PathBuf::from(path);
        if p.is_absolute() { p } else { cwd.join(p) }
    }
}

fn gather_system_info(cwd: &PathBuf, display: bool, wayland: bool, x11: bool) -> String {
    format!(
        "\
        OS: {}
        User: {}
        Display available: {}
        Initial working directory: {}
        Display server:\n- Wayland: {}\n- X11: {},
        ",
        env::consts::OS,
        env::var("USER").unwrap_or_else(|_| "unknown".into()),
        display,
        cwd.display(),
        wayland,
        x11
    )
}
