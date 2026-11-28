//! Where the chat is asked, with which key, and of which model. Kept with
//! the rest of the settings, so that a start of the recovery finds it again.

use serde::{Deserialize, Serialize};

/// Where DeepSeek's API answers, which is where a recovery asks until told
/// otherwise.
pub const BASE: &str = "https://api.deepseek.com";

/// The model DeepSeek answers `deepseek-flash` with, which is the one a
/// recovery starts with.
pub const MODEL: &str = "deepseek-flash";

/// What a chat is asked with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ai {
    /// The address of the server, without `chat/completions` on the end.
    pub base_url: String,
    /// The model asked.
    pub model: String,
    /// The key sent as the bearer of the request; none until one is typed.
    pub api_key: String,
    /// The model thinks before it answers, which takes longer and costs
    /// more, and reads better on a question that needs working out.
    pub thinking: bool,
    /// The model may search the web as it pleases, and answer with the
    /// pages it found. Searched and read on the server: nothing is fetched
    /// here but the answer.
    pub web_search: bool,
    /// The model may open the links the last question named.
    pub read_links: bool,
}

impl Default for Ai {
    fn default() -> Self {
        Ai {
            base_url: BASE.into(),
            model: MODEL.into(),
            api_key: String::new(),
            thinking: false,
            web_search: true,
            read_links: true,
        }
    }
}

impl Ai {
    /// Where the conversation is posted: the server named, and the one path
    /// every server of this shape answers.
    pub fn url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// A key has been typed, so there is something to ask with.
    pub fn ready(&self) -> bool {
        !self.api_key.trim().is_empty() && !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_the_servers_and_one_path() {
        assert_eq!(Ai::default().url(), "https://api.deepseek.com/chat/completions");
        assert_eq!(Ai { base_url: "https://api.deepseek.com/".into(), ..Ai::default() }.url(), "https://api.deepseek.com/chat/completions");
        assert_eq!(Ai { base_url: "http://192.168.1.2:8000/v1".into(), ..Ai::default() }.url(), "http://192.168.1.2:8000/v1/chat/completions");
    }

    #[test]
    fn a_chat_needs_a_key() {
        assert!(!Ai::default().ready());
        assert!(!Ai { api_key: "  ".into(), ..Ai::default() }.ready(), "spaces are not a key");
        assert!(Ai { api_key: "sk-x".into(), ..Ai::default() }.ready());
        assert!(!Ai { api_key: "sk-x".into(), model: String::new(), ..Ai::default() }.ready());
    }

    #[test]
    fn what_is_not_kept_is_what_a_recovery_starts_with() {
        let kept: Ai = serde_json::from_str(r#"{ "api_key": "sk-x" }"#).unwrap();
        assert_eq!(kept, Ai { api_key: "sk-x".into(), ..Ai::default() });
    }
}
