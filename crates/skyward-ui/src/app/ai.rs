//! The chat page: what the model says as it says it, and how the answer
//! ended. The question leaves as an [`Effect::Ask`](Effect::Ask), and the
//! answer comes back a piece at a time, drawn as it arrives. The page that
//! sets the chat up keeps what it types on the same [`Chat`], so that a key
//! typed there is the one the next question is asked with.

use super::*;

/// A text field of the page where the chat is set up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiField {
    /// Where the server answers, without `chat/completions` on the end.
    Address,
    /// The model asked.
    Model,
    /// The key sent as the bearer of the request.
    Key,
}

impl App {
    /// A piece of the answer being written arrived: its words, a step it
    /// took to the web, or that it is whole.
    pub fn ai_said(&mut self, piece: Piece) {
        let Some(turn) = self.state.chat.writing.as_mut() else { return };
        match piece {
            // What the model worked out is not shown: the page is small,
            // and the answer is what is wanted.
            Piece::Reasoning(_) | Piece::Done => {}
            Piece::Content(words) => turn.said.push_str(&words),
            Piece::Searched(queries) => turn.steps.push(Step::Searched(queries)),
            Piece::Opened(url) => turn.steps.push(Step::Opened(url)),
            Piece::Found { title, url } => turn.steps.push(Step::Found { title, url }),
            Piece::Failed(why) => turn.steps.push(Step::Failed(why)),
        }
        self.refresh_text();
    }

    /// The answer is whole, or it stopped: what it came to, or why it did
    /// not come at all.
    pub fn ai_ended(&mut self, ended: Result<Box<AiAnswer>, String>) {
        let Some(mut turn) = self.state.chat.writing.take() else { return };
        match ended {
            Ok(answer) => {
                if turn.said.is_empty() {
                    turn.said = answer.content.clone();
                }
                turn.cites = answer.citations.into_iter().map(|c| c.url).collect();
                if let Some(name) = answer.title {
                    self.state.chat.title = Some(name);
                }
                turn.calls = answer.calls;
            }
            Err(why) => turn.why = Some(why),
        }
        self.state.chat.turns.push(turn);
        self.refresh();
    }

    /// What the account behind the key holds, as the server said, or why it
    /// could not be asked: the page writes it in its own words.
    pub fn ai_balance(&mut self, balance: Result<Balance, String>) {
        self.state.chat.balance = Some(balance);
        self.refresh();
    }
}
