use colored::*;
use std::{path::PathBuf, process::Command};

const MAX_CAPTURE_LEN: usize = 800;

pub struct CommandResult {
    pub exit_code: i32,
    pub user_view: String,
    pub ai_view: String,
}

pub fn execute_and_capture(cmd: &str, dir: &PathBuf) -> CommandResult {
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

pub fn resolve_cd_target(path: &str, cwd: &PathBuf) -> PathBuf {
    if path.is_empty() || path == "~" {
        dirs_next::home_dir().unwrap_or_else(|| cwd.clone())
    } else {
        let p = PathBuf::from(path);
        if p.is_absolute() { p } else { cwd.join(p) }
    }
}

pub fn is_screenshot_command(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    c.contains("scrot")
        || c.contains("gnome-screenshot")
        || c.contains("import")
        || c.contains("screencapture")
}

fn truncate(s: &str) -> String {
    if s.len() > MAX_CAPTURE_LEN {
        format!("{}\n... (truncated)", &s[..MAX_CAPTURE_LEN])
    } else {
        s.to_string()
    }
}
