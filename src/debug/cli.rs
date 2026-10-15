use crate::eval::{DebugAction, DebugHook, Environment, Evaluator};
use crate::lexer::Lexer;
use crate::parser::{Parser, Precedence};
use crate::span::{SourceMap, Span};
use super::{normalize_path, SteppingMode};
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct CliDebugger {
    pub breakpoints: HashMap<String, HashSet<usize>>, // normalized path -> set of line numbers
    pub stepping_mode: SteppingMode,
    pub stop_on_entry: bool,
    pub has_stopped_on_entry: bool,
    pub sm: Arc<Mutex<SourceMap>>,
    pub main_file_path: String,
    pub last_command: Option<String>,
    reader: Box<dyn BufRead + Send + Sync>,
    writer: Box<dyn Write + Send + Sync>,
}

impl CliDebugger {
    pub fn new(
        sm: Arc<Mutex<SourceMap>>,
        main_file_path: String,
        initial_breakpoints: Vec<usize>,
        stop_on_entry: bool,
        reader: Box<dyn BufRead + Send + Sync>,
        writer: Box<dyn Write + Send + Sync>,
    ) -> Self {
        let mut breakpoints = HashMap::new();
        if !initial_breakpoints.is_empty() {
            let norm = normalize_path(&main_file_path);
            let set: HashSet<usize> = initial_breakpoints.into_iter().collect();
            breakpoints.insert(norm, set);
        }

        CliDebugger {
            breakpoints,
            stepping_mode: SteppingMode::None,
            stop_on_entry,
            has_stopped_on_entry: false,
            sm,
            main_file_path,
            last_command: None,
            reader,
            writer,
        }
    }

    pub fn new_stdio(
        sm: Arc<Mutex<SourceMap>>,
        main_file_path: String,
        initial_breakpoints: Vec<usize>,
        stop_on_entry: bool,
    ) -> Self {
        Self::new(
            sm,
            main_file_path,
            initial_breakpoints,
            stop_on_entry,
            Box::new(io::BufReader::new(io::stdin())),
            Box::new(io::stdout()),
        )
    }

    fn print_source_snippet(&mut self, file_path: &str, line: usize, context: usize) {
        let sm_guard = self.sm.lock().unwrap();
        let norm = normalize_path(file_path);
        let maybe_file = sm_guard.find_by_path(Path::new(file_path)).or_else(|| {
            sm_guard
                .files()
                .iter()
                .find(|f| normalize_path(&f.path.to_string_lossy()) == norm)
                .cloned()
        });

        if let Some(sf) = maybe_file {
            let start_line = line.saturating_sub(context).max(1);
            let end_line = line + context;
            for l in start_line..=end_line {
                if let Some(line_str) = sf.get_line(l) {
                    if l == line {
                        let _ = writeln!(
                            self.writer,
                            "  {} {:4} | {}",
                            "->".green().bold(),
                            l.to_string().green().bold(),
                            line_str.white().bold()
                        );
                    } else {
                        let _ = writeln!(
                            self.writer,
                            "     {:4} | {}",
                            l.to_string().dimmed(),
                            line_str
                        );
                    }
                }
            }
        }
    }

    fn print_variables(&mut self, env: &Environment) {
        let vars = env.get_all_variables();
        if vars.is_empty() {
            let _ = writeln!(
                self.writer,
                "  {}",
                "(tidak ada variabel lokal dalam cakupan)".dimmed()
            );
        } else {
            let _ = writeln!(self.writer, "{}", "Variabel Lokal:".cyan().bold());
            for (name, val) in vars {
                let _ = writeln!(
                    self.writer,
                    "  {} ({}) = {}",
                    name.green().bold(),
                    val.type_name().yellow(),
                    val
                );
            }
        }
    }

