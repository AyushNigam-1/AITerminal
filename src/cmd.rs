use colored::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const MAX_CAPTURE_LEN: usize = 800;

pub struct CommandResult {
    pub exit_code: i32,
    pub user_view: String,
    pub ai_view: String,
    pub suggestion: Option<String>,    // 👈 NEW FEATURE OUTPUT
    pub created_file: Option<PathBuf>, // 👈 ADD THIS
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

            // 🔍 Typo fix (existing feature)
            let suggestion = if code != 0 {
                suggest_fix(cmd, &stderr, dir)
            } else {
                None
            };

            // 🖼️ NEW: detect screenshot creation
            let created_file = if code == 0 && is_screenshot_command(cmd) {
                let candidates = ["screenshot.png", "Screenshot.png", "screen.png"];

                candidates
                    .iter()
                    .map(|name| dir.join(name))
                    .find(|path| path.exists())
            } else {
                None
            };

            let user_view = if code == 0 {
                format!("{}\n{}", "✔ Success".green().bold(), stdout)
            } else {
                format!("{}\n{}", "✖ Failed".red().bold(), stderr)
            };

            let ai_view = format!(
                "command: {}\nexit_code: {}\nstdout:\n{}\nstderr:\n{}",
                cmd, code, stdout, stderr
            );

            CommandResult {
                exit_code: code,
                user_view,
                ai_view,
                suggestion,
                created_file, // 👈 ADD THIS
            }
        }

        Err(e) => CommandResult {
            exit_code: -1,
            user_view: format!("{} {}", "✖ Error:".red().bold(), e),
            ai_view: e.to_string(),
            suggestion: None,
            created_file: None, // 👈 ADD THIS
        },
    }
}

/// 🔧 Suggest a fixed command if the failure looks like a filename typo
fn suggest_fix(cmd: &str, stderr: &str, cwd: &Path) -> Option<String> {
    let missing = extract_missing_path(stderr)?;

    let entries = fs::read_dir(cwd).ok()?;

    let mut best_match: Option<String> = None;
    let mut best_score = usize::MAX;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        let score = simple_distance(&missing, &name);

        // Heuristic threshold (small typos only)
        if score < best_score && score <= 2 {
            best_score = score;
            best_match = Some(name);
        }
    }

    let corrected = best_match?;

    // Replace only the first occurrence of the typo
    Some(cmd.replacen(&missing, &corrected, 1))
}

/// Extract missing file name from common Unix errors
fn extract_missing_path(stderr: &str) -> Option<String> {
    // Examples:
    // rm: cannot remove 'index.hmtl': No such file or directory
    // cat: index.tx: No such file or directory

    let markers = [
        "cannot remove '",
        "cannot access '",
        "No such file or directory",
    ];

    for marker in markers {
        if let Some(pos) = stderr.find(marker) {
            let after = &stderr[pos + marker.len()..];
            if let Some(end) = after.find('\'') {
                return Some(after[..end].to_string());
            }
        }
    }

    // Fallback: last word before colon
    stderr.split(':').nth(1).map(|s| s.trim().to_string())
}

/// Very small, safe distance function (no external crates)
fn simple_distance(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    let len_diff = a_bytes.len().abs_diff(b_bytes.len());

    let mismatch = a_bytes
        .iter()
        .zip(b_bytes.iter())
        .filter(|(x, y)| x != y)
        .count();

    len_diff + mismatch
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
