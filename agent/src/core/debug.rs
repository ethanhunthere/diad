//! DIAD AGENT — GDB Machine Interface (MI-2) debugger manager.
//!
//! Spawns `gdb --interpreter=mi2 --quiet` per session and speaks the MI-2
//! protocol: `-break-insert file:line`, `-exec-run`, `-exec-next`,
//! `-exec-step`, `-exec-finish`, `-exec-continue`, `-stack-list-frames`,
//! `-stack-list-variables`. A reader thread parses MI records and keeps a
//! shared snapshot (breakpoints, state, stack, locals) that the REST layer
//! serves to the frontend Debug panel.
//!
//! Protocol conventions (MI-2):
//! - Command results: `^done[,details]` / `^error,msg="..."` / `^running`
//! - Async stop:     `*stopped,reason=breakpoint-hit,frame={...},thread-id="1"`
//! - Async start:    `*running,thread-id="all"`
//! - Notifications:  `=breakpoint-created,bkpt={number="1",...}`
//! - Console:        `~"text"`
//! - Prompt:         `(gdb)`

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;

/// How long a synchronous MI command may take before we give up.
const CMD_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize)]
pub struct BreakpointDto {
    pub number: u32,
    pub file: String,
    pub line: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrameDto {
    pub level: u32,
    pub func: String,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct VarDto {
    pub name: String,
    pub value: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugSnapshot {
    pub running: bool,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub reason: Option<String>,
    pub breakpoints: Vec<BreakpointDto>,
    pub stack: Vec<FrameDto>,
    pub variables: Vec<VarDto>,
    pub console: Vec<String>,
}

/// One live GDB session (owns the child + reader thread + snapshot).
pub struct DebugSession {
    pub id: String,
    pub program: String,
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    /// Serializes MI command writes so records can't interleave.
    cmd_lock: Mutex<()>,
    /// Incremented by the reader thread each time `(gdb)` prompt is seen.
    prompt_count: Arc<(Mutex<u64>, Condvar)>,
    /// Shared snapshot updated by the reader thread.
    state: Mutex<DebugSnapshot>,
    /// Internal MI→DTO breakpoint map keyed by MI number.
    bps: Mutex<HashMap<u32, BreakpointDto>>,
}

impl DebugSession {
    /// Spawn gdb in MI-2 mode and start the reader thread.
    pub fn start(
        id: String,
        program: &str,
        args: &[String],
        cwd: Option<&str>,
    ) -> std::io::Result<Arc<DebugSession>> {
        let mut cmd = Command::new("gdb");
        cmd.args(["--interpreter=mi2", "--quiet"])
            .arg("--args")
            .arg(program)
            .args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().expect("gdb stdin piped");
        let stdout = child.stdout.take().expect("gdb stdout piped");

        let session = Arc::new(DebugSession {
            id,
            program: program.to_string(),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            cmd_lock: Mutex::new(()),
            prompt_count: Arc::new((Mutex::new(0), Condvar::new())),
            state: Mutex::new(DebugSnapshot {
                running: false,
                exited: false,
                exit_code: None,
                reason: None,
                breakpoints: Vec::new(),
                stack: Vec::new(),
                variables: Vec::new(),
                console: Vec::new(),
            }),
            bps: Mutex::new(HashMap::new()),
        });

        // Reader thread: parses every MI record into the snapshot.
        let s = Arc::clone(&session);
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let line = line.trim_end().to_string();
                if line.is_empty() {
                    continue;
                }
                s.ingest(&line);
                if line == "(gdb)" {
                    let (lock, cv) = &*s.prompt_count;
                    let mut n = lock.lock().unwrap();
                    *n += 1;
                    cv.notify_all();
                }
            }
            // gdb exited: mark the session dead.
            s.state.lock().unwrap().exited = true;
            let (lock, cv) = &*s.prompt_count;
            *lock.lock().unwrap() += 1;
            cv.notify_all();
        });

        Ok(session)
    }

    /// Write one MI command to gdb's stdin.
    fn send(&self, mi: &str) -> std::io::Result<()> {
        let mut stdin = self.stdin.lock().unwrap();
        writeln!(stdin, "{mi}")?;
        stdin.flush()
    }

