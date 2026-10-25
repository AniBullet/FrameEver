use std::process::Stdio;

use clap::{Args, Subcommand};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Args)]
pub struct McpArgs {
    #[command(subcommand)]
    command: McpCommands,
}

#[derive(Subcommand)]
enum McpCommands {
    Serve,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RecordStartInput {
    #[serde(default)]
    screen: Option<String>,
    #[serde(default)]
    window: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    camera: Option<String>,
    #[serde(default)]
    mic: Option<String>,
    #[serde(default)]
    system_audio: bool,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RecordingIdInput {
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ProjectInput {
    path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExportInput {
    path: String,
    output: String,
    #[serde(default)]
    format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScreenshotInput {
    #[serde(default)]
    screen: Option<String>,
    #[serde(default)]
    window: Option<String>,
    path: String,
}

#[derive(Clone)]
struct LocalMcpServer {
    tool_router: ToolRouter<Self>,
}

fn record_start_args(input: RecordStartInput) -> Vec<String> {
    let mut args = vec![
        "record".to_string(),
        "start".to_string(),
        "--detach".to_string(),
    ];
    let flags = [
        ("--screen", input.screen),
        ("--window", input.window),
        ("--mode", input.mode),
        ("--camera", input.camera),
        ("--mic", input.mic),
        ("--path", input.path),
    ];
    for (flag, value) in flags {
        if let Some(value) = value {
            args.push(flag.to_string());
            args.push(value);
        }
    }
    if input.system_audio {
        args.push("--system-audio".to_string());
    }
    args
}

fn screenshot_args(input: ScreenshotInput) -> Vec<String> {
    let mut args = vec!["screenshot".to_string()];
    if let Some(screen) = input.screen {
        args.extend(["--screen".to_string(), screen]);
    }
    if let Some(window) = input.window {
        args.extend(["--window".to_string(), window]);
    }
    args.extend(["--path".to_string(), input.path]);
    args
}

fn export_args(input: ExportInput) -> Vec<String> {
    let mut args = vec![
        "export".to_string(),
        input.path,
        "--output".to_string(),
        input.output,
    ];
    if let Some(format) = input.format {
        args.extend(["--format".to_string(), format]);
    }
    args
}

fn redact_window_titles(mut value: Value) -> Value {
    if let Some(windows) = value.get_mut("windows").and_then(Value::as_array_mut) {
        for window in windows {
            if let Some(window) = window.as_object_mut() {
                window.remove("name");
            }
        }
    }
    value
}

fn last_json_line(stdout: &[u8]) -> Option<Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| serde_json::from_str(line).ok())
}

async fn run_cli(args: Vec<String>) -> Result<Value, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let output = tokio::process::Command::new(executable)
        .arg("--json")
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| format!("Could not run frameever: {error}"))?;
    let value = last_json_line(&output.stdout);
    if output.status.success() {
        return value.ok_or_else(|| "The command produced no JSON result".to_string());
    }
    Err(value
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| String::from_utf8_lossy(&output.stderr).trim().to_string()))
}

fn tool_result(result: Result<Value, String>) -> CallToolResult {
    match result {
        Ok(value) => CallToolResult::structured(value),
        Err(error) => CallToolResult::structured_error(json!({ "error": error })),
    }
}

impl LocalMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl LocalMcpServer {
    #[tool(
        name = "doctor",
        description = "Report capture readiness, permissions and environment diagnostics",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn doctor(&self) -> CallToolResult {
        tool_result(run_cli(vec!["doctor".to_string()]).await)
    }

    #[tool(
        name = "targets",
        description = "List screens, windows, cameras and microphones that can be recorded. Window titles are withheld; windows are identified by id and app name",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn targets(&self) -> CallToolResult {
        tool_result(
            run_cli(vec!["targets".to_string()])
                .await
                .map(redact_window_titles),
        )
    }