    fn evaluate_and_print(&mut self, expr_str: &str, span: Span, env: &Environment) {
        let expr_str = expr_str.trim();
        if expr_str.is_empty() {
            let _ = writeln!(self.writer, "Format: p <ekspresi atau nama_variabel>");
            return;
        }

        // 1. Coba pencarian variabel langsung
        if let Some(val) = env.get(expr_str) {
            let _ = writeln!(
                self.writer,
                "{} ({}) = {}",
                expr_str.green().bold(),
                val.type_name().yellow(),
                val
            );
            return;
        }

        // 2. Evaluasi ekspresi dinamis
        let mut lexer = Lexer::new(expr_str, span.file_id);
        match lexer.tokenize() {
            Ok(tokens) => {
                let mut parser = Parser::new(tokens, span.file_id);
                match parser.parse_expr(Precedence::Lowest) {
                    Ok(expr) => {
                        let mut eval = Evaluator::new();
                        let mut eval_env = env.clone();
                        match eval.eval_expr(&expr, &mut eval_env) {
                            Ok(val) => {
                                let _ = writeln!(
                                    self.writer,
                                    "{} ({}) = {}",
                                    expr_str.green().bold(),
                                    val.type_name().yellow(),
                                    val
                                );
                            }
                            Err(diag) => {
                                let _ = writeln!(
                                    self.writer,
                                    "{}: {}",
                                    "Galat evaluasi".red().bold(),
                                    diag.message
                                );
                            }
                        }
                    }
                    Err(d) => {
                        let _ = writeln!(
                            self.writer,
                            "{}: {}",
                            "Galat sintaks ekspresi".red().bold(),
                            d.message
                        );
                    }
                }
            }
            Err(d) => {
                let _ = writeln!(
                    self.writer,
                    "{}: {}",
                    "Galat token ekspresi".red().bold(),
                    d.message
                );
            }
        }
    }

    fn print_backtrace(
        &mut self,
        fn_name: &str,
        file_path: &str,
        line: usize,
        col: usize,
        call_stack: &[(String, Span)],
    ) {
        let _ = writeln!(self.writer, "{}", "Jejak Panggilan (Call Stack):".cyan().bold());
        let _ = writeln!(
            self.writer,
            "  #0 {} pada {}:{}:{}",
            fn_name.yellow().bold(),
            file_path,
            line,
            col
        );

        let sm_guard = self.sm.lock().unwrap();
        for (idx, (p_fn, p_span)) in call_stack.iter().rev().skip(1).enumerate() {
            let (p_file, p_line, p_col) = if let Some(pf) = sm_guard.get_file(p_span.file_id) {
                let (l, c) = pf.line_col(p_span.start);
                (pf.path.to_string_lossy().to_string(), l, c)
            } else {
                ("<berkas>".to_string(), 1, 1)
            };
            let _ = writeln!(
                self.writer,
                "  #{} {} pada {}:{}:{}",
                idx + 1,
                p_fn.yellow(),
                p_file,
                p_line,
                p_col
            );
        }
    }

    fn print_breakpoints(&mut self) {
        let _ = writeln!(
            self.writer,
            "{}",
            "Titik Henti Aktif (Active Breakpoints):".cyan().bold()
        );
        let mut found = false;
        for (path, lines) in &self.breakpoints {
            if lines.is_empty() {
                continue;
            }
            found = true;
            let mut sorted: Vec<usize> = lines.iter().copied().collect();
            sorted.sort();
            for l in sorted {
                let _ = writeln!(self.writer, "  - {}:{}", path, l.to_string().yellow().bold());
            }
        }
        if !found {
            let _ = writeln!(
                self.writer,
                "  {}",
                "(tidak ada titik henti yang dipasang)".dimmed()
            );
        }
    }

    fn handle_add_breakpoint(&mut self, spec: &str, current_file: &str) {
        let (target_file, line_num) = if let Some((f, l_str)) = spec.split_once(':') {
            if let Ok(l) = l_str.parse::<usize>() {
                (f.to_string(), l)
            } else {
                let _ = writeln!(self.writer, "Nomor baris tidak valid: '{}'", l_str);
                return;
            }
        } else if let Ok(l) = spec.parse::<usize>() {
            (current_file.to_string(), l)
        } else {
            let _ = writeln!(
                self.writer,
                "Format titik henti tidak valid: '{}'. Gunakan 'b <baris>' atau 'b <berkas>:<baris>'",
                spec
            );
            return;
        };

        let norm = normalize_path(&target_file);
        self.breakpoints.entry(norm).or_default().insert(line_num);
        let _ = writeln!(
            self.writer,
            "{} Titik henti ditambahkan pada {}:{}",
            "✓".green().bold(),
            target_file.cyan(),
            line_num.to_string().yellow().bold()
        );
    }

