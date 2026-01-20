use base64::{Engine as _, engine::general_purpose};
use reqwest::{Client, multipart};
use std::time::Duration;
use thiserror::Error;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

use super::types::Message;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

#[derive(Debug, Error)]
pub enum GroqError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Groq API error: {0}")]
    Api(String),
}

pub struct GroqClient {
    http: Client,
    api_key: String,
    model: String,
}

impl GroqClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            http: Client::builder()
                .timeout(Duration::from_secs(30)) // 30s timeout prevents infinite hangs
                .build()
                .unwrap_or_default(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub async fn chat(&self, messages: Vec<Message>) -> Result<String, GroqError> {
        let payload = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 512
        });

        let res = self
            .http
            .post(GROQ_API_URL)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(GroqError::Api(res.text().await?));
        }

        let json: serde_json::Value = res.json().await?;
        Ok(json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    pub async fn analyze_image_file(
        &self,
        image_path: &str,
        user_prompt: &str,
    ) -> Result<String, GroqError> {
        // 1️⃣ Load image bytes
        let image_bytes = std::fs::read(image_path)
            .map_err(|e| GroqError::Api(format!("Failed to read image: {}", e)))?;

        // 2️⃣ Encode as base64
        let encoded = general_purpose::STANDARD.encode(&image_bytes);

        // 3️⃣ Build vision-aware input
        // Use the Responses API (vision models) instead of chat completions
        let payload = serde_json::json!({
            "model": self.model,
            "input": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": user_prompt
                        },
                        {
                            "type": "input_image",
                            "image": {
                                "data": encoded,
                                "mime": "image/png"
                            }
                        }
                    ]
                }
            ]
        });

        // 4️⃣ Send request
        let url = format!("{}/responses", GROQ_API_URL);
        let res = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(GroqError::Api(res.text().await?));
        }

        let json: serde_json::Value = res.json().await?;
        // Extract text out of the first output entry (model answer)
        let answer = json["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(answer)
    }

    pub async fn transcribe_audio(&self, file_path: &str) -> Result<String, GroqError> {
        // 1. Prepare File Upload
        let file = File::open(file_path)
            .await
            .map_err(|e| GroqError::Api(format!("File not found: {}", e)))?;

        let stream = FramedRead::new(file, BytesCodec::new());
        let file_part = multipart::Part::stream(reqwest::Body::wrap_stream(stream))
            .file_name("recording.wav")
            .mime_str("audio/wav")
            .unwrap();

        // 2. Build Form
        let form = multipart::Form::new()
            .part("file", file_part)
            .text("model", "whisper-large-v3-turbo") // Optimized for speed
            .text("response_format", "json");

        // 3. Send Request to Transcription Endpoint
        let res = self
            .http
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(GroqError::Api(res.text().await?));
        }

        let json: serde_json::Value = res.json().await?;
        Ok(json["text"].as_str().unwrap_or("").to_string())
    }
}