    #[tool(
        name = "recordings_list",
        description = "List local recordings with their paths and status",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn recordings_list(&self) -> CallToolResult {
        tool_result(run_cli(vec!["recordings".to_string(), "list".to_string()]).await)
    }

    #[tool(
        name = "record_start",
        description = "Start a detached recording of a screen or window. Returns the recording id, process id and project path",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn record_start(
        &self,
        Parameters(input): Parameters<RecordStartInput>,
    ) -> CallToolResult {
        tool_result(run_cli(record_start_args(input)).await)
    }

    #[tool(
        name = "record_stop",
        description = "Stop a detached recording by id and finalize its project",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn record_stop(&self, Parameters(input): Parameters<RecordingIdInput>) -> CallToolResult {
        tool_result(
            run_cli(vec![
                "record".to_string(),
                "stop".to_string(),
                "--id".to_string(),
                input.id,
            ])
            .await,
        )
    }

    #[tool(
        name = "record_status",
        description = "Report whether detached recordings are running",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn record_status(&self) -> CallToolResult {
        tool_result(run_cli(vec!["record".to_string(), "status".to_string()]).await)
    }

    #[tool(
        name = "project_validate",
        description = "Check that a .ever recording project is complete",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn project_validate(
        &self,
        Parameters(input): Parameters<ProjectInput>,
    ) -> CallToolResult {
        tool_result(
            run_cli(vec![
                "project".to_string(),
                "validate".to_string(),
                input.path,
            ])
            .await,
        )
    }

    #[tool(
        name = "export",
        description = "Export a .ever project to a local video file and return the output path",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn export(&self, Parameters(input): Parameters<ExportInput>) -> CallToolResult {
        tool_result(run_cli(export_args(input)).await)
    }

    #[tool(
        name = "screenshot",
        description = "Save a screenshot of a screen or window to a local file and return its path and size, never the image itself",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn screenshot(&self, Parameters(input): Parameters<ScreenshotInput>) -> CallToolResult {
        tool_result(run_cli(screenshot_args(input)).await)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for LocalMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("frameever", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Local recording control only. Tools return paths, ids, status and device names; they never return recordings, screenshots, audio, transcripts or window titles, and nothing is sent off this computer.",
            )
    }
}

impl McpArgs {
    pub async fn run(self) -> Result<(), String> {
        match self.command {
            McpCommands::Serve => {
                let service = LocalMcpServer::new()
                    .serve(rmcp::transport::stdio())
                    .await
                    .map_err(|error| error.to_string())?;
                service.waiting().await.map_err(|error| error.to_string())?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_titles_are_withheld_from_targets() {
        let redacted = redact_window_titles(json!({
            "screens": [{ "id": "1", "name": "Display 1" }],
            "windows": [{ "id": "7", "name": "Q3 plan.xlsx", "ownerName": "Excel" }]
        }));
        assert_eq!(redacted["windows"][0].get("name"), None);
        assert_eq!(redacted["windows"][0]["ownerName"], "Excel");
        assert_eq!(redacted["screens"][0]["name"], "Display 1");
    }

    #[test]
    fn record_start_always_detaches_and_maps_options() {
        let args = record_start_args(RecordStartInput {
            screen: Some("1".to_string()),
            window: None,
            mode: Some("instant".to_string()),
            camera: None,
            mic: Some("Mic".to_string()),
            system_audio: true,
            path: None,
        });
        assert_eq!(
            args,
            [
                "record",
                "start",
                "--detach",
                "--screen",
                "1",
                "--mode",
                "instant",
                "--mic",
                "Mic",
                "--system-audio"
            ]
        );
    }

    #[test]
    fn the_last_json_line_is_the_result() {
        let value = last_json_line(
            b"{\"event\":\"Progress\"}\n{\"event\":\"Completed\",\"path\":\"a.mp4\"}\n\n",
        );
        assert_eq!(value.unwrap()["path"], "a.mp4");
        assert_eq!(last_json_line(b"not json"), None);
    }
}
