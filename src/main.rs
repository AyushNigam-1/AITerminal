use colored::*;
use std::{
    env,
    io::{self, Write},
};

mod cmd;
mod groq;
mod handler;
mod sys;

use groq::{AudioRecorder, GroqClient, Message};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env file not found");

    let mut current_dir = env::current_dir().expect("Failed to get cwd");
    let (has_display, wayland, x11) = sys::detect_display();
    let system_info = sys::gather_info(&current_dir, has_display, wayland, x11);

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
            MSG: <text>
            CMD: <linux command>
            
            RULES:
            - After CMD execution, you will receive COMMAND_OUTPUT.
            - You MUST verify the output before claiming success.
            "#,
            system_info
        ),
    }];

    // --- MAIN LOOP ---
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

        // Variable to hold either the typed text OR the transcribed voice text
        let mut final_prompt = input.to_string();

        // 🎤 VOICE MODE TRIGGER
        // If the user typed ":rec" or ":voice", we start recording
        if input == ":rec" || input == ":voice" {
            println!(
                "{}",
                "🎙️  Recording... Press ENTER to stop.".red().bold().blink()
            );

            // 1. Start Audio Driver
            let recorder = match AudioRecorder::start() {
                Ok(r) => r,
                Err(e) => {
                    println!("{} {}", "Failed to init mic:".red(), e);
                    continue;
                }
            };

            // 2. Block until user hits Enter
            let mut pause = String::new();
            io::stdin().read_line(&mut pause).unwrap();

            // 3. Stop and Save
            let temp_file = "voice_temp.wav";
            println!("{}", "Processing audio...".dimmed());

            if let Err(e) = recorder.stop_and_save(temp_file) {
                println!("{} {}", "Audio save error:".red(), e);
                continue;
            }

            // 4. Send to Cloud (Groq Whisper)
            // Note: This blocks the UI briefly. For a smoother experience, you could wrap this in a spinner too.
            match groq_client.transcribe_audio(temp_file).await {
                Ok(text) => {
                    println!("{} {}", "Transcribed:".green().bold(), text.italic());
                    final_prompt = text; // Replace ":rec" with the actual spoken words

                    // Cleanup the temporary wav file
                    let _ = std::fs::remove_file(temp_file);
                }
                Err(e) => {
                    println!("{} {}", "Transcription failed:".red(), e);
                    continue;
                }
            }
        }

        // If transcription returned empty string or user just hit enter, skip processing
        if final_prompt.is_empty() {
            continue;
        }

        // Push the final prompt (typed or spoken) to history
        history.push(Message {
            role: "user".into(),
            content: final_prompt.clone(),
        });

        // --- AI PROCESSING LOOP ---
        loop {
            // 1. Create spinner channel
            let (tx, mut rx) = tokio::sync::oneshot::channel();

            // 2. Spawn the spinner in a separate background task
            let spinner_handle = tokio::spawn(async move {
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let mut i = 0;
                loop {
                    // Check if we received the stop signal
                    if rx.try_recv().is_ok() {
                        break;
                    }

                    print!("\r{} AI thinking...", frames[i % frames.len()].cyan());
                    io::stdout().flush().unwrap();

                    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                    i += 1;
                }
                // Clear the spinner line when done
                print!("\r\x1b[K");
                io::stdout().flush().unwrap();
            });

            // 3. Call AI
            let reply_result = groq_client.chat(history.clone()).await;

            // 4. Stop spinner
            let _ = tx.send(());
            spinner_handle.await.unwrap(); // Wait for spinner to clean up

            // 5. Handle the result
            let reply = match reply_result {
                Ok(r) => r,
                Err(err) => {
                    println!("{} {}", "Error:".red(), err);
                    break;
                }
            };

            // Clear line one last time to be safe
            print!("\r\x1b[K");

            let should_continue = handler::handle_reply(
                &reply,
                &mut history,
                &mut current_dir,
                has_display,
                &groq_client, // Note: Ensure your handler signature accepts groq_client if needed for recursion
                              // If your handler doesn't take groq_client, remove this argument.
                              // Based on previous context, you might not be passing groq_client to handler yet,
                              // but checking your provided file, it looks like you updated handler signature.
            )
            .await;

            if !should_continue {
                break;
            }
        }
    }
}