    fn print_help(&mut self) {
        let _ = writeln!(
            self.writer,
            "{}",
            "=== Panduan Perintah Awakutu DapCode (dap-debug) ===".cyan().bold()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {} / {}  : Langkah masuk ke pernyataan berikutnya (step into)",
            "s".green().bold(),
            "step".green(),
            "langkah".green()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {} / {}   : Langkah lewati fungsi/pernyataan (step over)",
            "n".green().bold(),
            "next".green(),
            "lewati".green()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {} / {}  : Lanjutkan eksekusi hingga titik henti atau selesai",
            "c".green().bold(),
            "continue".green(),
            "terus".green()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {}          : Langkah keluar dari fungsi saat ini (step out)",
            "out".green().bold(),
            "keluar_fungsi".green()
        );
        let _ = writeln!(
            self.writer,
            "  {} [baris]           : Tambah titik henti (contoh: b 15 atau b modul.dap:20)",
            "b".green().bold()
        );
        let _ = writeln!(
            self.writer,
            "  {} <baris>           : Hapus titik henti pada baris tertentu",
            "hb".green().bold()
        );
        let _ = writeln!(
            self.writer,
            "  {}                   : Tampilkan seluruh titik henti aktif",
            "lb".green().bold()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {} / {}      : Tampilkan semua variabel lokal beserta tipenya",
            "v".green().bold(),
            "vars".green(),
            "variabel".green()
        );
        let _ = writeln!(
            self.writer,
            "  {} <ekspresi>        : Cetak/evaluasi ekspresi atau variabel",
            "p".green().bold()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {}          : Tampilkan jejak panggilan fungsi (call stack)",
            "bt".green().bold(),
            "backtrace".green()
        );
        let _ = writeln!(
            self.writer,
            "  {} [baris]           : Tampilkan cuplikan kode sumber",
            "l".green().bold()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {}            : Tampilkan panduan bantuan ini",
            "h".green().bold(),
            "bantuan".green()
        );
        let _ = writeln!(
            self.writer,
            "  {} / {}           : Hentikan debugging dan keluar",
            "q".green().bold(),
            "keluar".green()
        );
        let _ = writeln!(
            self.writer,
            "  {}                   : Tekan [Enter] tanpa input untuk mengulang perintah langkah sebelumnya\n",
            "[Enter]".dimmed()
        );
    }
}

impl DebugHook for CliDebugger {
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
        let mut reason = String::new();

        if self.stop_on_entry && !self.has_stopped_on_entry {
            self.has_stopped_on_entry = true;
            should_stop = true;
            reason = "henti awal (stop-on-entry)".to_string();
        } else if let Some(lines) = self.breakpoints.get(&norm_path) {
            if lines.contains(&line) {
                should_stop = true;
                reason = format!("titik henti pada baris {}", line);
            }
        }

        if !should_stop {
            match self.stepping_mode {
                SteppingMode::StepIn => {
                    should_stop = true;
                    reason = "langkah masuk (step-in)".to_string();
                }
                SteppingMode::StepOver { target_depth } => {
                    if call_stack.len() <= target_depth {
                        should_stop = true;
                        reason = "langkah lewati (step-over)".to_string();
                    }
                }
                SteppingMode::StepOut { target_depth } => {
                    if call_stack.len() < target_depth {
                        should_stop = true;
                        reason = "langkah keluar fungsi (step-out)".to_string();
                    }
                }
                SteppingMode::None => {}
            }
        }

        if !should_stop {
            return DebugAction::Continue;
        }

        // Reset stepping mode once paused
        self.stepping_mode = SteppingMode::None;

        // Print stop info
        let _ = writeln!(
            self.writer,
            "{} Berhenti karena {} pada baris {}:{} (fungsi '{}')",
            "[AWAKUTU]".yellow().bold(),
            reason.cyan(),
            line,
            col,
            fn_name.green().bold()
        );

        // Display source snippet
        self.print_source_snippet(&file_path, line, 2);

