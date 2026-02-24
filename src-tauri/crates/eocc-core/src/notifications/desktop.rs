use super::{NotificationSink, SessionNotification};

#[derive(Default)]
pub struct DesktopSink;

impl DesktopSink {
    pub fn new() -> Self {
        Self
    }
}

impl NotificationSink for DesktopSink {
    fn name(&self) -> &str {
        "desktop"
    }

    fn send(&self, notification: &SessionNotification) -> Result<(), String> {
        notify_rust::Notification::new()
            .summary(&notification.title())
            .body(&notification.body())
            .show()
            .map_err(|e| format!("desktop notification failed: {}", e))?;
        Ok(())
    }
}
