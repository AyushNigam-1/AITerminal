use std::{env, path::PathBuf};

pub fn gather_info(cwd: &PathBuf, display: bool, wayland: bool, x11: bool) -> String {
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

pub fn detect_display() -> (bool, bool, bool) {
    let has_display = env::var("DISPLAY").is_ok() || env::var("WAYLAND_DISPLAY").is_ok();
    let is_wayland = env::var("WAYLAND_DISPLAY").is_ok();
    let is_x11 = env::var("DISPLAY").is_ok() && !is_wayland;
    (has_display, is_wayland, is_x11)
}
