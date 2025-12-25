use colored::*;
use std::io::{self, Write};

pub fn confirm_yes(prompt: &str) -> bool {
    print!("{}", prompt.bold());
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    input.trim().eq_ignore_ascii_case("y")
}
