use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

// #[derive(Debug)]
// pub enum CommandRisk {
//     Safe,
//     Caution,
//     Dangerous,
// }
