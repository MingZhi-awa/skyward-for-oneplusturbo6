//! The conversation asked, and the answer read as it is written: a chunk at
//! a time, so that the page shows the first words while the rest is still
//! being made.
//!
//! The model may search the web, open the pages it found and read the links
//! the question named, all of it on the server: what comes back here is the
//! steps it took, the pages it looked at, and the answer with the places it
//! took its words from. What was read is kept in the calls it made, and is
//! given back with the next question so that it can still be quoted.

use crate::config::Ai;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};

/// Who said a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// One message of the conversation, as the server takes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// The steps an answer took to the web, kept whole and given back: the
    /// pages it read are restored from them, and quoted again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_tool_calls: Option<Value>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Message {
        Message { role: Role::System, content: content.into(), server_tool_calls: None }
    }

    pub fn user(content: impl Into<String>) -> Message {
        Message { role: Role::User, content: content.into(), server_tool_calls: None }
    }

    pub fn assistant(content: impl Into<String>) -> Message {
        Message { role: Role::Assistant, content: content.into(), server_tool_calls: None }
    }

    /// The answer as the next question takes it: what it said, and the
    /// steps it took to say it.
    pub fn answered(answer: &Answer) -> Message {
        Message { role: Role::Assistant, content: answer.content.clone(), server_tool_calls: answer.calls.clone() }
    }
}

/// A page an answer took its words from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub url: String,
    pub title: Option<String>,
    /// Where the words quoted begin and end in the answer, by code point.
    pub start: u32,
    pub end: u32,
}

/// A piece of the answer, as it comes: the working out the model does
/// before it answers, where it thinks, the steps it takes to the web, and
/// then the answer itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Reasoning(String),
    Content(String),
    /// A search, with the words it searched for.
    Searched(Vec<String>),
    /// A page opened, or a link read: its address.
    Opened(String),
    /// A page a search found.
    Found { title: Option<String>, url: String },
    /// A step that failed, and why the server said.
    Failed(String),
    /// The answer is whole.
    Done,
}

/// What an answer came to: its words, its working out, the pages it took
/// them from, and the steps it took, to be given back with the next
/// question.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answer {
    pub content: String,
    pub reasoning: String,
    /// A short name for the conversation, made as the answer is: asked for
    /// on the first question of a conversation, and kept.
    pub title: Option<String>,
    pub citations: Vec<Citation>,
    pub calls: Option<Value>,
}

/// What a streamed chunk carries, of the fields a page shows.
#[derive(Debug, Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    error: Option<ApiError>,
}

#[derive(Debug, Default, Deserialize)]
struct Choice {
    #[serde(default)]
    delta: Delta,
    /// A step of the answer's work to the web, one at most in a chunk.
    #[serde(default)]
    server_tool_event: Option<ToolEvent>,
}

#[derive(Debug, Default, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    /// Sent once, with the chunk that ends the answer.
    #[serde(default)]
    server_tool_calls: Option<Value>,
    #[serde(default)]
    annotations: Option<Vec<Annotation>>,
    /// Sent once, with the chunk that ends the answer, where a title was
    /// asked for.
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToolEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    call: Option<Call>,
}

#[derive(Debug, Default, Deserialize)]
struct Call {
    #[serde(default)]
    action: Option<Action>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    sources: Option<Vec<Source>>,
    #[serde(default)]
    page: Option<Page>,
    #[serde(default)]
    error: Option<CallError>,
}

