//! `Canvas` frames: resolve the session's project on every call, apply the
//! kiosk file-tool policy, and hand the call to the canvas router (spine
//! AD-18, AD-21).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use parking_lot::Mutex;
use serde_json::Value;

use super::HostPlanServer;
use crate::acp::host_mcp::canvas::{CanvasError, CanvasErrorCode, CanvasOutcome};
use crate::acp::host_mcp::canvas_budget;
use crate::acp::host_mcp::canvas_policy::{self, CanvasRoute, ProjectFile};
use crate::acp::host_mcp::presence::CallOrigin;
use crate::acp::host_mcp::FrameReply;
use crate::canvas::persistence::{CallFailed, LateAnswer};

/// How long an export with a `path` that timed out may still finish and be
/// written.
pub(super) const LATE_EXPORT_LIMIT: Duration = Duration::from_secs(600);

/// The project a session works in: the router key and the folder exports
/// are written under (the session's working directory).
struct SessionScope {
    project_id: String,
    root: std::path::PathBuf,
}

impl HostPlanServer {
    pub(super) async fn canvas_call(
        &self,
        session_id: &str,
        agent_id: &str,
        tool: Option<String>,
        method: Option<String>,
        params: Option<String>,
    ) -> FrameReply {
        let tool = tool.unwrap_or_else(|| "-".to_string());
        let Some(method) = method else {
            return canvas_failure(&CanvasError::new(
                CanvasErrorCode::UnknownMethod,
                "a canvas frame needs a method",
            ));
        };
        let Some(scope) = self.scope_of(session_id) else {
            log::warn!(
                "[canvas-gateway] {tool} {method} refused: session {} has no project",
                crate::logging::redact_session_id(session_id)
            );
            return canvas_failure(&CanvasError::new(
                CanvasErrorCode::CanvasClosed,
                "this session belongs to no project, so it has no canvas",
            ));
        };
        let project_id = scope.project_id.as_str();
        let params = params.unwrap_or_else(|| "{}".to_string());
        // Params the gateway cannot parse cannot name a file the editor would
        // parse either; they go through for the editor to reject.
        let parsed = serde_json::from_str::<Value>(&params).ok();
        let route = parsed.as_ref().map_or(CanvasRoute::Forward, |parsed| {
            canvas_policy::route(&method, parsed)
        });
        let outcome = match route {
            CanvasRoute::Forward => {
                log::debug!("[canvas-gateway] {tool} {method} for project {project_id}");
                let timeout = self.call_budget(project_id, &method, parsed.as_ref()).await;
                let outcome = self
                    .canvas
                    .call_within(project_id, &method, params, timeout)
                    .await;
                // A batch that failed inside the editor says which command
                // failed but not which step of the batch it was, so an agent
                // debugging a large batch must bisect it by hand. Find the
                // failing step here and name it.
                match (&parsed, &outcome) {
                    (
                        Some(parsed),
                        Err(error)
                            if method == "engine.execute"
                                && error.code == CanvasErrorCode::EngineError
                                && is_plain_batch(parsed),
                    ) => self
                        .name_failing_batch_step(project_id, &tool, parsed, error, timeout)
                        .await,
                    _ => outcome,
                }
            }
            CanvasRoute::HostSave => {
                log::info!("[canvas-gateway] {tool} host save for project {project_id}");
                self.canvas
                    .call(project_id, "host.save", "{}".to_string())
                    .await
            }
            CanvasRoute::ExportTo { path, execute } => {
                self.export_to(&tool, &scope, &path, &execute).await
            }
            CanvasRoute::Refuse(error) => {
                log::info!(
                    "[canvas-gateway] {tool} {method} refused in kiosk for project {project_id}"
                );
                Err(error)
            }
        };
        match outcome {
            Ok(result) => {
                if let Some(parsed) = &parsed {
                    let origin = CallOrigin {
                        session_id,
                        agent_id,
                        project_id,
                    };
                    self.presence
                        .after_call(origin, &tool, &method, parsed, &result);
                }
                FrameReply::ok_with(Value::String(result))
            }
            Err(error) => canvas_failure(&error),
        }
    }

