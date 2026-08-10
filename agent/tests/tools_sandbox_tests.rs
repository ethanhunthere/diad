//! Security-critical negative tests for the tool sandbox.
//!
//! Every assertion here is a *negative* test: the operation MUST fail.
//! The default registry sandbox allowlist is `~`, `/tmp`, `/etc/diad`
//! (see `ToolRegistry::default`), so `/etc/passwd` is outside it.

use diad_agent::tools;

fn registry() -> tools::ToolRegistry {
    tools::create_default_registry()
}

// ---------------------------------------------------------------------------
// Filesystem sandbox escapes
// ---------------------------------------------------------------------------

#[test]
fn test_read_file_outside_sandbox_rejected() {
    let r = registry();
    let result = r
        .execute_tool("read_file", serde_json::json!({"path": "/etc/passwd"}))
        .expect("handler must return a result");

    assert!(!result.success, "reading /etc/passwd must be denied");
    let msg = format!("{} {}", result.output, result.error.unwrap_or_default());
    assert!(
        msg.contains("sandbox"),
        "error must mention the sandbox: {msg}"
    );
}

#[test]
fn test_write_file_outside_sandbox_rejected() {
    let r = registry();
    let result = r
        .execute_tool(
            "write_file",
            serde_json::json!({"path": "/etc/diad-evil.conf", "content": "pwned"}),
        )
        .expect("handler must return a result");

    assert!(!result.success, "writing into /etc must be denied");
}

#[test]
fn test_read_file_dotdot_escape_rejected() {
    let r = registry();
    let result = r
        .execute_tool("read_file", serde_json::json!({"path": "../../etc/passwd"}))
        .expect("handler must return a result");

    assert!(!result.success, "dotdot traversal must be denied");
}

#[test]
fn test_chmod_file_outside_sandbox_rejected() {
    let r = registry();
    let result = r
        .execute_tool(
            "chmod_file",
            serde_json::json!({"path": "/etc/passwd", "mode": "777"}),
        )
        .expect("handler must return a result");

    assert!(!result.success, "chmod outside sandbox must be denied");
}

// ---------------------------------------------------------------------------
// Process sandbox escapes
// ---------------------------------------------------------------------------

#[test]
fn test_spawn_process_banned_shell_rejected() {
    let r = registry();
    let result = r
        .execute_tool(
            "spawn_process",
            serde_json::json!({"command": "sh", "args": ["-c", "rm -rf /"]}),
        )
        .expect("handler must return a result");

    assert!(
        !result.success,
        "spawning a shell must be denied (shell escape vector)"
    );
    let msg = format!("{} {}", result.output, result.error.unwrap_or_default());
    assert!(
        msg.contains("Refusing") || msg.contains("banned"),
        "error must explain the refusal: {msg}"
    );
}

#[test]
fn test_spawn_process_sudo_rejected() {
    let r = registry();
    let result = r
        .execute_tool("spawn_process", serde_json::json!({"command": "sudo"}))
        .expect("handler must return a result");

    assert!(!result.success, "spawning sudo must be denied");
}

#[test]
fn test_spawn_process_empty_command_rejected() {
    let r = registry();
    let result = r
        .execute_tool("spawn_process", serde_json::json!({"command": "  "}))
        .expect("handler must return a result");

    assert!(!result.success, "empty command must be denied");
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

#[test]
fn test_http_get_invalid_url_fails_gracefully() {
    let r = registry();
    // "not a url" fails URL parsing immediately — no network is contacted.
    let result = r
        .execute_tool("http_get", serde_json::json!({"url": "not a url"}))
        .expect("handler must return a result");

    assert!(!result.success, "malformed URL must fail cleanly");
}
