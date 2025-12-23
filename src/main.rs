use colored::*;
use std::io::{self, Write};

fn main() {
    println!("{}", "Welcome to your AI Terminal!".bold().green());
    println!("{}", "Type 'exit' or press Ctrl+C to quit.\n".dimmed());

    loop {
        print!("{}", "> ".cyan().bold());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();

        match input {
            "exit" | "quit" => {
                println!("{}", "Goodbye!".bold().yellow());
                break;
            }
            "" => continue,
            _ => {
                println!("{} {}", "Echo:".dimmed(), input.bright_blue());
            }
        }
    }
}
