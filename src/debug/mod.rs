use crate::eval::{DebugAction, DebugHook, Environment, Evaluator};
use crate::json::{parse_json, JsonValue};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::span::{SourceMap, Span};
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

pub fn normalize_path(p: &str) -> String {
    let mut s = p.trim().replace('\\', "/");
    if let Some(stripped) = s.strip_prefix("file://") {
        s = stripped.to_string();
    }
    if s.starts_with('/') && s.len() >= 3 && s.as_bytes()[2] == b':' {
        s = s[1..].to_string();
    }
    s.to_lowercase()
}

#[derive(Debug, Clone)]
pub struct StackFrameInfo {
    pub id: usize,
    pub name: String,
    pub source_path: String,
    pub line: usize,
    pub column: usize,
    pub variables: Vec<(String, String, String)>, // (name, value, type)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteppingMode {
    None,
    StepIn,
    StepOver { target_depth: usize },
    StepOut { target_depth: usize },
}

pub struct SharedDebugState {
    pub breakpoints: HashMap<String, Vec<usize>>, // normalized path -> lines
    pub stop_on_entry: bool,
    pub has_stopped_on_entry: bool,
    pub is_running: bool,
    pub is_terminated: bool,
    pub stepping_mode: SteppingMode,
    pub current_frames: Vec<StackFrameInfo>,
    pub resume_action: Option<DebugAction>,
    pub paused_reason: Option<String>,
    pub pending_events: Vec<String>,
}

pub struct DapHook {
    pub state: Arc<Mutex<SharedDebugState>>,
    pub condvar: Arc<Condvar>,
    pub sm: Arc<Mutex<SourceMap>>,
    pub main_file_path: String,
}

impl DebugHook for DapHook {
    fn on_statement(
        &mut self,
        span: Span,
        env: &Environment,
        fn_name: &str,
        call_stack: &[(String, Span)],
    ) -> DebugAction {
        let (file_path, line, col) = {
            let sm_guard = self.sm.lock().unwrap();
            if let Some(f) = sm_guard.get_file(span.file_id) {
                let (l, c) = f.line_col(span.start);
                (f.path.to_string_lossy().to_string(), l, c)
            } else {
                (self.main_file_path.clone(), 1, 1)
            }
        };

        let norm_path = normalize_path(&file_path);
        let mut should_stop = false;
        let mut reason = "step".to_string();

        {
            let mut state = self.state.lock().unwrap();

            // 1. Check stopOnEntry
            if state.stop_on_entry && !state.has_stopped_on_entry {
                state.has_stopped_on_entry = true;
                should_stop = true;
                reason = "entry".to_string();
            }
            // 2. Check breakpoints
            else if let Some(lines) = state.breakpoints.get(&norm_path) {
                if lines.contains(&line) {
                    should_stop = true;
                    reason = "breakpoint".to_string();
                }
            }

            // 3. Check stepping mode
            if !should_stop {
                match state.stepping_mode {
                    SteppingMode::StepIn => {
                        should_stop = true;
                        reason = "step".to_string();
                    }
                    SteppingMode::StepOver { target_depth } => {
                        if call_stack.len() <= target_depth {
                            should_stop = true;
                            reason = "step".to_string();
                        }
                    }
                    SteppingMode::StepOut { target_depth } => {
                        if call_stack.len() < target_depth {
                            should_stop = true;
                            reason = "step".to_string();
                        }
                    }
                    SteppingMode::None => {}
                }
            }

            if should_stop {
                // Build stack frames
                let mut frames = Vec::new();

                // Top frame (current execution point)
                let vars = env.get_all_variables()
                    .into_iter()
                    .map(|(k, v)| {
                        let t = v.type_name();
                        (k, v.to_string(), t)
                    })
                    .collect();

                frames.push(StackFrameInfo {
                    id: 1,
                    name: fn_name.to_string(),
                    source_path: file_path.clone(),
                    line,
                    column: col,
                    variables: vars,
                });

                // Parent frames from call stack
                let sm_guard = self.sm.lock().unwrap();
                for (idx, (p_fn, p_span)) in call_stack.iter().rev().skip(1).enumerate() {
                    let (pl, pc, p_path) = if let Some(pf) = sm_guard.get_file(p_span.file_id) {
                        let (l, c) = pf.line_col(p_span.start);
                        (l, c, pf.path.to_string_lossy().to_string())
                    } else {
                        (1, 1, self.main_file_path.clone())
                    };

                    frames.push(StackFrameInfo {
                        id: idx + 2,
                        name: p_fn.clone(),
                        source_path: p_path,
                        line: pl,
                        column: pc,
                        variables: Vec::new(),
                    });
                }

                state.current_frames = frames;
                state.paused_reason = Some(reason.clone());
                state.resume_action = None;

                // Push stopped event
                let stopped_evt = format!(
                    r#"{{"type":"event","event":"stopped","body":{{"reason":"{}","threadId":1,"allThreadsStopped":true}}}}"#,
                    reason
                );
                state.pending_events.push(stopped_evt);
            }
        }

        if should_stop {
            let mut state = self.state.lock().unwrap();
            while state.resume_action.is_none() && !state.is_terminated {
                state = self.condvar.wait(state).unwrap();
            }

            let action = state.resume_action.unwrap_or(DebugAction::Continue);
            state.resume_action = None;
            return action;
        }

        DebugAction::Continue
    }