    /// Send a synchronous command and wait for the next `(gdb)` prompt.
    fn sync_cmd(&self, mi: &str) -> Result<(), String> {
        let _guard = self.cmd_lock.lock().unwrap();
        let (lock, cv) = &*self.prompt_count;
        let before = *lock.lock().unwrap();
        self.send(mi).map_err(|e| format!("gdb write failed: {e}"))?;
        let mut n = lock.lock().unwrap();
        let deadline = std::time::Instant::now() + CMD_TIMEOUT;
        while *n <= before {
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err("gdb command timed out".into());
            }
            let (guard, _) = cv
                .wait_timeout(n, deadline - now)
                .map_err(|_| "gdb wait poisoned".to_string())?;
            n = guard;
        }
        Ok(())
    }

    /// Parse `*stopped` records and update the snapshot (called by reader).
    fn ingest(&self, line: &str) {
        if let Some(quoted) = line.strip_prefix("~\"") {
            // Console output: strip MI quoting.
            let text = quoted
                .trim_end_matches('"')
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\\"", "\"");
            let mut st = self.state.lock().unwrap();
            st.console.push(text);
            let cap = 500usize;
            let len = st.console.len();
            if len > cap {
                st.console.drain(0..len - cap);
            }
            return;
        }
        if line.starts_with("=breakpoint-created") || line.starts_with("=breakpoint-modified") {
            if let Some(bp) = parse_bkpt(line) {
                self.bps.lock().unwrap().insert(bp.number, bp);
                self.refresh_breakpoints();
            }
            return;
        }
        if line.starts_with("=breakpoint-deleted") {
            let nums: Vec<u32> = parse_breakpoint_numbers(line);
            {
                let mut bps = self.bps.lock().unwrap();
                for n in nums {
                    bps.remove(&n);
                }
            }
            self.refresh_breakpoints();
            return;
        }
        if line.starts_with("*stopped") {
            let mut st = self.state.lock().unwrap();
            st.running = false;
            st.reason = extract_field(line, "reason");
            if let Some(frame) = parse_frame(line) {
                st.stack = vec![frame];
            }
            st.exited = st.reason.as_deref() == Some("exited");
            st.exit_code = extract_field(line, "exit-code")
                .and_then(|v| v.parse::<i32>().ok());
            return;
        }
        if line.starts_with("*running") {
            let mut st = self.state.lock().unwrap();
            st.running = true;
            st.reason = Some("running".into());
            return;
        }
        if line.starts_with("^done,stack=") || line.starts_with("^done,stack=[") {
            if let Some(frames) = parse_stack(line) {
                let mut st = self.state.lock().unwrap();
                st.stack = frames;
            }
            return;
        }
        if line.starts_with("^done,variables=") || line.starts_with("^done,variables=[") {
            if let Some(vars) = parse_variables(line) {
                let mut st = self.state.lock().unwrap();
                st.variables = vars;
            }
            return;
        }
        if line.starts_with("^done,bkpt=") {
            if let Some(bp) = parse_bkpt(line) {
                self.bps.lock().unwrap().insert(bp.number, bp);
                self.refresh_breakpoints();
            }
        }
    }

    fn refresh_breakpoints(&self) {
        let mut list: Vec<BreakpointDto> = self.bps.lock().unwrap().values().cloned().collect();
        list.sort_by_key(|b| b.number);
        let mut st = self.state.lock().unwrap();
        st.breakpoints = list;
    }

    // ---- Public control API -------------------------------------------------

    /// Set a breakpoint at `file:line`.
    pub fn set_breakpoint(&self, file: &str, line: u32) -> Result<BreakpointDto, String> {
        self.sync_cmd(&format!("-break-insert {file}:{line}"))?;
        self.refresh_breakpoints();
        let bps = self.bps.lock().unwrap();
        bps.values()
            .find(|b| b.file == file && b.line == line)
            .cloned()
            .ok_or_else(|| format!("breakpoint not created at {file}:{line}"))
    }

    /// Delete a breakpoint by MI number.
    pub fn delete_breakpoint(&self, number: u32) -> Result<(), String> {
        self.sync_cmd(&format!("-break-delete {number}"))?;
        self.bps.lock().unwrap().remove(&number);
        self.refresh_breakpoints();
        Ok(())
    }

    pub fn run(&self) -> Result<(), String> {
        self.send("-exec-run").map_err(|e| format!("gdb write failed: {e}"))
    }

    pub fn continue_exec(&self) -> Result<(), String> {
        self.send("-exec-continue").map_err(|e| format!("gdb write failed: {e}"))
    }

    pub fn step(&self) -> Result<(), String> {
        self.send("-exec-step").map_err(|e| format!("gdb write failed: {e}"))
    }

    pub fn next(&self) -> Result<(), String> {
        self.send("-exec-next").map_err(|e| format!("gdb write failed: {e}"))
    }

    pub fn finish(&self) -> Result<(), String> {
        self.send("-exec-finish").map_err(|e| format!("gdb write failed: {e}"))
    }

    /// Refresh the stack + local variables (call while stopped).
    pub fn refresh_frames(&self) -> Result<(), String> {
        self.sync_cmd("-stack-list-frames 0 20")?;
        self.sync_cmd("-stack-list-variables --all-values")?;
        Ok(())
    }

    pub fn snapshot(&self) -> DebugSnapshot {
        self.state.lock().unwrap().clone()
    }

    /// Kill the inferior and quit gdb.
    pub fn kill(&self) {
        let _ = self.send("-exec-abort");
        let _ = self.send("-gdb-exit");
        let _ = self.child.lock().unwrap().kill();
    }
}

