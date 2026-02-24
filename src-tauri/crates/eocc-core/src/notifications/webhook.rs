use super::{NotificationSink, SessionNotification};
use serde::Serialize;

pub struct WebhookSink {
    url: String,
}

#[derive(Serialize)]
struct WebhookPayload {
    text: String,
    project_name: String,
    project_dir: String,
    status: String,
    priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

impl WebhookSink {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl NotificationSink for WebhookSink {
    fn name(&self) -> &str {
        "webhook"
    }

    fn send(&self, notification: &SessionNotification) -> Result<(), String> {
        let payload = WebhookPayload {
            text: format!("{}\n{}", notification.title(), notification.body()),
            project_name: notification.project_name.clone(),
            project_dir: notification.project_dir.clone(),
            status: format!("{:?}", notification.new_status),
            priority: notification.priority.to_string(),
            url: notification.click_url.clone(),
        };

        let body =
            serde_json::to_string(&payload).map_err(|e| format!("serialize failed: {}", e))?;

        ureq::post(&self.url)
            .set("Content-Type", "application/json")
            .send_string(&body)
            .map_err(|e| format!("webhook request failed: {}", e))?;

        Ok(())
    }
}
