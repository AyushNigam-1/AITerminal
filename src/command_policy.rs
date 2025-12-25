#[derive(Debug)]
pub enum CommandRisk {
    Safe,
    Caution,
    Dangerous,
}
pub struct CommandResult {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
pub fn classify_command(cmd: &str) -> CommandRisk {
    let cmd = cmd.trim().to_lowercase();

    let dangerous_patterns = [
        "rm -rf /",
        "rm -rf ~",
        "rm -rf *",
        "dd if=",
        "mkfs",
        "shutdown",
        "reboot",
        ":(){:|:&};:",
    ];

    for pat in dangerous_patterns {
        if cmd.contains(pat) {
            return CommandRisk::Dangerous;
        }
    }

    let caution_starts = ["rm ", "mv ", "cp ", "chmod ", "chown ", "kill ", "pkill "];

    for pat in caution_starts {
        if cmd.starts_with(pat) {
            return CommandRisk::Caution;
        }
    }

    CommandRisk::Safe
}