    /// Find which step of a failed `command.batch` failed and name it in the
    /// error. The engine error names the failing command, but a batch of
    /// dozens of steps repeats commands, and `$step` references make the
    /// failing step depend on the whole prefix, so which step failed is only
    /// findable by running prefixes. Each probe batch is the prefix plus a
    /// sentinel step that always fails (`command.batch` with `commands: null`
    /// is a parameter error the engine raises without touching the document),
    /// so every probe rolls back and the document never changes: verified
    /// against the pinned engine, where a probe leaves the page's children and
    /// the undo depth exactly as they were.
    ///
    /// Returns the original error untouched when no distinct failing step is
    /// found (transport errors, malformed batches, a failing step that is
    /// itself a nested `command.batch`), so a probe can never make the
    /// agent's answer worse.
    async fn name_failing_batch_step(
        &self,
        project_id: &str,
        tool: &str,
        params: &Value,
        error: &CanvasError,
        timeout: Duration,
    ) -> CanvasOutcome {
        let Some(steps) = params.get("commands").and_then(Value::as_array) else {
            return Err(error.clone());
        };
        // The sentinel's own error: a probe of just the sentinel gives it, and
        // any probe error different from it means a real step failed first.
        let Some(sentinel) = self
            .probe_batch(project_id, tool, &[], timeout)
            .await
            .err()
        else {
            // The sentinel no longer fails (an engine update): probing would
            // apply prefixes, so keep the original error.
            return Err(error.clone());
        };
        let sentinel_message = sentinel.message;
        let full = self
            .probe_batch(project_id, tool, steps, timeout)
            .await
            .err();
        let Some(full) = full.filter(|full| full.message != sentinel_message) else {
            // The whole batch's error is the sentinel's (or the batch stopped
            // failing): nothing to bisect.
            return Err(error.clone());
        };
        // Find the smallest prefix length k whose probe fails before the
        // sentinel: steps[..k] fails, steps[..k-1] does not, so step k-1
        // is the first failing step.
        let mut lo = 0; // probe(steps[..0] + sentinel) is the sentinel error.
        let mut hi = steps.len();
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            let failed = self
                .probe_batch(project_id, tool, &steps[..mid], timeout)
                .await
                .err()
                .is_some_and(|probe| probe.message != sentinel_message);
            if failed {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let Some(step) = steps.get(hi - 1) else {
            return Err(error.clone());
        };
        let command = step.get("command").and_then(Value::as_str).unwrap_or("?");
        let name = step.get("params").and_then(|p| p.get("name")).and_then(Value::as_str);
        let step_label = match name {
            Some(name) => format!("step {} (`{command}` \"{name}\")", hi - 1),
            None => format!("step {} (`{command}`)", hi - 1),
        };
        let full_message = full.message;
        let message = if full_message.starts_with("invalid parameters for `command.batch`: ") {
            // The step's own failure, already naming the command.
            format!("{step_label}: {}", strip_command(&full_message, command))
        } else {
            format!("{step_label}: {full_message}")
        };
        log::info!(
            "[canvas-gateway] {tool} command.batch for project {project_id} failed at {}",
            step_label
        );
        Err(CanvasError::new(error.code, message))
    }

    /// Run `steps` plus the sentinel as one `command.batch` execute. Every
    /// probe ends in a step that always fails, so the editor rolls it back.
    async fn probe_batch(
        &self,
        project_id: &str,
        tool: &str,
        steps: &[&Value],
        timeout: Duration,
    ) -> CanvasOutcome {
        let mut commands: Vec<Value> = steps.iter().map(|step| (*step).clone()).collect();
        commands.push(serde_json::json!({
            "command": "command.batch",
            "params": { "commands": null },
        }));
        let execute = serde_json::json!({ "command": "command.batch", "params": { "commands": commands } });
        let params = execute.to_string();
        log::debug!(
            "[canvas-gateway] {tool} command.batch probe of {} steps for project {project_id}",
            steps.len()
        );
        self.canvas
            .call_within(project_id, "engine.execute", params, timeout)
            .await
    }

    /// How long `method` may take ([`canvas_budget`]).
    async fn call_budget(
        &self,
        project_id: &str,
        method: &str,
        params: Option<&Value>,
    ) -> Duration {
        canvas_budget::call_budget(
            self.canvas.mounts(),
            project_id,
            method,
            params,
            self.canvas.call_timeout(),
        )
        .await
    }

    /// Export inline, then write the bytes to `path` inside the project. The
    /// path is checked before the editor runs, so a bad one costs no export,
    /// and again at the write. The reply is upstream's export-to-path result
    /// with the relative path. An export that outlasts its wait is still
    /// written when the editor finishes it ([`Self::write_late`]).
    async fn export_to(
        &self,
        tool: &str,
        scope: &SessionScope,
        path: &str,
        execute: &Value,
    ) -> CanvasOutcome {
        let project_id = scope.project_id.as_str();
        let file = canvas_policy::resolve_project_file(&scope.root, path).inspect_err(|_| {
            log::info!("[canvas-gateway] {tool} export path refused for project {project_id}");
        })?;
        let timeout = self
            .call_budget(project_id, "engine.execute", Some(execute))
            .await;
        let claim = self.exports.claim(&file.absolute);
        let result = match self
            .canvas
            .call_keeping_late(project_id, "engine.execute", execute.to_string(), timeout)
            .await
        {
            Ok(result) => result,
            Err(CallFailed {
                error,
                late: Some(late),
            }) => {
                let message = format!(
                    "{}. The export is still running: Termul writes it to `{}` if it finishes \
                     within {} minutes and no newer export to that path starts first",
                    error.message,
                    file.relative,
                    LATE_EXPORT_LIMIT.as_secs() / 60
                );
                self.write_late(tool, scope, path, file, late, claim);
                return Err(CanvasError::new(error.code, message));
            }
            Err(failed) => return Err(failed.error),
        };
        let (reply, bytes) = written_export(&result, &file)?;
        let size = bytes.len();
        let root = scope.root.clone();
        let requested = path.to_string();
        tokio::task::spawn_blocking(move || {
            canvas_policy::write_project_file(&root, &requested, &bytes)
        })
        .await
        .map_err(|error| engine_error(format!("the export write stopped: {error}")))?
        .inspect_err(|_| {
            log::info!("[canvas-gateway] {tool} export write refused for project {project_id}");
        })?;
        log::info!(
            "[canvas-gateway] {tool} wrote export {} ({size} bytes) for project {project_id}",
            file.relative
        );
        Ok(reply.to_string())
    }

    /// Wait for the late answer of an export that timed out and write it to
    /// `path`, unless a newer export to the same file started meanwhile.
    fn write_late(
        &self,
        tool: &str,
        scope: &SessionScope,
        path: &str,
        file: ProjectFile,
        late: LateAnswer,
        claim: ExportClaim,
    ) {
        let tool = tool.to_string();
        let project_id = scope.project_id.clone();
        let root = scope.root.clone();
        let requested = path.to_string();
        tokio::spawn(async move {
            let relative = file.relative.clone();
            let outcome = late.wait(LATE_EXPORT_LIMIT).await;
            let bytes = match outcome.map(|result| written_export(&result?, &file)) {
                Some(Ok((_, bytes))) => bytes,
                Some(Err(error)) => {
                    log::info!(
                        "[canvas-gateway] {tool} late export {relative} for project {project_id} failed: {} ({})",
                        error.code.as_str(),
                        error.message
                    );
                    return;
                }
                None => {
                    log::info!(
                        "[canvas-gateway] {tool} late export {relative} for project {project_id} never finished"
                    );
                    return;
                }
            };
            let size = bytes.len();
            let written = tokio::task::spawn_blocking(move || {
                claim.write_if_newest(|| {
                    canvas_policy::write_project_file(&root, &requested, &bytes)
                })
            })
            .await;
            match written {
                Ok(Some(Ok(_))) => log::info!(
                    "[canvas-gateway] {tool} wrote late export {relative} ({size} bytes) for project {project_id}"
                ),
                Ok(None) => log::info!(
                    "[canvas-gateway] {tool} late export {relative} for project {project_id} dropped: a newer export to it started"
                ),
                Ok(Some(Err(error))) => log::info!(
                    "[canvas-gateway] {tool} late export write {relative} refused for project {project_id}: {}",
                    error.message
                ),
                Err(error) => log::warn!(
                    "[canvas-gateway] {tool} late export write {relative} stopped for project {project_id}: {error}"
                ),
            }
        });
    }

    /// The session's project and working directory, read from the session
    /// registry each time: a cached binding could outlive a project change.
    fn scope_of(&self, session_id: &str) -> Option<SessionScope> {
        let metadata = self.persistence.as_ref()?.metadata(session_id).ok()?;
        let project_id = metadata.project_id.filter(|project| !project.is_empty())?;
        Some(SessionScope {
            project_id,
            root: std::path::PathBuf::from(metadata.cwd),
        })
    }
}

/// The editor's inline export `{format, data, …}` → the bytes, and the reply
/// `{path, bytes, format, …}` without the data.
fn written_export(result: &str, file: &ProjectFile) -> Result<(Value, Vec<u8>), CanvasError> {
    let mut reply: Value = serde_json::from_str(result)
        .map_err(|error| engine_error(format!("the export returned invalid JSON: {error}")))?;
    let fields = reply
        .as_object_mut()
        .ok_or_else(|| engine_error("the export returned no object"))?;
    let data = match fields.remove("data") {
        Some(Value::String(data)) => data,
        _ => return Err(engine_error("the export returned no data")),
    };
    let encoded = data.split_once(',').map_or(data.as_str(), |(_, rest)| rest);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|error| engine_error(format!("the export returned invalid base64: {error}")))?;
    fields.insert("path".to_string(), Value::String(file.relative.clone()));
    fields.insert("bytes".to_string(), Value::from(bytes.len()));
    Ok((reply, bytes))
}

