//! Integration tests for the tool registry surface.
//!
//! Verifies that every built-in tool is registered, executes by name, and
//! that unknown tools are rejected.

use diad_agent::tools;

// ---------------------------------------------------------------------------
// Registry completeness
// ---------------------------------------------------------------------------

#[test]
fn test_tool_registry_registers_all_tools() {
    let registry = tools::create_default_registry();
    let names = registry.list_tool_names();

    // Check all expected tools are present
    assert!(names.contains(&"read_file".to_string()));
    assert!(names.contains(&"write_file".to_string()));
    assert!(names.contains(&"list_dir".to_string()));
    assert!(names.contains(&"search_files".to_string()));
    assert!(names.contains(&"chmod_file".to_string()));
    assert!(names.contains(&"spawn_process".to_string()));
    assert!(names.contains(&"list_processes".to_string()));
    assert!(names.contains(&"kill_process".to_string()));
    assert!(names.contains(&"http_get".to_string()));
    assert!(names.contains(&"http_post".to_string()));
    assert!(names.contains(&"ping_host".to_string()));
    assert!(names.contains(&"get_system_info".to_string()));
    assert!(names.contains(&"install_package".to_string()));
    assert!(names.contains(&"remove_package".to_string()));
    assert!(names.contains(&"manage_service".to_string()));

    // All tools should have definitions
    let definitions = registry.list_tools();
    assert_eq!(definitions.len(), names.len());
}

#[test]
fn test_tool_execute_unknown_tool() {
    let registry = tools::create_default_registry();
    let result = registry.execute_tool("nonexistent_tool", serde_json::json!({}));

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Unknown tool"));
}

// ---------------------------------------------------------------------------
// Representative executions
// ---------------------------------------------------------------------------

#[test]
fn test_tool_list_dir_cwd() {
    let registry = tools::create_default_registry();
    let result = registry.execute_tool("list_dir", serde_json::json!({"path": "."}));

    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(tool_result.success, "list_dir should succeed on current dir");
}

#[test]
fn test_tool_get_system_info() {
    let registry = tools::create_default_registry();
    let result = registry.execute_tool("get_system_info", serde_json::json!({}));

    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(tool_result.success);
    assert!(!tool_result.output.is_empty());
}
