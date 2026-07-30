use crate::state::AgentInfo;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn notify_started(app: &AppHandle, agent: &AgentInfo) {
    let body = format!(
        "opencode agent started — {}",
        agent.working_dir
    );

    let _ = app.notification()
        .builder()
        .title("Agent Started")
        .body(&body)
        .show();
}

pub fn notify_stopped(app: &AppHandle, agent: &AgentInfo) {
    let body = format!(
        "opencode agent finished — {}",
        agent.working_dir
    );

    let _ = app.notification()
        .builder()
        .title("Agent Stopped")
        .body(&body)
        .show();
}
