use super::{NotificationPriority, NotificationSink, SessionNotification};

pub struct NtfySink {
    server: String,
    topic: String,
    token: Option<String>,
}

impl NtfySink {
    pub fn new(server: String, topic: String, token: Option<String>) -> Self {
        Self {
            server,
            topic,
            token,
        }
    }

    fn url(&self) -> String {
        let server = self.server.trim_end_matches('/');
        format!("{}/{}", server, self.topic)
    }

    fn ntfy_priority(priority: &NotificationPriority) -> &'static str {
        match priority {
            NotificationPriority::Low => "low",
            NotificationPriority::Normal => "default",
            NotificationPriority::High => "high",
        }
    }

    fn tags(notification: &SessionNotification) -> String {
        match notification.new_status {
            crate::state::SessionStatus::Active => "green_circle".to_string(),
            crate::state::SessionStatus::WaitingPermission => "lock".to_string(),
            crate::state::SessionStatus::WaitingInput => "hourglass".to_string(),
            crate::state::SessionStatus::Completed => "white_check_mark".to_string(),
        }
    }
}

impl NotificationSink for NtfySink {
    fn name(&self) -> &str {
        "ntfy"
    }

    fn send(&self, notification: &SessionNotification) -> Result<(), String> {
        let mut request = ureq::post(&self.url())
            .set("Title", &notification.title())
            .set("Priority", Self::ntfy_priority(&notification.priority))
            .set("Tags", &Self::tags(notification));

        if let Some(ref token) = self.token {
            request = request.set("Authorization", &format!("Bearer {}", token));
        }

        request
            .send_string(&notification.body())
            .map_err(|e| format!("ntfy request failed: {}", e))?;

        Ok(())
    }
}
