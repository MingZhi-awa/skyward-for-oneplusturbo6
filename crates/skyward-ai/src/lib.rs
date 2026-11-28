//! A chat with a model, asked over the network.
//!
//! DeepSeek's API is the one asked first, and it is OpenAI's shape: a POST
//! of the conversation so far to `chat/completions`, answered as it is
//! written when `stream` is true, a chunk at a time, each a line of JSON
//! after `data: `, and `data: [DONE]` at the end. Any other server that
//! takes and answers the same may be named instead.

pub mod balance;
pub mod chat;
pub mod config;

pub use balance::{Balance, Money, ask as balance};
pub use chat::{Answer, Citation, Message, Piece, Role, Stopped, stream};
pub use config::Ai;