impl Drop for DebugSession {
    fn drop(&mut self) {
        let _ = self.child.lock().unwrap().kill();
    }
}

// ---------------------------------------------------------------------------
// MI parsing helpers
// ---------------------------------------------------------------------------

/// Extract `key="value"` (or `key=value`) from an MI record, wherever the
/// field starts: after a comma, an opening brace/bracket, or at position 0.
fn extract_field(line: &str, key: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(stripped) = line[i..].strip_prefix(key) {
            if let Some(after_eq) = stripped.strip_prefix('=') {
                let prev_ok = i == 0 || matches!(bytes[i - 1] as char, ',' | '{' | '[' | ' ' | '\t');
                if prev_ok {
                    return Some(strip_value(after_eq));
                }
            }
        }
        i += 1;
    }
    None
}

fn strip_value(v: &str) -> String {
    let v = v.trim();
    if let Some(rest) = v.strip_prefix('"') {
        // Quoted value: stop at the closing (unescaped) quote.
        let mut out = String::new();
        let mut escaped = false;
        for ch in rest.chars() {
            if escaped {
                out.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                return out;
            }
            out.push(ch);
        }
        return out;
    }
    // Unquoted: stop at the first top-level comma.
    // `char_indices()` yields byte offsets, which are what string slicing needs
    // (a char index from `enumerate()` would panic on multi-byte UTF-8).
    let mut depth = 0i32;
    for (byte_idx, ch) in v.char_indices() {
        match ch {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ',' if depth == 0 => return v[..byte_idx].to_string(),
            _ => {}
        }
    }
    v.to_string()
}

/// Parse `bkpt={number="1",...}` inside a record.
fn parse_bkpt(line: &str) -> Option<BreakpointDto> {
    let start = line.find("bkpt=")?;
    let body = strip_value(&line[start + 5..]);
    let number = extract_field(&body, "number")?.parse::<u32>().ok()?;
    let file = extract_field(&body, "fullname")
        .or_else(|| extract_field(&body, "file"))
        .unwrap_or_default();
    let line_no = extract_field(&body, "line").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let enabled = extract_field(&body, "enabled").as_deref() != Some("n");
    Some(BreakpointDto { number, file, line: line_no, enabled })
}

fn parse_breakpoint_numbers(line: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for tok in line.split(',') {
        if let Some(rest) = tok.trim().strip_prefix("bkpt=") {
            if let Ok(n) = rest.parse::<u32>() {
                out.push(n);
            }
        }
    }
    out
}

/// Parse a frame tuple: `frame={level="0",func="main",file="x.c",line="5",...}`.
fn parse_frame(line: &str) -> Option<FrameDto> {
    let start = line.find("frame=")?;
    let body = strip_value(&line[start + 6..]);
    Some(FrameDto {
        level: extract_field(&body, "level").and_then(|v| v.parse().ok()).unwrap_or(0),
        func: extract_field(&body, "func").unwrap_or_default(),
        file: extract_field(&body, "fullname").or_else(|| extract_field(&body, "file")).unwrap_or_default(),
        line: extract_field(&body, "line").and_then(|v| v.parse().ok()).unwrap_or(0),
    })
}

/// Parse `^done,stack=[frame={...},frame={...}]`.
fn parse_stack(line: &str) -> Option<Vec<FrameDto>> {
    let start = line.find("stack=[")?;
    let body = &line[start + 7..];
    let mut frames = Vec::new();
    for seg in split_tuples(body) {
        if let Some(f) = parse_frame(&format!("frame={seg}")) {
            frames.push(f);
        }
    }
    Some(frames)
}

/// Parse `^done,variables=[name="x",arg="int",value="42",...]` (groups of 3).
fn parse_variables(line: &str) -> Option<Vec<VarDto>> {
    let start = line.find("variables=[")?;
    let body = &line[start + 11..];
    let mut vars = Vec::new();
    let mut name = String::new();
    let mut kind = String::new();
    let mut value = String::new();
    let mut i = 0usize;
    for tok in split_top_level(body) {
        if let Some(v) = tok.strip_prefix("name=") {
            name = strip_value(v);
        } else if let Some(v) = tok.strip_prefix("arg=") {
            kind = strip_value(v);
        } else if let Some(v) = tok.strip_prefix("value=") {
            value = strip_value(v);
        }
        i += 1;
        if i.is_multiple_of(3) {
            vars.push(VarDto { name: name.clone(), value: value.clone(), kind: kind.clone() });
            name.clear();
            kind.clear();
            value.clear();
        }
    }
    Some(vars)
}