    fn on_output(&mut self, text: &str) {
        let mut state = self.state.lock().unwrap();
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        let evt = format!(
            r#"{{"type":"event","event":"output","body":{{"category":"stdout","output":"{}"}}}}"#,
            escaped
        );
        state.pending_events.push(evt);
    }
}

pub struct DebugServer {
    seq: u64,
    state: Arc<Mutex<SharedDebugState>>,
    condvar: Arc<Condvar>,
    sm: Arc<Mutex<SourceMap>>,
    program_path: Option<String>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl DebugServer {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(SharedDebugState {
            breakpoints: HashMap::new(),
            stop_on_entry: false,
            has_stopped_on_entry: false,
            is_running: false,
            is_terminated: false,
            stepping_mode: SteppingMode::None,
            current_frames: Vec::new(),
            resume_action: None,
            paused_reason: None,
            pending_events: Vec::new(),
        }));

        DebugServer {
            seq: 1,
            state,
            condvar: Arc::new(Condvar::new()),
            sm: Arc::new(Mutex::new(SourceMap::new())),
            program_path: None,
            worker_handle: None,
        }
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.seq;
        self.seq += 1;
        s
    }

    pub fn start(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut reader = stdin.lock();

        loop {
            // Drain pending events from worker thread
            self.flush_pending_events();

            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }

            let line = line.trim();
            if line.starts_with("Content-Length:") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() == 2 {
                    let len: usize = parts[1].trim().parse().unwrap_or(0);
                    let mut empty = String::new();
                    reader.read_line(&mut empty)?;

                    let mut body_buf = vec![0u8; len];
                    reader.read_exact(&mut body_buf)?;
                    let body_str = String::from_utf8_lossy(&body_buf);

                    let responses = self.handle_message(&body_str);
                    for resp in responses {
                        self.send_message(&resp);
                    }

                    self.flush_pending_events();
                }
            }
        }

        // Clean up worker thread
        {
            let mut state = self.state.lock().unwrap();
            state.is_terminated = true;
            state.resume_action = Some(DebugAction::Terminate);
            self.condvar.notify_all();
        }

        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }

        Ok(())
    }

    pub fn flush_pending_events(&mut self) {
        let events = {
            let mut state = self.state.lock().unwrap();
            std::mem::take(&mut state.pending_events)
        };

        for evt_body in events {
            let seq = self.next_seq();
            // Wrap or ensure seq is included
            let complete_evt = if evt_body.contains("\"seq\":") {
                evt_body
            } else {
                format!(r#"{{"seq":{},{}"#, seq, &evt_body[1..])
            };
            self.send_message(&complete_evt);
        }
    }

    pub fn send_message(&self, content: &str) {
        let msg = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(msg.as_bytes());
        let _ = stdout.flush();
    }

    pub fn handle_message(&mut self, body: &str) -> Vec<String> {
        let Ok(msg) = parse_json(body) else {
            return Vec::new();
        };

        let req_seq = match msg.get("seq") {
            Some(JsonValue::Number(n)) => *n as u64,
            _ => 1,
        };

        let command = msg
            .get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let args = msg.get("arguments");

        let mut out = Vec::new();

        match command.as_str() {
            "initialize" => {
                let seq = self.next_seq();
                let resp = format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"initialize","success":true,"body":{{"supportsConfigurationDoneRequest":true,"supportsFunctionBreakpoints":false,"supportsConditionalBreakpoints":false,"supportsEvaluateForHovers":true,"supportsStepBack":false,"supportsSetVariable":false}}}}"#,
                    seq, req_seq
                );
                out.push(resp);

                // Send initialized event
                let init_seq = self.next_seq();
                let evt = format!(
                    r#"{{"seq":{},"type":"event","event":"initialized"}}"#,
                    init_seq
                );
                out.push(evt);
            }
            "launch" => {
                if let Some(args) = args {
                    if let Some(prog) = args.get("program").and_then(|p| p.as_str()) {
                        self.program_path = Some(prog.to_string());
                    }
                    if let Some(JsonValue::Bool(b)) = args.get("stopOnEntry") {
                        let mut state = self.state.lock().unwrap();
                        state.stop_on_entry = *b;
                    }
                }

                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"launch","success":true}}"#,
                    seq, req_seq
                ));
            }
            "setBreakpoints" => {
                let mut registered = Vec::new();
                if let Some(args) = args {
                    let source_path = args
                        .get("source")
                        .and_then(|s| s.get("path"))
                        .and_then(|p| p.as_str())
                        .unwrap_or("");

                    let norm_path = normalize_path(source_path);

                    let mut lines_to_set = Vec::new();
                    if let Some(JsonValue::Array(bps)) = args.get("breakpoints") {
                        for b in bps {
                            if let Some(JsonValue::Number(l)) = b.get("line") {
                                let line_num = *l as usize;
                                lines_to_set.push(line_num);
                                registered.push(format!(r#"{{"verified":true,"line":{}}}"#, line_num));
                            }
                        }
                    } else if let Some(JsonValue::Array(lns)) = args.get("lines") {
                        for l in lns {
                            if let Some(num) = l.as_f64() {
                                let line_num = num as usize;
                                lines_to_set.push(line_num);
                                registered.push(format!(r#"{{"verified":true,"line":{}}}"#, line_num));
                            }
                        }
                    }

                    let mut state = self.state.lock().unwrap();
                    state.breakpoints.insert(norm_path, lines_to_set);
                }

                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"setBreakpoints","success":true,"body":{{"breakpoints":[{}]}}}}"#,
                    seq, req_seq, registered.join(",")
                ));
            }
            "setExceptionBreakpoints" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"setExceptionBreakpoints","success":true}}"#,
                    seq, req_seq
                ));
            }
            "configurationDone" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"configurationDone","success":true}}"#,
                    seq, req_seq
                ));

                // Launch program in background execution thread
                self.start_worker_execution();
            }
            "threads" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"threads","success":true,"body":{{"threads":[{{"id":1,"name":"Utama"}}]}}}}"#,
                    seq, req_seq
                ));
            }
            "stackTrace" => {
                let seq = self.next_seq();
                let state = self.state.lock().unwrap();

                let mut frames_json = Vec::new();
                for f in &state.current_frames {
                    let file_name = Path::new(&f.source_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "program.dap".to_string());

                    let esc_path = f.source_path.replace('\\', "/");
                    frames_json.push(format!(
                        r#"{{"id":{},"name":"{}","source":{{"name":"{}","path":"{}"}},"line":{},"column":{}}}"#,
                        f.id, f.name, file_name, esc_path, f.line, f.column
                    ));
                }

                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"stackTrace","success":true,"body":{{"stackFrames":[{}],"totalFrames":{}}}}}"#,
                    seq, req_seq, frames_json.join(","), frames_json.len()
                ));
            }
            "scopes" => {
                let seq = self.next_seq();
                // We provide Local scope (variablesReference: 1000)
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"scopes","success":true,"body":{{"scopes":[{{"name":"Lokal","presentationHint":"locals","variablesReference":1000,"expensive":false}}]}}}}"#,
                    seq, req_seq
                ));
            }
            "variables" => {
                let seq = self.next_seq();
                let state = self.state.lock().unwrap();

                let mut vars_json = Vec::new();
                if let Some(top_frame) = state.current_frames.first() {
                    for (name, val, ty) in &top_frame.variables {
                        let esc_val = val
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .replace('\n', " ");
                        vars_json.push(format!(
                            r#"{{"name":"{}","value":"{}","type":"{}","variablesReference":0}}"#,
                            name, esc_val, ty
                        ));
                    }
                }

                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"variables","success":true,"body":{{"variables":[{}]}}}}"#,
                    seq, req_seq, vars_json.join(",")
                ));
            }
            "evaluate" => {
                let seq = self.next_seq();
                let expr = args
                    .and_then(|a| a.get("expression"))
                    .and_then(|e| e.as_str())
                    .unwrap_or("")
                    .trim();

                let state = self.state.lock().unwrap();
                let mut match_found = None;

                if let Some(top_frame) = state.current_frames.first() {
                    for (name, val, ty) in &top_frame.variables {
                        if name == expr {
                            match_found = Some((val.clone(), ty.clone()));
                            break;
                        }
                    }
                }

                if let Some((val, ty)) = match_found {
                    let esc_val = val
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', " ");
                    out.push(format!(
                        r#"{{"seq":{},"type":"response","request_seq":{},"command":"evaluate","success":true,"body":{{"result":"{}","type":"{}","variablesReference":0}}}}"#,
                        seq, req_seq, esc_val, ty
                    ));
                } else {
                    out.push(format!(
                        r#"{{"seq":{},"type":"response","request_seq":{},"command":"evaluate","success":true,"body":{{"result":"<tidak ditemukan>","variablesReference":0}}}}"#,
                        seq, req_seq
                    ));
                }
            }
            "continue" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"continue","success":true,"body":{{"allThreadsContinued":true}}}}"#,
                    seq, req_seq
                ));

                let mut state = self.state.lock().unwrap();
                state.stepping_mode = SteppingMode::None;
                state.resume_action = Some(DebugAction::Continue);
                self.condvar.notify_all();
            }
            "next" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"next","success":true}}"#,
                    seq, req_seq
                ));

                let mut state = self.state.lock().unwrap();
                let cur_depth = state.current_frames.len();
                state.stepping_mode = SteppingMode::StepOver {
                    target_depth: cur_depth,
                };
                state.resume_action = Some(DebugAction::StepOver);
                self.condvar.notify_all();
            }
            "stepIn" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"stepIn","success":true}}"#,
                    seq, req_seq
                ));

                let mut state = self.state.lock().unwrap();
                state.stepping_mode = SteppingMode::StepIn;
                state.resume_action = Some(DebugAction::StepIn);
                self.condvar.notify_all();
            }
            "stepOut" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"stepOut","success":true}}"#,
                    seq, req_seq
                ));

                let mut state = self.state.lock().unwrap();
                let cur_depth = state.current_frames.len();
                state.stepping_mode = SteppingMode::StepOut {
                    target_depth: cur_depth,
                };
                state.resume_action = Some(DebugAction::StepOut);
                self.condvar.notify_all();
            }
            "pause" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"pause","success":true}}"#,
                    seq, req_seq
                ));

                let mut state = self.state.lock().unwrap();
                state.stepping_mode = SteppingMode::StepIn;
            }
            "disconnect" | "terminate" => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"{}","success":true}}"#,
                    seq, req_seq, command
                ));

                let mut state = self.state.lock().unwrap();
                state.is_terminated = true;
                state.resume_action = Some(DebugAction::Terminate);
                self.condvar.notify_all();
            }
            _ => {
                let seq = self.next_seq();
                out.push(format!(
                    r#"{{"seq":{},"type":"response","request_seq":{},"command":"{}","success":true}}"#,
                    seq, req_seq, command
                ));
            }
        }

        out
    }

    pub fn start_worker_execution(&mut self) {
        let prog_path = match &self.program_path {
            Some(p) => p.clone(),
            None => return,
        };

        let state_arc = Arc::clone(&self.state);
        let condvar_arc = Arc::clone(&self.condvar);
        let sm_arc = Arc::clone(&self.sm);

        let handle = thread::spawn(move || {
            let code = match std::fs::read_to_string(&prog_path) {
                Ok(c) => c,
                Err(e) => {
                    let mut state = state_arc.lock().unwrap();
                    let evt = format!(
                        r#"{{"type":"event","event":"output","body":{{"category":"stderr","output":"Gagal membaca berkas: {}\n"}}}}"#,
                        e
                    );
                    state.pending_events.push(evt);
                    state.pending_events.push(r#"{"type":"event","event":"terminated"}"#.to_string());
                    state.pending_events.push(r#"{"type":"event","event":"exited","body":{"exitCode":1}}"#.to_string());
                    return;
                }
            };

            let file_id = {
                let mut sm_guard = sm_arc.lock().unwrap();
                let (fid, _) = sm_guard.add_file(&prog_path, code.clone());
                fid
            };

            let mut lexer = Lexer::new(&code, file_id);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(d) => {
                    let mut state = state_arc.lock().unwrap();
                    let evt = format!(
                        r#"{{"type":"event","event":"output","body":{{"category":"stderr","output":"Galat sintaks: {}\n"}}}}"#,
                        d.message
                    );
                    state.pending_events.push(evt);
                    state.pending_events.push(r#"{"type":"event","event":"terminated"}"#.to_string());
                    state.pending_events.push(r#"{"type":"event","event":"exited","body":{"exitCode":1}}"#.to_string());
                    return;
                }
            };

            let mut parser = Parser::new(tokens, file_id);
            let program = match parser.parse_program() {
                Ok(p) => p,
                Err(d) => {
                    let mut state = state_arc.lock().unwrap();
                    let evt = format!(
                        r#"{{"type":"event","event":"output","body":{{"category":"stderr","output":"Galat parsing: {}\n"}}}}"#,
                        d.message
                    );
                    state.pending_events.push(evt);
                    state.pending_events.push(r#"{"type":"event","event":"terminated"}"#.to_string());
                    state.pending_events.push(r#"{"type":"event","event":"exited","body":{"exitCode":1}}"#.to_string());
                    return;
                }
            };

            let hook = Arc::new(Mutex::new(DapHook {
                state: Arc::clone(&state_arc),
                condvar: Arc::clone(&condvar_arc),
                sm: Arc::clone(&sm_arc),
                main_file_path: prog_path.clone(),
            }));

            let mut evaluator = Evaluator::new();
            evaluator.set_debug_hook(hook);

            let exit_code = match evaluator.run_program(&program) {
                Ok(_) => 0,
                Err(diag) => {
                    let mut state = state_arc.lock().unwrap();
                    let evt = format!(
                        r#"{{"type":"event","event":"output","body":{{"category":"stderr","output":"Galat waktu-jalan: {}\n"}}}}"#,
                        diag.message
                    );
                    state.pending_events.push(evt);
                    1
                }
            };

            {
                let mut state = state_arc.lock().unwrap();
                state.is_terminated = true;
                state.pending_events.push(r#"{"type":"event","event":"terminated"}"#.to_string());
                state.pending_events.push(format!(
                    r#"{{"type":"event","event":"exited","body":{{"exitCode":{}}}}}"#,
                    exit_code
                ));
            }
        });

        self.worker_handle = Some(handle);
    }
}
