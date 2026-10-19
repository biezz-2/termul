use serde_json::json;

use super::validate_props;

#[test]
fn opacity_above_one_is_rejected_not_rescaled() {
    let arguments = json!({"ids": [3], "props": {"opacity": 1.5}});
    let error = validate_props("set_properties", &arguments).unwrap_err();
    assert!(
        error.contains("opacity") && error.contains("0") && error.contains("1"),
        "names the key and range: {error}"
    );
    assert!(validate_props("set_properties", &json!({"props": {"opacity": 80}})).is_err());
    assert!(validate_props("set_properties", &json!({"props": {"opacity": -0.5}})).is_err());
}

#[test]
fn in_range_numbers_pass_through_untouched() {
    assert!(validate_props("set_properties", &json!({"props": {"opacity": 0.5}})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"opacity": 1.0}})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"opacity": 0}})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"width": 400}})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"cornerRadius": [12, 0, 8, 4]}})).is_ok());
}

#[test]
fn negative_sizes_and_radii_are_rejected() {
    assert!(validate_props("set_properties", &json!({"props": {"width": -50}})).is_err());
    assert!(validate_props("set_properties", &json!({"props": {"height": -1}})).is_err());
    // cornerRadius may be one number or [tl, tr, br, bl]; a negative entry is clamped away silently today.
    assert!(validate_props("set_properties", &json!({"props": {"cornerRadius": -10}})).is_err());
    assert!(
        validate_props("set_properties", &json!({"props": {"cornerRadius": [12, -10, 8, 4]}})).is_err(),
        "each corner is checked"
    );
    assert!(validate_props("set_properties", &json!({"props": {"width": 0}})).is_ok());
}

#[test]
fn non_numbers_and_other_keys_are_left_to_figcraft() {
    // Validation only covers documented numeric ranges; type errors and unknown
    // keys are the engine's own errors, not ours to duplicate.
    assert!(validate_props("set_properties", &json!({"props": {"opacity": "bold"}})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"madeUpKey": 999}})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"rotation": 720}})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"strokeWeight": -3}})).is_ok());
}

#[test]
fn create_node_props_are_checked_too() {
    // create_node applies props through node.set, so opacity: 1.5 there has the same silent rescale.
    assert!(validate_props("create_node", &json!({"type": "frame", "props": {"opacity": 1.5}})).is_err());
    assert!(validate_props("create_node", &json!({"type": "frame", "props": {"opacity": 0.5}})).is_ok());
}

#[test]
fn run_command_node_set_params_are_checked() {
    let run = json!({"command": "node.set", "params": {"ids": [3], "props": {"opacity": 1.5}}});
    assert!(validate_props("run_command", &run).is_err());
    let ok_run = json!({"command": "node.set", "params": {"ids": [3], "props": {"opacity": 0.5}}});
    assert!(validate_props("run_command", &ok_run).is_ok());
    // Other commands keep their params unchecked.
    let other = json!({"command": "frame.create", "params": {"width": -50}});
    assert!(validate_props("run_command", &other).is_ok());
}

#[test]
fn batch_node_set_steps_are_checked() {
    let batch = json!({"commands": [
        {"command": "node.create", "params": {"type": "frame"}},
        {"command": "node.set", "params": {"ids": [1], "props": {"opacity": 5.0}}},
        {"command": "node.set", "params": {"ids": [2], "props": {"width": -50}}}
    ]});
    let error = validate_props("batch", &batch).unwrap_err();
    assert!(error.contains("opacity") && error.contains("width"), "every failing step is reported: {error}");
    let ok_batch = json!({"commands": [
        {"command": "node.create", "params": {"type": "frame", "width": -50}},
        {"command": "node.set", "params": {"ids": [2], "props": {"opacity": 0.9}}}
    ]});
    assert!(validate_props("batch", &ok_batch).is_ok(), "node.create's own width is not node.set props");
}

#[test]
fn other_tools_are_never_checked() {
    assert!(validate_props("get_node", &json!({"id": 3, "props": {"opacity": 1.5}})).is_ok());
    assert!(validate_props("export", &json!({"props": {"opacity": 1.5}})).is_ok());
}

#[test]
fn missing_or_malformed_props_pass_validation() {
    // Structural errors ("`props` must be a non-empty object") stay figcraft's.
    assert!(validate_props("set_properties", &json!({})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": []})).is_ok());
    assert!(validate_props("set_properties", &json!({"props": {"opacity": null}})).is_ok());
    assert!(validate_props("batch", &json!({"commands": "nope"})).is_ok());
    assert!(validate_props("run_command", &json!({"command": "node.set", "params": 5})).is_ok());
}