/// Split a top-level `[...]` body into tokens at depth 0.
fn split_top_level(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in body.chars() {
        match ch {
            '{' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            '}' | ']' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Split `[frame={...},frame={...}]` into each tuple body, braces kept so
/// `parse_frame` can re-locate `frame=`.
fn split_tuples(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in body.chars() {
        match ch {
            '{' => {
                depth += 1;
                cur.push(ch);
            }
            '}' => {
                depth -= 1;
                cur.push(ch);
                if depth == 0 {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ if depth >= 1 => cur.push(ch),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// Holds all live debug sessions, keyed by session id.
pub struct DebugManager {
    sessions: Mutex<HashMap<String, Arc<DebugSession>>>,
    seq: AtomicU64,
}

impl DebugManager {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()), seq: AtomicU64::new(0) }
    }

    /// Start a new GDB session for `program`.
    pub fn start(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&str>,
    ) -> Result<(String, Arc<DebugSession>), String> {
        let id = format!("dbg-{}", self.seq.fetch_add(1, Ordering::SeqCst));
        let session = DebugSession::start(id.clone(), program, args, cwd)
            .map_err(|e| format!("failed to start gdb: {e}"))?;
        self.sessions.lock().unwrap().insert(id.clone(), Arc::clone(&session));
        Ok((id, session))
    }

    pub fn get(&self, id: &str) -> Option<Arc<DebugSession>> {
        self.sessions.lock().unwrap().get(id).cloned()
    }

    pub fn stop(&self, id: &str) -> bool {
        if let Some(s) = self.sessions.lock().unwrap().remove(id) {
            s.kill();
            true
        } else {
            false
        }
    }

    pub fn list(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .map(|(id, s)| (id.clone(), s.program.clone()))
            .collect();
        out.sort();
        out
    }
}

impl Default for DebugManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_field() {
        let line = r#"*stopped,reason="breakpoint-hit",frame={level="0",func="main",file="/tmp/a.c",line="7"},thread-id="1""#;
        assert_eq!(extract_field(line, "reason").as_deref(), Some("breakpoint-hit"));
        assert_eq!(extract_field(line, "exit-code"), None);
    }

    #[test]
    fn test_parse_frame() {
        let line = r#"*stopped,reason="breakpoint-hit",frame={level="0",func="main",fullname="/tmp/a.c",line="7"}"#;
        let f = parse_frame(line).unwrap();
        assert_eq!(f.func, "main");
        assert_eq!(f.file, "/tmp/a.c");
        assert_eq!(f.line, 7);
        assert_eq!(f.level, 0);
    }

    #[test]
    fn test_parse_bkpt() {
        let line = r#"^done,bkpt={number="1",type="breakpoint",disp="keep",enabled="y",addr="0x0000000000401131",func="main",file="a.c",fullname="/tmp/a.c",line="7"}"#;
        let b = parse_bkpt(line).unwrap();
        assert_eq!(b.number, 1);
        assert_eq!(b.file, "/tmp/a.c");
        assert_eq!(b.line, 7);
        assert!(b.enabled);
    }

    #[test]
    fn test_parse_stack() {
        let line = r#"^done,stack=[frame={level="0",addr="0x0000000000401131",func="main",file="a.c",fullname="/tmp/a.c",line="7"},frame={level="1",addr="0x0000000000401142",func="helper",file="b.c",fullname="/tmp/b.c",line="3"}]"#;
        let frames = parse_stack(line).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].func, "main");
        assert_eq!(frames[0].line, 7);
        assert_eq!(frames[1].func, "helper");
        assert_eq!(frames[1].file, "/tmp/b.c");
    }

    #[test]
    fn test_parse_variables() {
        let line = r#"^done,variables=[name="x",arg="int",value="42",name="y",arg="float",value="1.5"]"#;
        let vars = parse_variables(line).unwrap();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "x");
        assert_eq!(vars[0].value, "42");
        assert_eq!(vars[0].kind, "int");
        assert_eq!(vars[1].name, "y");
        assert_eq!(vars[1].value, "1.5");
    }

    #[test]
    fn test_split_tuples() {
        let body = r#"frame={level="0",func="main"},frame={level="1",func="helper"}"#;
        let parts = split_tuples(body);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("func=\"main\""));
        assert!(parts[1].contains("func=\"helper\""));
    }

    #[test]
    fn test_console_quoting() {
        let line = r#"~"Hello\nworld""#;
        // ingest only strips quoting when the session exists; test the strip logic.
        let text = line[2..].trim_end_matches('"').replace("\\n", "\n");
        assert_eq!(text, "Hello\nworld");
    }
}
