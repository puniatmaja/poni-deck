use crate::state::AgentInfo;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn notify_started(app: &AppHandle, agent: &AgentInfo) {
    let tool = if agent.tool == "claude" { "claude" } else { "opencode" };
    let body = format!(
        "{tool} agent started — {}",
        agent.working_dir
    );

    let _ = app.notification()
        .builder()
        .title("Agent Started")
        .body(&body)
        .show();
}

pub fn notify_stopped(app: &AppHandle, agent: &AgentInfo) {
    let tool = if agent.tool == "claude" { "claude" } else { "opencode" };
    let body = format!(
        "{tool} agent finished — {}",
        agent.working_dir
    );

    let _ = app.notification()
        .builder()
        .title("Agent Stopped")
        .body(&body)
        .show();
}
