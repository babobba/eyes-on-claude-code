use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRecord {
    pub timestamp: String,
    pub project_name: String,
    pub project_dir: String,
    pub status: String,
    pub channels: Vec<ChannelResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResult {
    pub name: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct NotificationHistory {
    records: VecDeque<NotificationRecord>,
}

impl NotificationHistory {
    pub fn push(&mut self, record: NotificationRecord) {
        self.records.push_back(record);
        while self.records.len() > MAX_HISTORY {
            self.records.pop_front();
        }
    }

    pub fn records(&self) -> Vec<NotificationRecord> {
        self.records.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(project: &str) -> NotificationRecord {
        NotificationRecord {
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            project_name: project.to_string(),
            project_dir: format!("/home/user/{}", project),
            status: "WaitingPermission".to_string(),
            channels: vec![ChannelResult {
                name: "ntfy".to_string(),
                success: true,
                error: None,
            }],
        }
    }

    #[test]
    fn push_and_retrieve() {
        let mut history = NotificationHistory::default();
        history.push(make_record("proj"));
        let records = history.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].project_name, "proj");
    }

    #[test]
    fn caps_at_max() {
        let mut history = NotificationHistory::default();
        for i in 0..150 {
            history.push(make_record(&format!("proj-{}", i)));
        }
        assert_eq!(history.records().len(), MAX_HISTORY);
        assert_eq!(history.records()[0].project_name, "proj-50");
    }

    #[test]
    fn clear_empties() {
        let mut history = NotificationHistory::default();
        history.push(make_record("proj"));
        history.clear();
        assert!(history.records().is_empty());
    }
}
