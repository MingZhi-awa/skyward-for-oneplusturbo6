//! Asking the model what is typed on the chat page, and what the key's
//! account holds: on a thread of its own that tells each piece of the answer
//! as it comes, so that the first words show while the rest is still being
//! written.

use anyhow::Result;
use skyward_ai::{Ai, Answer, Balance, Message, Piece};
use skyward_net::http::{self, Servers};
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub enum Task {
    /// Ask `ai` to continue `history`, with a title for the conversation
    /// where `title` says so: a conversation's first question.
    Ask { ai: Ai, history: Vec<Message>, title: bool },
    /// Ask what the account the key belongs to holds.
    Balance(Ai),
}

pub enum Event {
    /// A piece of the answer, as it came.
    Piece(Piece),
    /// The answer is whole, or why it did not come: what it came to.
    Answered(Result<Box<Answer>, String>),
    /// What the account holds, as the server said, or why it could not be
    /// asked.
    Balance(Result<Balance, String>),
}

pub struct Worker {
    /// Whether stopping was asked for: the answer is let go at the next
    /// piece.
    stop: Arc<AtomicBool>,
    /// The connection in use, which stopping cuts at once: a piece may be
    /// a long time coming.
    agent: Arc<Mutex<Option<ureq::Agent>>>,
    worker: skyward_sys::worker::Worker<Event>,
}

impl Worker {
    pub fn start(task: Task, servers: Servers) -> Result<Worker> {
        let stop = Arc::new(AtomicBool::new(false));
        let agent: Arc<Mutex<Option<ureq::Agent>>> = Arc::new(Mutex::new(None));
        let (stopping, using) = (stop.clone(), agent.clone());
        let worker = skyward_sys::worker::Worker::start("ai", move |teller| {
            let tell = move |event: Event| {
                teller.tell(event);
            };
            let a = http::agent(servers);
            *using.lock().expect("the agent is not held by a thread that panicked") = Some(a.clone());
            match task {
                Task::Ask { ai, history, title } => {
                    let answer = {
                        let mut piece = |p: Piece| tell(Event::Piece(p));
                        skyward_ai::stream(&a, &ai, &history, title, &stopping, &mut piece).map(Box::new).map_err(|e| format!("{e:#}"))
                    };
                    tell(Event::Answered(answer));
                }
                Task::Balance(ai) => {
                    let said = skyward_ai::balance(&a, &ai).map_err(|e| format!("{e:#}"));
                    tell(Event::Balance(said));
                }
            }
            if let Some(a) = using.lock().expect("the agent is not held by a thread that panicked").take() {
                a.shutdown();
            }
        })?;
        Ok(Worker { stop, agent, worker })
    }

    /// Readable while events wait.
    pub fn fd(&self) -> RawFd {
        self.worker.fd()
    }

    /// The events since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.worker.events()
    }

    /// Whether the work has ended.
    pub fn ended(&self) -> bool {
        self.worker.ended()
    }

    /// Stop what is being asked, cutting the connection where one is open:
    /// the page keeps what came of the answer.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(a) = self.agent.lock().expect("the agent is not held by a thread that panicked").take() {
            a.shutdown();
        }
    }
}
