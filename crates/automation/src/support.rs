use std::collections::HashMap;
use std::time::Duration;

use crate::{TriggerContext, sanitize_filename_component, shell_command_line};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);

pub fn context_env(ctx: &TriggerContext) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if let Some(path) = &ctx.project_path {
        env.push(("FRAMEEVER_PROJECT_PATH", path.to_string_lossy().to_string()));
    }
    if let Some(path) = &ctx.image_path {
        env.push(("FRAMEEVER_IMAGE_PATH", path.to_string_lossy().to_string()));
    }
    if let Some(path) = &ctx.output_path {
        env.push(("FRAMEEVER_OUTPUT_PATH", path.to_string_lossy().to_string()));
    }
    if let Some(link) = &ctx.share_link {
        env.push(("FRAMEEVER_SHARE_LINK", link.clone()));
    }
    env
}

pub fn apply_filename_template(template: &str, ctx: &TriggerContext) -> String {
    let now = chrono::Local::now();
    let mut result = template.to_string();
    result = result.replace("{date}", &now.format("%Y-%m-%d").to_string());
    result = result.replace("{time}", &now.format("%H-%M-%S").to_string());
    result = result.replace("{datetime}", &now.format("%Y-%m-%d_%H-%M-%S").to_string());
    if let Some(title) = &ctx.window_title {
        result = result.replace("{window}", &sanitize_filename_component(title));
    }
    result
}

pub fn apply_body_template(template: &str, ctx: &TriggerContext) -> String {
    let mut result = template.to_string();
    if let Some(path) = &ctx.project_path {
        result = result.replace("{project_path}", &path.to_string_lossy());
    }
    if let Some(path) = &ctx.image_path {
        result = result.replace("{image_path}", &path.to_string_lossy());
    }
    if let Some(path) = &ctx.output_path {
        result = result.replace("{output_path}", &path.to_string_lossy());
    }
    if let Some(link) = &ctx.share_link {
        result = result.replace("{share_link}", link);
    }
    result
}

pub fn notification_text(
    ctx: &TriggerContext,
    title_template: &str,
    body_template: &str,
) -> (String, String) {
    let fill = |template: &str| apply_body_template(&apply_filename_template(template, ctx), ctx);
    (fill(title_template), fill(body_template))
}

pub async fn run_command(
    ctx: &TriggerContext,
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    env: &HashMap<String, String>,
    use_shell: bool,
) -> Result<(), String> {
    let mut cmd = if use_shell {
        let shell_line = shell_command_line(program, args);
        #[cfg(target_os = "windows")]
        let mut command = tokio::process::Command::new("cmd");
        #[cfg(target_os = "windows")]
        command.args(["/C", &shell_line]);
        #[cfg(not(target_os = "windows"))]
        let mut command = tokio::process::Command::new("sh");
        #[cfg(not(target_os = "windows"))]
        command.args(["-c", &shell_line]);
        command
    } else {
        let mut command = tokio::process::Command::new(program);
        command.args(args);
        command
    };

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (key, value) in env {
        cmd.env(key, value);
    }
    for (key, value) in context_env(ctx) {
        cmd.env(key, value);
    }

    cmd.kill_on_drop(true);
    let output = tokio::time::timeout(COMMAND_TIMEOUT, cmd.output())
        .await
        .map_err(|_| format!("Command timed out after {}s", COMMAND_TIMEOUT.as_secs()))?
        .map_err(|error| format!("Failed to run command: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Command exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