        // Interactive command loop
        loop {
            let _ = write!(self.writer, "{}", "dap-debug> ".green().bold());
            let _ = self.writer.flush();

            let mut input = String::new();
            match self.reader.read_line(&mut input) {
                Ok(0) => {
                    // EOF
                    let _ = writeln!(self.writer, "\n{}", "Sesi awakutu dihentikan.".dimmed());
                    return DebugAction::Terminate;
                }
                Ok(_) => {}
                Err(_) => {
                    return DebugAction::Terminate;
                }
            }

            let trimmed = input.trim();
            let cmd_str = if trimmed.is_empty() {
                match &self.last_command {
                    Some(prev) => prev.clone(),
                    None => continue,
                }
            } else {
                trimmed.to_string()
            };

            let parts: Vec<&str> = cmd_str.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let primary_cmd = parts[0].to_lowercase();
            let args = &parts[1..];

            match primary_cmd.as_str() {
                // 1. Step in
                "s" | "step" | "langkah" | "masuk" => {
                    self.last_command = Some(primary_cmd);
                    self.stepping_mode = SteppingMode::StepIn;
                    return DebugAction::Continue;
                }
                // 2. Step over
                "n" | "next" | "lewati" | "lanjutkan" => {
                    self.last_command = Some(primary_cmd);
                    self.stepping_mode = SteppingMode::StepOver {
                        target_depth: call_stack.len(),
                    };
                    return DebugAction::Continue;
                }
                // 3. Continue
                "c" | "cont" | "continue" | "terus" | "jalan" => {
                    self.last_command = Some(primary_cmd);
                    self.stepping_mode = SteppingMode::None;
                    return DebugAction::Continue;
                }
                // 4. Step out
                "out" | "fin" | "finish" | "keluar_fungsi" => {
                    self.last_command = Some(primary_cmd);
                    self.stepping_mode = SteppingMode::StepOut {
                        target_depth: call_stack.len(),
                    };
                    return DebugAction::Continue;
                }
                // 5. Breakpoint (b / break / titik_henti)
                "b" | "break" | "titik_henti" => {
                    if args.is_empty() {
                        self.print_breakpoints();
                    } else {
                        self.handle_add_breakpoint(args[0], &file_path);
                    }
                }
                // 6. Clear breakpoint (hb / clear / hapus / hapus_titik_henti)
                "hb" | "clear" | "hapus" | "hapus_titik_henti" => {
                    if args.is_empty() {
                        let _ = writeln!(self.writer, "Format: hb <nomor_baris>");
                    } else if let Ok(l) = args[0].parse::<usize>() {
                        let norm = normalize_path(&file_path);
                        if let Some(lines) = self.breakpoints.get_mut(&norm) {
                            if lines.remove(&l) {
                                let _ = writeln!(
                                    self.writer,
                                    "{} Titik henti pada baris {} dihapus.",
                                    "✓".green().bold(),
                                    l
                                );
                            } else {
                                let _ = writeln!(
                                    self.writer,
                                    "{} Tidak ada titik henti aktif pada baris {}.",
                                    "!".yellow().bold(),
                                    l
                                );
                            }
                        }
                    } else {
                        let _ = writeln!(self.writer, "Nomor baris harus berupa angka positif.");
                    }
                }
                // 7. List breakpoints
                "lb" | "breakpoints" | "daftar_titik_henti" => {
                    self.print_breakpoints();
                }
                // 8. Local variables
                "v" | "vars" | "variabel" => {
                    self.print_variables(env);
                }
                // 9. Print / evaluate expression
                "p" | "print" | "cetak" | "eval" => {
                    if args.is_empty() {
                        let _ = writeln!(self.writer, "Format: p <ekspresi atau nama_variabel>");
                    } else {
                        let expr_str = cmd_str[parts[0].len()..].trim();
                        self.evaluate_and_print(expr_str, span, env);
                    }
                }
                // 10. Call stack backtrace
                "bt" | "backtrace" | "jejak" => {
                    self.print_backtrace(fn_name, &file_path, line, col, call_stack);
                }
                // 11. List source lines
                "l" | "list" | "lihat" => {
                    self.last_command = Some(primary_cmd);
                    let target_l = if let Some(arg) = args.first() {
                        arg.parse::<usize>().unwrap_or(line)
                    } else {
                        line
                    };
                    self.print_source_snippet(&file_path, target_l, 5);
                }
                // 12. Help
                "h" | "help" | "bantuan" | "?" | ":bantuan" => {
                    self.print_help();
                }
                // 13. Quit
                "q" | "quit" | "keluar" | "exit" | ":keluar" => {
                    let _ = writeln!(self.writer, "{}", "Sesi awakutu dihentikan.".yellow());
                    return DebugAction::Terminate;
                }
                _ => {
                    let _ = writeln!(
                        self.writer,
                        "Perintah '{}' tidak dikenal. Ketik '{}' untuk daftar perintah.",
                        cmd_str.red(),
                        "h".green().bold()
                    );
                }
            }
        }
    }

    fn on_output(&mut self, text: &str) {
        let _ = write!(self.writer, "{}", text);
        let _ = self.writer.flush();
    }
}
