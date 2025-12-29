use crate::cmd;
use crate::groq::Message;
use colored::*;
use std::{
    io::{self, Write},
    path::PathBuf,
};

pub async fn handle_reply(
    reply: &str,
    history: &mut Vec<Message>,
    current_dir: &mut PathBuf,
    has_display: bool,
) -> bool {
    let mut msg = String::new();
    let mut cmd = String::new();

    // 1. Parse Response
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

    // 2. Check Display Capabilities
    if !has_display && cmd::is_screenshot_command(&cmd) {
        println!(
            "{}",
            "Screenshots blocked: no graphical display.".red().bold()
        );
        history.push(Message {
            role: "assistant".into(),
            content: "Screenshot blocked: no graphical display.".into(),
        });
        return false;
    }

    // 3. Handle 'cd' internally
    if let Some(path) = cmd.strip_prefix("cd").map(|s| s.trim()) {
        let target = cmd::resolve_cd_target(path, current_dir);
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

    // 4. Confirm & Execute
    println!("{} {}", "Proposed command:".bold().yellow(), cmd.cyan());
    print!("{}", "Execute? (y/n): ".bold());
    io::stdout().flush().unwrap();

    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm).unwrap();

    if !confirm.trim().eq_ignore_ascii_case("y") {
        println!("{}", "Cancelled.".dimmed());
        return false;
    }

    let result = cmd::execute_and_capture(&cmd, current_dir);
    println!("{}", result.user_view);

    history.push(Message {
        role: "assistant".into(),
        content: reply.to_string(),
    });
    history.push(Message {
        role: "user".into(),
        content: format!("COMMAND_OUTPUT:\n{}", result.ai_view),
    });

    true
}
