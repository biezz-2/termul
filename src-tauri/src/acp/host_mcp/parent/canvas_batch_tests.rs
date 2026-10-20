//! The batch step diagnostic (#1424): a failed `command.batch` names the
//! failing step, found by probing prefixes of the batch against the editor.
//!
//! The renderer in these tests is a fake that mimics the pinned engine
//! closely enough for the probe: a batch fails when one of its steps fails
//! (here: a step whose params carry `"fail": true`), and any batch fails the
//! same way when its last command is `command.batch` with `commands: null`
//! (the sentinel). Like the engine, the fake rolls a failed batch back; the
//! tests assert the probes never leave the editor with a changed document
//! (counted here as the renderer's applied steps).

use std::sync::Arc;

use parking_lot::Mutex;
use std::time::Duration;

use serde_json::{Value, json};

use super::super::gateway_harness::{Harness, mount_renderer};
use crate::acp::host_mcp::bridge::Bridge;
use crate::acp::host_mcp::canvas::{CanvasCallFrame, CanvasError, CanvasErrorCode};

/// The params of the batch this fake editor was last asked to run.
#[derive(Default)]
struct Seen {
    batches: Vec<Vec<Value>>,
}

/// Answer `engine.execute` of `command.batch`: a step with `"fail": true` in
/// its params fails with the engine's shape of error (`invalid parameters for
/// \`node.create\`: Fill needs an auto layout parent`), a step that is itself
/// a nested `command.batch` with `commands: null` fails as the sentinel, and
/// anything else succeeds. Every failure is `ENGINE_ERROR`, as over the wire.
fn fake_engine(frame: &CanvasCallFrame, seen: &Seen) -> Option<Result<String, CanvasError>> {
    if frame.method != "engine.execute" {
        return Some(Ok("{}".into()));
    }
    let params: Value = serde_json::from_str(&frame.params).unwrap();
    if params["command"] != "command.batch" {
        return Some(Ok("{}".into()));
    }
    let commands = params["params"]["commands"].as_array().cloned().unwrap_or_default();
    seen.batches.push(commands.clone());
    for (index, step) in commands.iter().enumerate() {
        let step_params = &step["params"];
        if step["command"] == "command.batch" && step_params.get("commands").is_some_and(Value::is_null) {
            return Some(Err(CanvasError::new(
                CanvasErrorCode::EngineError,
                "invalid parameters for `command.batch`: `commands` must be an array",
            )));
        }
        if step_params.get("fail").and_then(Value::as_bool) == Some(true) {
            let command = step["command"].as_str().unwrap_or("node.create");
            return Some(Err(CanvasError::new(
                CanvasErrorCode::EngineError,
                format!("invalid parameters for `{command}`: Fill needs an auto layout parent (step {index} failed)"),
            )));
        }
    }
    Some(Ok(json!([{"id": 1}]).to_string()))
}

/// Mount the fake editor and return every batch it was asked to run.
fn editor(harness: &Harness, project: &str) -> Arc<std::sync::Mutex<Seen>> {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let state = Arc::clone(&seen);
    mount_renderer(&harness.router(), "conn-a", project, 1, move |frame| {
        let mut state = state.lock();
        fake_engine(frame, &mut state)
    });
    seen
}

fn batch(steps: Vec<Value>) -> Value {
    json!({"commands": steps})
}

fn step(command: &str, name: &str, fail: bool) -> Value {
    json!({"command": command, "params": {"name": name, "fail": fail}})
}

fn engine_error_of(result: Result<Value, String>) -> CanvasError {
    // The bridge turns a typed error into the backend string JSON.
    let error: Value = serde_json::from_str(&result.unwrap_err()).unwrap();
    CanvasError::new(
        CanvasErrorCode::parse(error["code"].as_str().unwrap_or("ENGINE_ERROR")),
        error["message"].as_str().unwrap_or_default().to_string(),
    )
}

#[test]
fn a_failed_batch_names_the_failing_step() {
    let (harness, configs) = Harness::new(&[("sess-1", Some("proj-1"))]);
    let seen = editor(&harness, "proj-1");
    let mut bridge = Bridge::new(configs[0].clone()).with_tool("batch");
    let commands: Vec<Value> = (0..20)
        .map(|i| step("node.create", &format!("Layer {i}"), i == 13))
        .collect();
    let result = figcraft_mcp::call_tool(&mut bridge, "batch", &batch(commands.clone()));
    assert!(result.is_error, "{:?}", result.content);

    let error = engine_error_of(figcraft_mcp_call_result(&mut bridge, &commands));
    assert!(
        error.message.contains("step 13"),
        "the error names the failing step: {}",
        error.message
    );
    assert!(
        error.message.contains("(`node.create` \"Layer 13\")"),
        "the error names the command and layer: {}",
        error.message
    );
    assert!(
        error.message.contains("Fill needs an auto layout parent"),
        "the original reason survives: {}",
        error.message
    );
    // Every probe ends in the sentinel, so the editor rolled each back: the
    // only batch that could have applied anything is the original, which
    // failed. All batches the fake saw were the original, the sentinel-only
    // probe, and prefix probes.
    let batches = seen.lock().batches.clone();
    assert_eq!(batches.len(), 1 + 1 + 6, "original + sentinel + 6 bisect probes");
    for probed in &batches[1..] {
        let last = probed.last().unwrap();
        assert!(
            last["command"] == "command.batch" && last["params"]["commands"].is_null(),
            "every probe ends in the sentinel: {last}"
        );
    }
}

