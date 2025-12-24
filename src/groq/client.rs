use reqwest::Client;
use thiserror::Error;

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
            http: Client::new(),
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
}