/// `params` is an `engine.execute` of `command.batch` that is not itself a
/// step of a larger batch: a nested batch resolves `$step` references against
/// its own steps, so a prefix of the outer batch would not repeat its steps.
fn is_plain_batch(params: &Value) -> bool {
    params.get("command").and_then(Value::as_str) == Some("command.batch")
        && params.get("params")
            .and_then(|p| p.get("commands"))
            .is_some_and(|commands| commands.is_array())
}

/// Drop the `` `command` `` prefix from `invalid parameters for \`command\`: rest`
/// when the step already names the command.
fn strip_command(message: &str, command: &str) -> &str {
    let prefix = format!("invalid parameters for `{command}`: ");
    message.strip_prefix(&prefix).unwrap_or(message)
}

/// The newest export to each project file, so a late export never replaces
/// a file that a newer export to the same path wrote or is about to write.
#[derive(Default)]
pub(super) struct ExportClaims {
    newest: Mutex<HashMap<PathBuf, u64>>,
    next: AtomicU64,
}

impl ExportClaims {
    /// Make an export to `path` the newest one.
    fn claim(self: &Arc<Self>, path: &Path) -> ExportClaim {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.newest.lock().insert(path.to_path_buf(), id);
        ExportClaim {
            claims: Arc::clone(self),
            path: path.to_path_buf(),
            id,
        }
    }
}

/// One export's claim on its file, released when dropped.
pub(super) struct ExportClaim {
    claims: Arc<ExportClaims>,
    path: PathBuf,
    id: u64,
}

impl ExportClaim {
    /// Run `write` unless a newer export to the same file started. The lock
    /// is held through the write, so none can start in between.
    fn write_if_newest<T>(&self, write: impl FnOnce() -> T) -> Option<T> {
        let newest = self.claims.newest.lock();
        (newest.get(&self.path) == Some(&self.id)).then(write)
    }
}

impl Drop for ExportClaim {
    fn drop(&mut self) {
        let mut newest = self.claims.newest.lock();
        if newest.get(&self.path) == Some(&self.id) {
            newest.remove(&self.path);
        }
    }
}

fn engine_error(message: impl Into<String>) -> CanvasError {
    CanvasError::new(CanvasErrorCode::EngineError, message)
}

fn canvas_failure(error: &CanvasError) -> FrameReply {
    FrameReply::err_code(error.code.as_str(), error.message.clone())
}