#[derive(Debug, Default, Deserialize)]
struct Action {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    queries: Vec<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Source {
    url: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Page {
    url: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CallError {
    #[serde(default)]
    code: Option<String>,
}

/// Where an answer says its words came from: nested, as the chat shape
/// writes it, or flat, as the responses shape does.
#[derive(Debug, Deserialize)]
struct Annotation {
    #[serde(default)]
    url_citation: Option<Cited>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    start_index: Option<u32>,
    #[serde(default)]
    end_index: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct Cited {
    url: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    start_index: u32,
    #[serde(default)]
    end_index: u32,
}

impl Annotation {
    fn citation(&self) -> Option<Citation> {
        if let Some(cited) = &self.url_citation {
            return Some(Citation { url: cited.url.clone(), title: cited.title.clone(), start: cited.start_index, end: cited.end_index });
        }
        Some(Citation {
            url: self.url.clone()?,
            title: self.title.clone(),
            start: self.start_index.unwrap_or(0),
            end: self.end_index.unwrap_or(0),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(default)]
    message: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

impl ApiError {
    fn say(&self) -> String {
        match (&self.message, &self.kind) {
            (Some(message), Some(kind)) => format!("{message} ({kind})"),
            (Some(message), None) => message.clone(),
            (None, Some(kind)) => kind.clone(),
            (None, None) => "the server says only that it failed".into(),
        }
    }
}

/// The body of the request: the conversation, that it is answered as it is
/// written, and the tools the model may take to the web.
pub fn body(ai: &Ai, history: &[Message], title: bool) -> String {
    let thinking = if ai.thinking { "enabled" } else { "disabled" };
    let mut tools = Vec::new();
    if ai.web_search {
        tools.push(serde_json::json!({ "type": "deepseek_web_search" }));
    }
    if ai.read_links {
        tools.push(serde_json::json!({ "type": "deepseek_read_url" }));
    }
    let mut asked = serde_json::json!({
        "model": ai.model,
        "messages": history,
        "stream": true,
        "thinking": { "type": thinking },
    });
    if !tools.is_empty() {
        asked["tools"] = Value::Array(tools);
    }
    // A title is made as the answer is, free of charge, and is asked for
    // once: a conversation keeps the one it was given.
    if title {
        asked["title"] = serde_json::json!({ "enabled": true });
    }
    asked.to_string()
}

/// One line of the answer as it arrived.
#[derive(Debug)]
enum Line {
    /// Blank lines, comments, and events that say nothing to show.
    Nothing,
    /// The answer is whole.
    Done,
    Chunk(Chunk),
}

/// One line of the answer, read.
fn read(line: &str) -> Result<Line> {
    let line = line.trim();
    let Some(rest) = line.strip_prefix("data:") else {
        return Ok(Line::Nothing);
    };
    let rest = rest.trim();
    if rest == "[DONE]" {
        return Ok(Line::Done);
    }
    if rest.is_empty() {
        return Ok(Line::Nothing);
    }
    let chunk: Chunk = serde_json::from_str(rest).with_context(|| format!("read the answer {rest}"))?;
    if let Some(error) = chunk.error {
        bail!("{}", error.say());
    }
    Ok(Line::Chunk(chunk))
}

/// What one chunk has to show, in the order the page reads it.
fn pieces(chunk: &Chunk) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(choice) = chunk.choices.first() else {
        return out;
    };
    if let Some(event) = &choice.server_tool_event {
        // Only the openings and endings are shown: a step's working out
        // comes as reasoning, and is shown as it arrives.
        let said = match event.kind.as_str() {
            "call_started" => event.call.as_ref().and_then(opening),
            "call_completed" => event.call.as_ref().and_then(closing),
            _ => None,
        };
        out.extend(said);
    }
    if let Some(reasoning) = choice.delta.reasoning_content.as_ref().filter(|r| !r.is_empty()) {
        out.push(Piece::Reasoning(reasoning.clone()));
    }
    if let Some(content) = choice.delta.content.as_ref().filter(|c| !c.is_empty()) {
        out.push(Piece::Content(content.clone()));
    }
    out
}

/// A step beginning: a search and its words, or a page opened or a link
/// read and its address.
fn opening(call: &Call) -> Option<Piece> {
    let action = call.action.as_ref()?;
    match action.kind.as_str() {
        "search" => Some(Piece::Searched(action.queries.clone())),
        "open_page" | "read_link" | "find_in_page" => Some(Piece::Opened(action.url.clone().or_else(|| call.page.as_ref().map(|p| p.url.clone()))?)),
        _ => None,
    }
}

/// A step ending: the pages it found, or why it failed.
fn closing(call: &Call) -> Option<Piece> {
    if let Some(code) = &call.error {
        return Some(Piece::Failed(code.code.clone().unwrap_or_else(|| "unavailable".into())));
    }
    if call.status.as_deref() == Some("failed") {
        return Some(Piece::Failed("unavailable".into()));
    }
    if let Some(source) = call.sources.as_ref().and_then(|sources| sources.first()) {
        return Some(Piece::Found { title: source.title.clone(), url: source.url.clone() });
    }
    // A page opened rather than searched for: its title is all there is to
    // show of it.
    let page = call.page.as_ref()?;
    Some(Piece::Found { title: page.title.clone(), url: page.url.clone() })
}

/// The answer the server refused the request with, read whole.
fn refused(status: u16, body: &str) -> String {
    let said = serde_json::from_str::<Chunk>(body)
        .ok()
        .and_then(|chunk| chunk.error)
        .map(|error| error.say());
    match said {
        Some(said) => format!("the server answered {status}: {said}"),
        None if body.trim().is_empty() => format!("the server answered {status}"),
        None => format!("the server answered {status}: {}", body.trim()),
    }
}

/// Ask `ai` to continue `history`, handing each piece to `on_piece` as it
/// comes, and stopping when `stop` is set: what a page does when the answer
/// is no longer wanted.
pub fn stream(agent: &ureq::Agent, ai: &Ai, history: &[Message], title: bool, stop: &AtomicBool, on_piece: &mut dyn FnMut(Piece)) -> Result<Answer> {
    ensure!(ai.ready(), "no key is typed, so there is nothing to ask with");
    ensure!(!history.is_empty(), "there is nothing to ask");
    let asked = body(ai, history, title);
    let mut response = agent
        .post(&ai.url())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {}", ai.api_key.trim()))
        .send(asked.as_bytes())
        .context("ask the server")?;
    let status = response.status();
    if !status.is_success() {
        let mut said = String::new();
        let _ = response.body_mut().with_config().limit(1 << 16).reader().read_to_string(&mut said);
        bail!("{}", refused(status.as_u16(), &said));
    }
    let body = response.body_mut().with_config().limit(u64::MAX).reader();
    let mut answer = Answer::default();
    for line in BufReader::new(body).lines() {
        // Stopping keeps what came of the answer: the page shows the words
        // that arrived, and asks afresh for the rest.
        if stop.load(Ordering::Relaxed) {
            return Ok(answer);
        }
        let line = line.context("read the answer")?;
        match read(&line)? {
            Line::Nothing => {}
            Line::Done => {
                on_piece(Piece::Done);
                return Ok(answer);
            }
            Line::Chunk(chunk) => {
                for piece in pieces(&chunk) {
                    match &piece {
                        Piece::Content(words) => answer.content.push_str(words),
                        Piece::Reasoning(words) => answer.reasoning.push_str(words),
                        _ => {}
                    }
                    on_piece(piece);
                }
                if let Some(choice) = chunk.choices.first() {
                    if let Some(calls) = &choice.delta.server_tool_calls {
                        answer.calls = Some(calls.clone());
                    }
                    if let Some(name) = &choice.delta.title {
                        answer.title = Some(name.clone());
                    }
                    if let Some(notes) = &choice.delta.annotations {
                        answer.citations.extend(notes.iter().filter_map(Annotation::citation));
                    }
                }
            }
        }
    }
    bail!("the server stopped before the answer was whole")
}

/// Work stopped because it was asked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stopped;

impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("stopped as asked")
    }
}

impl std::error::Error for Stopped {}

#[cfg(test)]
mod tests {
    use super::*;

    fn asking(text: &str) -> Value {
        serde_json::from_str(&body(&Ai::default(), &[Message::user(text)], false)).unwrap()
    }

    #[test]
    fn the_request_names_the_model_and_asks_for_it_as_it_is_written() {
        let asked = asking("hi");
        assert_eq!(asked["model"], "deepseek-flash");
        assert_eq!(asked["stream"], true);
        assert_eq!(asked["messages"][0]["role"], "user");
        assert_eq!(asked["thinking"]["type"], "disabled");
        let thinking: Value = serde_json::from_str(&body(&Ai { thinking: true, ..Ai::default() }, &[Message::user("hi")], false)).unwrap();
        assert_eq!(thinking["thinking"]["type"], "enabled");
    }

    #[test]
    fn the_tools_the_model_may_take_to_the_web_are_declared() {
        let asked = asking("hi");
        assert_eq!(asked["tools"], serde_json::json!([{ "type": "deepseek_web_search" }, { "type": "deepseek_read_url" }]));
        let read_only: Value = serde_json::from_str(&body(&Ai { web_search: false, ..Ai::default() }, &[Message::user("hi")], false)).unwrap();
        assert_eq!(read_only["tools"], serde_json::json!([{ "type": "deepseek_read_url" }]), "a link is still read without a search");
        let plain: Value = serde_json::from_str(&body(&Ai { web_search: false, read_links: false, ..Ai::default() }, &[Message::user("hi")], false)).unwrap();
        assert!(plain.get("tools").is_none(), "nothing to declare, nothing sent");
    }

    #[test]
    fn a_title_is_asked_for_once() {
        let titled: Value = serde_json::from_str(&body(&Ai::default(), &[Message::user("hi")], true)).unwrap();
        assert_eq!(titled["title"]["enabled"], true);
        assert!(asking("hi").get("title").is_none(), "a conversation keeps the title it was given");
    }

    #[test]
    fn each_chunk_gives_its_words() {
        let content = r#"data: {"id":"1","object":"chat.completion.chunk","model":"deepseek-flash","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}],"usage":null}"#;
        let Line::Chunk(chunk) = read(content).unwrap() else { panic!("a chunk") };
        assert_eq!(pieces(&chunk), vec![Piece::Content("Hello".into())]);
        let reasoning = r#"data: {"choices":[{"index":0,"delta":{"reasoning_content":"thinking it over"},"finish_reason":null}]}"#;
        let Line::Chunk(chunk) = read(reasoning).unwrap() else { panic!("a chunk") };
        assert_eq!(pieces(&chunk), vec![Piece::Reasoning("thinking it over".into())]);
        let empty = r#"data: {"choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#;
        let Line::Chunk(chunk) = read(empty).unwrap() else { panic!("a chunk") };
        assert!(pieces(&chunk).is_empty(), "a chunk that says nothing new is not shown");
        let last = r#"data: {"choices":[{"index":0,"delta":{"content":"","title":"北京明天天气"},"finish_reason":"stop"}],"usage":{"total_tokens":18}}"#;
        let Line::Chunk(chunk) = read(last).unwrap() else { panic!("a chunk") };
        assert!(pieces(&chunk).is_empty(), "the count of tokens is not words");
        assert!(matches!(read("data: [DONE]").unwrap(), Line::Done));
        assert!(matches!(read("").unwrap(), Line::Nothing));
        assert!(matches!(read(": keep-alive").unwrap(), Line::Nothing));
    }

    #[test]
    fn a_search_shows_its_words_and_the_pages_it_found() {
        let began = r#"data: {"choices":[{"index":0,"delta":{},"server_tool_event":{"type":"call_started","step":0,"call":{"index":0,"id":"s1","type":"deepseek_web_search","kind":"search","action":{"type":"search","queries":["Iceland population 2024","Iceland population 2025"]},"status":"in_progress"}}}]}"#;
        let Line::Chunk(chunk) = read(began).unwrap() else { panic!("a chunk") };
        assert_eq!(pieces(&chunk), vec![Piece::Searched(vec!["Iceland population 2024".into(), "Iceland population 2025".into()])]);
        let ended = r#"data: {"choices":[{"index":0,"delta":{},"server_tool_event":{"type":"call_completed","step":0,"call":{"index":0,"id":"s1","type":"deepseek_web_search","kind":"search","action":{"type":"search","queries":["Iceland population 2024"]},"status":"completed","sources":[{"type":"url","url":"https://www.huaon.com/a.html","title":"2024年冰岛人口","snippet":"...","site_name":"华经情报网"}]}}}]}"#;
        let Line::Chunk(chunk) = read(ended).unwrap() else { panic!("a chunk") };
        assert_eq!(pieces(&chunk), vec![Piece::Found { title: Some("2024年冰岛人口".into()), url: "https://www.huaon.com/a.html".into() }]);
    }

    #[test]
    fn a_page_opened_or_a_link_read_shows_its_address() {
        let opened = r#"data: {"choices":[{"index":0,"delta":{},"server_tool_event":{"type":"call_started","step":1,"call":{"index":0,"id":"s2","type":"deepseek_web_search","kind":"deep_search","action":{"type":"open_page","url":"https://en.tokyotower.co.jp/company/"},"status":"in_progress"}}}]}"#;
        let Line::Chunk(chunk) = read(opened).unwrap() else { panic!("a chunk") };
        assert_eq!(pieces(&chunk), vec![Piece::Opened("https://en.tokyotower.co.jp/company/".into())]);
        let read_link = r#"data: {"choices":[{"index":0,"delta":{},"server_tool_event":{"type":"call_started","step":0,"call":{"index":0,"id":"r1","type":"deepseek_read_url","kind":"read_link","action":{"type":"read_link","url":"https://example.com"},"status":"in_progress"}}}]}"#;
        let Line::Chunk(chunk) = read(read_link).unwrap() else { panic!("a chunk") };
        assert_eq!(pieces(&chunk), vec![Piece::Opened("https://example.com".into())]);
    }

    #[test]
    fn a_step_that_fails_says_why() {
        let failed = r#"data: {"choices":[{"index":0,"delta":{},"server_tool_event":{"type":"call_completed","step":0,"call":{"index":0,"id":"r1","type":"deepseek_read_url","kind":"read_link","action":{"type":"read_link","url":"https://example.com"},"status":"failed","error":{"code":"url_not_allowed"}}}}]}"#;
        let Line::Chunk(chunk) = read(failed).unwrap() else { panic!("a chunk") };
        assert_eq!(pieces(&chunk), vec![Piece::Failed("url_not_allowed".into())]);
    }

    #[test]
    fn the_pages_the_words_came_from_are_kept() {
        let ended = r#"data: {"choices":[{"index":0,"delta":{"content":"","annotations":[{"type":"url_citation","url_citation":{"url":"https://example.com/a","title":"A","start_index":0,"end_index":10}}]},"finish_reason":"stop"}]}"#;
        let Line::Chunk(chunk) = read(ended).unwrap() else { panic!("a chunk") };
        let notes: Vec<Citation> = chunk.choices[0].delta.annotations.as_ref().unwrap().iter().filter_map(Annotation::citation).collect();
        assert_eq!(notes, vec![Citation { url: "https://example.com/a".into(), title: Some("A".into()), start: 0, end: 10 }]);
        assert_eq!(Message::answered(&Answer { content: "hi".into(), calls: None, ..Answer::default() }), Message::assistant("hi"));
    }

    #[test]
    fn an_error_in_the_stream_is_the_answer() {
        let error = r#"data: {"error":{"message":"Authentication Fails","type":"authentication_error"}}"#;
        let why = read(error).unwrap_err().to_string();
        assert!(why.contains("Authentication Fails") && why.contains("authentication_error"), "{why}");
    }

    #[test]
    fn what_a_refused_request_says_is_read() {
        assert_eq!(refused(401, r#"{"error":{"message":"Authentication Fails","type":"authentication_error"}}"#), "the server answered 401: Authentication Fails (authentication_error)");
        assert_eq!(refused(502, "  "), "the server answered 502");
        assert_eq!(refused(500, "oops"), "the server answered 500: oops");
    }

    #[test]
    fn a_chat_with_nothing_to_ask_or_no_key_is_not_sent() {
        let agent = ureq::Agent::config_builder().build().new_agent();
        let mut nowhere = |_: Piece| {};
        let ai = Ai::default();
        assert!(stream(&agent, &ai, &[Message::user("hi")], false, &AtomicBool::new(false), &mut nowhere).is_err());
        let keyed = Ai { api_key: "sk-x".into(), ..Ai::default() };
        assert!(stream(&agent, &keyed, &[], false, &AtomicBool::new(false), &mut nowhere).is_err());
    }
}
