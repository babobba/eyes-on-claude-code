use super::{NotificationPriority, NotificationSink, SessionNotification};

pub struct PushoverSink {
    user_key: String,
    app_token: String,
    device: Option<String>,
}

impl PushoverSink {
    pub fn new(user_key: String, app_token: String, device: Option<String>) -> Self {
        Self {
            user_key,
            app_token,
            device,
        }
    }

    fn pushover_priority(priority: &NotificationPriority) -> &'static str {
        match priority {
            NotificationPriority::Low => "-1",
            NotificationPriority::Normal => "0",
            NotificationPriority::High => "1",
        }
    }
}

impl NotificationSink for PushoverSink {
    fn name(&self) -> &str {
        "pushover"
    }

    fn send(&self, notification: &SessionNotification) -> Result<(), String> {
        let mut form = format!(
            "token={}&user={}&title={}&message={}&priority={}",
            urlencoded(&self.app_token),
            urlencoded(&self.user_key),
            urlencoded(&notification.title()),
            urlencoded(&notification.body()),
            Self::pushover_priority(&notification.priority),
        );

        if let Some(ref device) = self.device {
            form.push_str(&format!("&device={}", urlencoded(device)));
        }

        if let Some(ref url) = notification.click_url {
            form.push_str(&format!("&url={}", urlencoded(url)));
            form.push_str(&format!("&url_title={}", urlencoded("Open in EOCC")));
        }

        ureq::post("https://api.pushover.net/1/messages.json")
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_string(&form)
            .map_err(|e| format!("pushover request failed: {}", e))?;

        Ok(())
    }
}

fn urlencoded(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}