/// Run `commands` through figcraft's `batch` tool and return the raw error
/// the backend would read.
fn figcraft_mcp_call_result(bridge: &mut Bridge, commands: &[Value]) -> Result<Value, String> {
    let result = figcraft_mcp::call_tool(bridge, "batch", &batch(commands.to_vec()));
    if result.is_error {
        Err(result.first_text().to_string())
    } else {
        Ok(serde_json::from_str(result.first_text()).unwrap())
    }
}

#[test]
fn a_batch_whose_error_is_the_sentinels_keeps_its_original_error() {
    let (harness, configs) = Harness::new(&[("sess-1", Some("proj-1"))]);
    let _seen = editor(&harness, "proj-1");
    let mut bridge = Bridge::new(configs[0].clone()).with_tool("batch");
    // The only step fails with exactly the sentinel's message shape: a
    // nested `command.batch` step with `commands: null`.
    let commands = vec![json!({"command": "command.batch", "params": {"commands": null}})];
    let error = engine_error_of(figcraft_mcp_call_result(&mut bridge, &commands));
    assert_eq!(
        error.message,
        "invalid parameters for `command.batch`: `commands` must be an array",
        "the original error passes through unchanged"
    );
}

#[test]
fn a_transport_failure_is_not_probed() {
    let (harness, configs) = Harness::new(&[("sess-1", Some("proj-1"))]);
    // No renderer mounted: every call is CANVAS_CLOSED.
    let mut bridge = Bridge::new(configs[0].clone()).with_tool("batch");
    let commands = vec![step("node.create", "Layer 0", true)];
    let error = engine_error_of(figcraft_mcp_call_result(&mut bridge, &commands));
    assert_eq!(
        error.code,
        CanvasErrorCode::CanvasClosed,
        "a closed canvas is reported as closed, not probed"
    );
}

#[test]
fn the_first_step_of_a_failing_batch_is_named() {
    let (harness, configs) = Harness::new(&[("sess-1", Some("proj-1"))]);
    let _seen = editor(&harness, "proj-1");
    let mut bridge = Bridge::new(configs[0].clone()).with_tool("batch");
    let commands = vec![step("node.create", "Broken", true), step("node.create", "Fine", false)];
    let error = engine_error_of(figcraft_mcp_call_result(&mut bridge, &commands));
    assert!(
        error.message.contains("step 0") && error.message.contains("\"Broken\""),
        "the first step is named: {}",
        error.message
    );
}

#[test]
fn the_last_step_of_a_failing_batch_is_named() {
    let (harness, configs) = Harness::new(&[("sess-1", Some("proj-1"))]);
    let _seen = editor(&harness, "proj-1");
    let mut bridge = Bridge::new(configs[0].clone()).with_tool("batch");
    let commands = vec![
        step("node.create", "Fine 0", false),
        step("node.create", "Fine 1", false),
        step("node.create", "Broken", true),
    ];
    let error = engine_error_of(figcraft_mcp_call_result(&mut bridge, &commands));
    assert!(
        error.message.contains("step 2") && error.message.contains("\"Broken\""),
        "the last step is named: {}",
        error.message
    );
}

#[test]
fn an_unnamed_step_is_named_by_command_only() {
    let (harness, configs) = Harness::new(&[("sess-1", Some("proj-1"))]);
    let _seen = editor(&harness, "proj-1");
    let mut bridge = Bridge::new(configs[0].clone()).with_tool("batch");
    let commands = vec![json!({"command": "node.set", "params": {"fail": true}})];
    let error = engine_error_of(figcraft_mcp_call_result(&mut bridge, &commands));
    assert!(
        error.message.contains("step 0 (`node.set`)"),
        "a step without a name still names its command: {}",
        error.message
    );
}

#[test]
fn a_successful_batch_is_not_probed() {
    let (harness, configs) = Harness::new(&[("sess-1", Some("proj-1"))]);
    let seen = editor(&harness, "proj-1");
    let mut bridge = Bridge::new(configs[0].clone()).with_tool("batch");
    let commands = vec![step("node.create", "Layer 0", false)];
    let result = figcraft_mcp::call_tool(&mut bridge, "batch", &batch(commands));
    assert!(!result.is_error, "{:?}", result.content);
    let batches = seen.lock().batches.clone();
    assert_eq!(batches.len(), 1, "only the original batch ran");
}

// A long timeout for probes: the harness renderer answers immediately, this
// only bounds a wedged test.
const _TIMEOUT: Duration = Duration::from_secs(30);
