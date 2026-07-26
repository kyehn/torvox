//! Prompt-matching input queue for AI agents (inspired by Haven #161).
//!
//! Watches scrollback for a configurable prompt pattern and injects
//! queued text when the pattern appears.
//!
//! # Requirements
//! - FR-048 — Input queue mechanism

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::types::SessionStore;

/// Prompt-matching input queue for AI agents.
///
/// Watches scrollback for a configurable prompt pattern and injects
/// queued text when the pattern appears. Useful for driving interactive
/// REPLs, scripts with prompts, or automated testing. The pattern is matched
/// as a substring of the recent scrollback tail.
#[derive(Clone)]
pub struct InputQueue {
    entries: Arc<Mutex<HashMap<String, QueuedEntry>>>,
}

#[derive(Clone)]
struct QueuedEntry {
    /// Unique identifier for this queued entry.
    entry_id: String,
    /// Session ID to monitor for the prompt pattern.
    session_id: u32,
    /// Text to inject when the prompt pattern matches.
    text: String,
    /// Key sequence to send after the text (e.g., Enter).
    submit_key: String,
    /// Substring pattern to watch for in scrollback output.
    prompt_pattern: String,
    /// Instant after which this entry expires.
    deadline: Instant,
}

impl Default for InputQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl InputQueue {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn enqueue(
        &self,
        session_id: u32,
        text: String,
        submit_key: String,
        prompt_pattern: String,
        timeout_seconds: u32,
    ) -> String {
        let entry_id = format!(
            "q-{}-{}",
            session_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let entry = QueuedEntry {
            entry_id: entry_id.clone(),
            session_id,
            text,
            submit_key,
            prompt_pattern,
            deadline: Instant::now() + Duration::from_secs(timeout_seconds.into()),
        };
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(entry_id.clone(), entry);
        entry_id
    }

    pub fn cancel(&self, entry_id: &str) -> bool {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(entry_id)
            .is_some()
    }

    pub fn pending(&self) -> Vec<Value> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .map(|entry| {
                json!({
                    "entry_id": entry.entry_id,
                    "session_id": entry.session_id,
                    "text_preview": if entry.text.len() > 40 { format!("{}…", &entry.text[..37]) } else { entry.text.clone() },
                    "prompt_pattern": entry.prompt_pattern,
                    "seconds_remaining": entry.deadline.saturating_duration_since(Instant::now()).as_secs(),
                })
            })
            .collect()
    }

    pub fn check_and_deliver(&self, store: &Arc<dyn SessionStore>, write_consent: bool) {
        if !write_consent {
            return;
        }
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (entry_id, entry) in self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            if now > entry.deadline {
                to_remove.push(entry_id.clone());
                continue;
            }

            let scrollback = match store.read_scrollback_tail(entry.session_id, 20) {
                Ok(lines) => lines.join("\n"),
                Err(_) => continue,
            };

            if scrollback.contains(&entry.prompt_pattern) {
                let data = format!("{}{}", entry.text, entry.submit_key);
                if let Err(error) = store.write(entry.session_id, data.into_bytes()) {
                    log::error!(
                        "mcp: failed to write shell entry for session {}: {}",
                        entry.session_id,
                        error
                    );
                }
                to_remove.push(entry_id.clone());
            }
        }

        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        for entry_id in to_remove {
            entries.remove(&entry_id);
        }
    }
}
