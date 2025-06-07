use mac_notification_sys::*;

pub fn show_notification(title: &str, body: &str) {
    // Notification::new()
    // .summary(title)
    // .body(body)
    // .appname("cwlogs-viewer")
    // .show();

    send_notification(title, Some("cwlogs-viewer"), body, None).expect("can't show notification");
}
