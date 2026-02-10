use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::Subscriber;
use tracing_subscriber::Layer;

const MAX_LOG_ENTRIES: usize = 1000;
const MAX_IMPORTANT_LOG_ENTRIES: usize = 10_000;
const IMPORTANT_KEYWORDS: [&str; 11] = [
    "slow tick",
    "tick loop overrun",
    "slow auto-save cycle",
    "auto-saved",
    "slow handler",
    "panic",
    "timed out",
    "timeout",
    "backpressure",
    "lag",
    "[perf]",
];

#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Clone)]
pub struct LogBuffer {
    entries: Arc<Mutex<VecDeque<LogEntry>>>,
    important_entries: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES))),
            important_entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_IMPORTANT_LOG_ENTRIES))),
        }
    }

    fn push(&self, entry: LogEntry) {
        {
            let mut entries = self.entries.lock().unwrap();
            Self::push_with_limit(&mut entries, entry.clone(), MAX_LOG_ENTRIES);
        }

        if Self::is_important(&entry.level, &entry.message) {
            let mut important_entries = self.important_entries.lock().unwrap();
            Self::push_with_limit(&mut important_entries, entry, MAX_IMPORTANT_LOG_ENTRIES);
        }
    }

    pub fn recent(&self, count: usize) -> Vec<LogEntry> {
        let entries = self.entries.lock().unwrap();
        let skip = entries.len().saturating_sub(count);
        entries.iter().skip(skip).cloned().collect()
    }

    pub fn recent_important(&self, count: usize) -> Vec<LogEntry> {
        let entries = self.important_entries.lock().unwrap();
        let skip = entries.len().saturating_sub(count);
        entries.iter().skip(skip).cloned().collect()
    }

    fn push_with_limit(entries: &mut VecDeque<LogEntry>, entry: LogEntry, limit: usize) {
        if entries.len() >= limit {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    fn is_important(level: &str, message: &str) -> bool {
        if matches!(level, "ERROR" | "WARN") {
            return true;
        }

        let message = message.to_ascii_lowercase();
        IMPORTANT_KEYWORDS
            .iter()
            .any(|keyword| message.contains(keyword))
    }
}

pub struct LogBufferLayer {
    buffer: LogBuffer,
}

impl LogBufferLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl<S: Subscriber> Layer<S> for LogBufferLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let timestamp = chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S%.3fZ")
            .to_string();

        let message = if let Some(target) = metadata.target().strip_prefix("isometric_server::") {
            format!("[{}] {}", target, visitor.message)
        } else {
            visitor.message
        };

        self.buffer.push(LogEntry {
            timestamp,
            level,
            message,
        });
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else if self.message.is_empty() {
            self.message = format!("{}={:?}", field.name(), value);
        } else {
            self.message
                .push_str(&format!(" {}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else if self.message.is_empty() {
            self.message = format!("{}={}", field.name(), value);
        } else {
            self.message
                .push_str(&format!(" {}={}", field.name(), value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LogBuffer, LogEntry};

    #[test]
    fn important_view_keeps_warn_error_and_key_performance_logs() {
        let buffer = LogBuffer::new();
        let base_ts = "2026-02-10 00:00:00.000Z".to_string();

        buffer.push(LogEntry {
            timestamp: base_ts.clone(),
            level: "INFO".to_string(),
            message: "Regular player update".to_string(),
        });
        buffer.push(LogEntry {
            timestamp: base_ts.clone(),
            level: "INFO".to_string(),
            message: "Slow tick: 75ms".to_string(),
        });
        buffer.push(LogEntry {
            timestamp: base_ts.clone(),
            level: "WARN".to_string(),
            message: "Inventory desync for player".to_string(),
        });
        buffer.push(LogEntry {
            timestamp: base_ts,
            level: "ERROR".to_string(),
            message: "DB write failed".to_string(),
        });

        let important = buffer.recent_important(10);
        assert_eq!(important.len(), 3);
        assert!(important.iter().any(|e| e.message.contains("Slow tick")));
        assert!(important.iter().any(|e| e.level == "WARN"));
        assert!(important.iter().any(|e| e.level == "ERROR"));
    }
}
