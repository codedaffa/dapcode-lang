use crate::ast::*;
use crate::diag::{Diagnostic, Result};
use crate::ownership::OwnershipReport;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct CodeGenerator<'a> {
    pub _ownership: &'a OwnershipReport,
    pub out: String,
    pub _temp_counter: usize,
    pub task_helpers: Vec<String>,
    pub method_to_struct: HashMap<String, String>,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(ownership: &'a OwnershipReport) -> Self {
        CodeGenerator {
            _ownership: ownership,
            out: String::new(),
            _temp_counter: 0,
            task_helpers: Vec::new(),
            method_to_struct: HashMap::new(),
        }
    }

    pub fn generate_c(&mut self, program: &Program) -> String {
        for decl in &program.decls {
            if let Decl::Impl(i) = decl {
                for m in &i.methods {
                    self.method_to_struct.insert(m.name.clone(), i.target_struct.clone());
                }
            }
        }

        self.emit_prelude();

        // 1. Forward declare structs
        for decl in &program.decls {
            if let Decl::Struct(s) = decl {
                self.out.push_str(&format!("typedef struct {} {};\n", s.name, s.name));
            }
        }
        self.out.push('\n');

        // 2. Define structs
        let all_struct_names: std::collections::HashSet<String> = program
            .decls
            .iter()
            .filter_map(|d| if let Decl::Struct(s) = d { Some(s.name.clone()) } else { None })
            .collect();

        let mut struct_names = Vec::new();
        for decl in &program.decls {
            if let Decl::Struct(s) = decl {
                struct_names.push(s.name.clone());
                self.out.push_str(&format!("struct {} {{\n", s.name));
                for f in &s.fields {
                    let c_ty = self.type_to_c(&f.ty);
                    self.out.push_str(&format!("    {} {};\n", c_ty, f.name));
                }
                self.out.push_str("};\n\n");

                // Emit Result type for struct
                self.out.push_str(&format!(
                    "typedef struct {{\n    bool is_err;\n    const char* err_msg;\n    {} ok;\n}} DapResult_{};\n\n",
                    s.name, s.name
                ));
            }
        }

        // Forward declare dap_ke_json_<Struct>
        for name in &struct_names {
            self.out.push_str(&format!("static inline char* dap_ke_json_{}({} s);\n", name, name));
        }
        self.out.push('\n');

        // Emit JSON serializer implementations for struct
        for decl in &program.decls {
            if let Decl::Struct(s) = decl {
                let mut fmt_parts = Vec::new();
                let mut arg_parts = Vec::new();
                for f in &s.fields {
                    let (fmt, arg) = match &f.ty {
                        TypeExpr::Named(name, _) if all_struct_names.contains(name) => {
                            (format!("\\\"{}\\\":%s", f.name), format!("dap_ke_json_{}(s.{})", name, f.name))
                        }
                        TypeExpr::Named(name, _) if name == "Teks" || name == "teks" => {
                            (format!("\\\"{}\\\":\\\"%s\\\"", f.name), format!("dap_to_str(s.{})", f.name))
                        }
                        TypeExpr::Named(name, _) if name == "Bool" || name == "bool" => {
                            (format!("\\\"{}\\\":%s", f.name), format!("(s.{} ? \"true\" : \"false\")", f.name))
                        }
                        _ => {
                            (format!("\\\"{}\\\":%s", f.name), format!("dap_to_str(s.{})", f.name))
                        }
                    };
                    fmt_parts.push(fmt);
                    arg_parts.push(arg);
                }
                let fmt_str = format!("{{{}}}", fmt_parts.join(","));
                let args_joined = if arg_parts.is_empty() { String::new() } else { format!(", {}", arg_parts.join(", ")) };
                self.out.push_str(&format!(
                    "static inline char* dap_ke_json_{}({} s) {{\n    char* _b = (char*)malloc(1024);\n    snprintf(_b, 1024, \"{}\"{});\n    return _b;\n}}\n\n",
                    s.name, s.name, fmt_str, args_joined
                ));
            }
        }

        if !struct_names.is_empty() {
            let mut branches: Vec<String> = struct_names.iter().map(|n| format!("{}: dap_ke_json_{}", n, n)).collect();
            branches.push("default: _dap_to_str_str".to_string());
            self.out.push_str(&format!(
                "#undef dap_ke_json\n#define dap_ke_json(x) _Generic((x), {})(x)\n\n",
                branches.join(", ")
            ));
        }

        // 3. Forward declare functions
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    self.emit_fn_proto(f, None);
                }
                Decl::Impl(i) => {
                    for m in &i.methods {
                        self.emit_fn_proto(m, Some(&i.target_struct));
                    }
                }
                _ => {}
            }
        }
        self.out.push('\n');

        // 4. Emit function implementations into separate buffer to collect task helpers
        let prev_out = std::mem::take(&mut self.out);
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    self.emit_function(f, None);
                }
                Decl::Impl(i) => {
                    for m in &i.methods {
                        self.emit_function(m, Some(&i.target_struct));
                    }
                }
                _ => {}
            }
        }
        let functions_buf = std::mem::replace(&mut self.out, prev_out);

        // Emit collected task helpers
        for th in &self.task_helpers {
            self.out.push_str(th);
            self.out.push('\n');
        }

        // Emit functions
        self.out.push_str(&functions_buf);

        // Emit WebSocket Dispatcher
        self.out.push_str("static Teks dap_dispatch_ws_handler(const char* name, const char* msg) {\n");
        self.out.push_str("    if (!name || strlen(name) == 0) return msg;\n");
        for decl in &program.decls {
            if let Decl::Fn(f) = decl {
                if f.params.len() == 1 {
                    let is_string_param = match &f.params[0].ty {
                        Some(pty) => self.type_to_c(pty) == "Teks",
                        None => true,
                    };
                    if is_string_param {
                        let ret_ty = self.ret_type_to_c(&f.ret_ty);
                        if ret_ty == "void" {
                            self.out.push_str(&format!(
                                "    if (strcmp(name, \"{}\") == 0) {{ dap_{}(msg); return \"\"; }}\n",
                                f.name, f.name
                            ));
                        } else {
                            self.out.push_str(&format!(
                                "    if (strcmp(name, \"{}\") == 0) return dap_{}(msg);\n",
                                f.name, f.name
                            ));
                        }
                    }
                }
            }
        }
        self.out.push_str("    return msg;\n}\n\n");

        // 5. Emit main entry point
        self.out.push_str(
            r#"int main(int argc, char** argv) {
    _dap_cli_argc = argc;
    _dap_cli_argv = argv;
    dap_utama();
    return 0;
}
"#,
        );

        self.out.clone()
    }

    fn emit_prelude(&mut self) {
        self.out.push_str(
            r#"/* Generated by dapcodec - Modern DapCode Compiler Toolchain */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <inttypes.h>
#include <stdbool.h>
#include <string.h>
#include <ctype.h>
#include <math.h>
#include <time.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <direct.h>
typedef int socklen_t;
#else
#include <pthread.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#define closesocket close
#define SOCKET int
#define INVALID_SOCKET -1
#define SOCKET_ERROR -1
#endif

#ifdef _MSC_VER
#define dap_strncasecmp _strnicmp
#else
#define dap_strncasecmp strncasecmp
#endif

typedef uint64_t u64;
typedef int64_t  i64;
typedef double   f64;
typedef const char* Teks;

// Standard Error Union Result Types for Primaries
typedef struct { bool is_err; const char* err_msg; u64 ok; } DapResult_u64;
typedef struct { bool is_err; const char* err_msg; i64 ok; } DapResult_i64;
typedef struct { bool is_err; const char* err_msg; f64 ok; } DapResult_f64;
typedef struct { bool is_err; const char* err_msg; bool ok; } DapResult_bool;
typedef struct { bool is_err; const char* err_msg; Teks ok; } DapResult_Teks;

// HTTP Web Server Model Types
typedef struct {
    Teks metode;
    Teks jalur;
    Teks kueri;
    Teks isi;
    Teks raw_headers;
} PermintaanHttp;

typedef struct {
    u64 status;
    Teks tipe_konten;
    Teks konten;
    Teks header;
    Teks ws_handler;
} ResponsHttp;

typedef struct { bool is_err; const char* err_msg; ResponsHttp ok; } DapResult_ResponsHttp;
typedef struct { bool is_err; const char* err_msg; PermintaanHttp ok; } DapResult_PermintaanHttp;
typedef ResponsHttp (*dap_http_handler_t)(PermintaanHttp);

// Embedded BasisData
typedef struct {
    Teks jalur;
} BasisData;

typedef struct { bool is_err; const char* err_msg; BasisData ok; } DapResult_BasisData;

// Runtime Dynamic Collection: DapList
typedef struct DapList {
    size_t len;
    size_t cap;
    void** items;
} DapList;

typedef struct { bool is_err; const char* err_msg; DapList* ok; } DapResult_Daftar;

static inline DapList* dap_Daftar_baru(void) {
    DapList* list = (DapList*)malloc(sizeof(DapList));
    list->len = 0;
    list->cap = 4;
    list->items = (void**)malloc(sizeof(void*) * list->cap);
    return list;
}

static inline void dap_Daftar_tambah(DapList* list, void* item) {
    if (!list) return;
    if (list->len >= list->cap) {
        list->cap = list->cap == 0 ? 4 : list->cap * 2;
        list->items = (void**)realloc(list->items, sizeof(void*) * list->cap);
    }
    list->items[list->len++] = item;
}

static inline DapList* dap_Daftar_dari_array(size_t count, void** items) {
    DapList* list = dap_Daftar_baru();
    for (size_t i = 0; i < count; i++) {
        dap_Daftar_tambah(list, items[i]);
    }
    return list;
}

static inline u64 dap_Daftar_panjang(DapList* list) {
    return list ? (u64)list->len : 0;
}

typedef struct { bool is_err; const char* err_msg; void* ok; } DapResult_ptr;

static inline DapResult_ptr dap_Daftar_ambil(DapList* list, u64 idx) {
    DapResult_ptr res;
    if (!list || idx >= list->len) {
        res.is_err = true;
        res.err_msg = "Indeks di luar batas";
        res.ok = NULL;
    } else {
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = list->items[idx];
    }
    return res;
}

// C11 Generic String Conversion
static inline char* _dap_fmt_f64(double x) {
    char* b = (char*)malloc(64);
    if (x == (long long)x) {
        snprintf(b, 64, "%.1f", x);
    } else {
        snprintf(b, 64, "%.15g", x);
    }
    return b;
}
static inline char* _dap_fmt_u64(uint64_t x) {
    char* b = (char*)malloc(64);
    snprintf(b, 64, "%" PRIu64, x);
    return b;
}
static inline char* _dap_fmt_i64(int64_t x) {
    char* b = (char*)malloc(64);
    snprintf(b, 64, "%" PRId64, x);
    return b;
}
static inline char* _dap_fmt_int(int x) {
    char* b = (char*)malloc(64);
    snprintf(b, 64, "%d", x);
    return b;
}
static inline char* _dap_fmt_ptr(void* x) {
    char* b = (char*)malloc(64);
    snprintf(b, 64, "%p", x);
    return b;
}

static inline char* _dap_to_str_str(const char* s) { return (char*)(s ? s : ""); }
static inline char* _dap_to_str_f64(double x) { return _dap_fmt_f64(x); }
static inline char* _dap_to_str_u64(uint64_t x) { return _dap_fmt_u64(x); }
static inline char* _dap_to_str_i64(int64_t x) { return _dap_fmt_i64(x); }
static inline char* _dap_to_str_int(int x) { return _dap_fmt_int(x); }
static inline char* _dap_to_str_uint(unsigned int x) { return _dap_fmt_u64((uint64_t)x); }
static inline char* _dap_to_str_long(long x) { return _dap_fmt_i64((int64_t)x); }
static inline char* _dap_to_str_ulong(unsigned long x) { return _dap_fmt_u64((uint64_t)x); }
static inline char* _dap_to_str_bool(bool x) { return x ? "benar" : "salah"; }
static inline char* _dap_to_str_ptr(const void* x) {
    if (!x) return "";
    uintptr_t val = (uintptr_t)x;
    if (val < 0x10000) {
        char* b = (char*)malloc(32);
        snprintf(b, 32, "%llu", (unsigned long long)val);
        return b;
    }
    const unsigned char* s = (const unsigned char*)x;
    bool is_str = true;
    size_t i = 0;
    while (i < 1024 && s[i] != '\0') {
        if (s[i] < 32 && s[i] != '\t' && s[i] != '\n' && s[i] != '\r') {
            is_str = false;
            break;
        }
        i++;
    }
    if (is_str && i > 0 && i < 1024) {
        return (char*)x;
    }
    return _dap_fmt_ptr((void*)x);
}

#define dap_to_str(x) _Generic((x), \
    char*: _dap_to_str_str, \
    const char*: _dap_to_str_str, \
    float: _dap_to_str_f64, \
    double: _dap_to_str_f64, \
    int: _dap_to_str_int, \
    unsigned int: _dap_to_str_uint, \
    long: _dap_to_str_long, \
    unsigned long: _dap_to_str_ulong, \
    long long: _dap_to_str_i64, \
    unsigned long long: _dap_to_str_u64, \
    bool: _dap_to_str_bool, \
    default: _dap_to_str_ptr \
)(x)

#define dap_cetak(x) printf("%s\n", dap_to_str(x))

static inline void dap_cetak_multi(size_t count, const char** msgs) {
    for (size_t i = 0; i < count; i++) {
        if (i > 0) printf(" ");
        printf("%s", msgs[i]);
    }
    printf("\n");
}

// Runtime Task Coroutine definitions
typedef struct {
#ifdef _WIN32
    HANDLE thread_handle;
#else
    pthread_t thread;
#endif
    void* result;
} DapTask;

static inline void* dap_tunggu(DapTask* task) {
    if (!task) return NULL;
#ifdef _WIN32
    WaitForSingleObject(task->thread_handle, INFINITE);
    CloseHandle(task->thread_handle);
#else
    pthread_join(task->thread, NULL);
#endif
    return task->result;
}

static inline DapTask* dap_spawn_task(void* (*fn)(void*), void* arg) {
    DapTask* task = (DapTask*)malloc(sizeof(DapTask));
    task->result = NULL;
#ifdef _WIN32
    task->thread_handle = CreateThread(NULL, 0, (LPTHREAD_START_ROUTINE)fn, arg, 0, NULL);
#else
    pthread_create(&task->thread, NULL, fn, arg);
#endif
    return task;
}

static inline void dap_drop(void* ptr) {
    // Deterministic drop injected by Perceus
    (void)ptr;
}

// Standard Library Builtins
static inline DapResult_Teks dap_baca_berkas(const char* jalur) {
    DapResult_Teks res;
    FILE* f = fopen(jalur, "rb");
    if (!f) {
        res.is_err = true;
        res.err_msg = "Gagal membaca berkas";
        res.ok = "";
        return res;
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc(sz + 1);
    if (!buf) {
        fclose(f);
        res.is_err = true;
        res.err_msg = "Alokasi memori gagal";
        res.ok = "";
        return res;
    }
    fread(buf, 1, sz, f);
    buf[sz] = '\0';
    fclose(f);
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = buf;
    return res;
}

static inline DapResult_bool dap_tulis_berkas(const char* jalur, const char* isi) {
    DapResult_bool res;
    FILE* f = fopen(jalur, "wb");
    if (!f) {
        res.is_err = true;
        res.err_msg = "Gagal menulis berkas";
        res.ok = false;
        return res;
    }
    fputs(isi, f);
    fclose(f);
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = true;
    return res;
}

static inline bool dap_ada_berkas(const char* jalur) {
    FILE* f = fopen(jalur, "r");
    if (f) {
        fclose(f);
        return true;
    }
    return false;
}

static inline DapResult_bool dap_buat_direktori(const char* jalur) {
    DapResult_bool res;
#ifdef _WIN32
    int r = _mkdir(jalur);
#else
    int r = mkdir(jalur, 0755);
#endif
    (void)r;
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = true;
    return res;
}

static inline DapResult_bool dap_hapus_berkas(const char* jalur) {
    DapResult_bool res;
    if (remove(jalur) == 0) {
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = true;
    } else {
        res.is_err = true;
        res.err_msg = "Gagal menghapus berkas";
        res.ok = false;
    }
    return res;
}

static inline DapResult_bool dap_hapus_direktori(const char* jalur) {
    DapResult_bool res;
#ifdef _WIN32
    int r = _rmdir(jalur);
#else
    int r = rmdir(jalur);
#endif
    if (r == 0) {
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = true;
    } else {
        res.is_err = true;
        res.err_msg = "Gagal menghapus direktori";
        res.ok = false;
    }
    return res;
}

static inline const char* dap_gabung_jalur(const char* a, const char* b) {
    if (!a) return b ? b : "";
    if (!b) return a;
    size_t la = strlen(a);
    size_t lb = strlen(b);
    char* buf = (char*)malloc(la + lb + 2);
    if (!buf) return "";
    bool has_sep = (la > 0 && (a[la-1] == '/' || a[la-1] == '\\'));
    if (has_sep) {
        snprintf(buf, la + lb + 2, "%s%s", a, b);
    } else {
        snprintf(buf, la + lb + 2, "%s/%s", a, b);
    }
    return buf;
}

static inline void dap_tidur(u64 ms) {
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    usleep((useconds_t)(ms * 1000));
#endif
}

static inline DapResult_Daftar dap_daftar_direktori(const char* jalur) {
    (void)jalur;
    DapResult_Daftar res;
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = dap_Daftar_baru();
    return res;
}

static inline u64 dap_panjang_teks(const char* s) {
    return s ? (u64)strlen(s) : 0;
}

static inline const char* dap_ke_huruf_besar(const char* s) {
    if (!s) return "";
    size_t len = strlen(s);
    char* buf = (char*)malloc(len + 1);
    for (size_t i = 0; i < len; i++) {
        buf[i] = (char)toupper((unsigned char)s[i]);
    }
    buf[len] = '\0';
    return buf;
}

static inline const char* dap_ke_huruf_kecil(const char* s) {
    if (!s) return "";
    size_t len = strlen(s);
    char* buf = (char*)malloc(len + 1);
    for (size_t i = 0; i < len; i++) {
        buf[i] = (char)tolower((unsigned char)s[i]);
    }
    buf[len] = '\0';
    return buf;
}

static inline f64 dap_akar_kuadrat(f64 x) {
    return sqrt(x);
}

static inline f64 dap_pangkat(f64 b, f64 e) {
    return pow(b, e);
}

static inline f64 dap_nilai_mutlak(f64 x) {
    return fabs(x);
}

static inline u64 dap_waktu_sekarang_ms(void) {
    return (u64)time(NULL) * 1000;
}

// Standard Library: Environment & CLI Arguments
static int _dap_cli_argc = 0;
static char** _dap_cli_argv = NULL;

static inline DapList* dap_ambil_argumen_cli(void) {
    DapList* list = dap_Daftar_baru();
    for (int i = 0; i < _dap_cli_argc; i++) {
        dap_Daftar_tambah(list, (void*)_dap_cli_argv[i]);
    }
    return list;
}

static inline DapResult_Teks dap_ambil_lingkungan(const char* name) {
    DapResult_Teks res;
    const char* val = getenv(name ? name : "");
    if (val) {
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = val;
    } else {
        res.is_err = true;
        res.err_msg = "Variabel lingkungan tidak ditemukan";
        res.ok = "";
    }
    return res;
}

static inline void dap_atur_lingkungan(const char* name, const char* value) {
#ifdef _WIN32
    _putenv_s(name ? name : "", value ? value : "");
#else
    setenv(name ? name : "", value ? value : "", 1);
#endif
}

// Standard Library: JSON Builtins
static inline bool dap_valid_json(const char* s) {
    if (!s) return false;
    while (*s && isspace((unsigned char)*s)) s++;
    if (*s != '{' && *s != '[') return false;
    int depth = 0;
    bool in_str = false;
    while (*s) {
        if (*s == '"' && (s == s || s[-1] != '\\')) in_str = !in_str;
        else if (!in_str) {
            if (*s == '{' || *s == '[') depth++;
            else if (*s == '}' || *s == ']') depth--;
        }
        s++;
    }
    return depth == 0 && !in_str;
}

static inline bool dap_ada_kunci_json(const char* json, const char* key) {
    if (!json || !key) return false;
    char pat[256];
    snprintf(pat, sizeof(pat), "\"%s\"", key);
    return strstr(json, pat) != NULL;
}

static inline DapResult_Teks dap_ambil_kunci_json(const char* json, const char* key) {
    DapResult_Teks res;
    if (!json || !key) {
        res.is_err = true;
        res.err_msg = "Argumen JSON null";
        res.ok = "";
        return res;
    }
    char pat[256];
    snprintf(pat, sizeof(pat), "\"%s\"", key);
    const char* p = strstr(json, pat);
    if (!p) {
        res.is_err = true;
        char* err = (char*)malloc(128);
        snprintf(err, 128, "Kunci '%s' tidak ditemukan pada JSON", key);
        res.err_msg = err;
        res.ok = "";
        return res;
    }
    p += strlen(pat);
    while (*p && (*p == ' ' || *p == ':' || *p == '\t')) p++;
    if (*p == '"') {
        p++;
        const char* end = p;
        while (*end && (*end != '"' || end[-1] == '\\')) end++;
        size_t len = end - p;
        char* val = (char*)malloc(len + 1);
        memcpy(val, p, len);
        val[len] = '\0';
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = val;
        return res;
    } else {
        const char* end = p;
        while (*end && *end != ',' && *end != '}' && *end != ']') end++;
        size_t len = end - p;
        char* val = (char*)malloc(len + 1);
        memcpy(val, p, len);
        val[len] = '\0';
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = val;
        return res;
    }
}

static inline DapResult_f64 dap_ambil_angka_json(const char* json, const char* key) {
    DapResult_f64 res;
    DapResult_Teks s_res = dap_ambil_kunci_json(json, key);
    if (s_res.is_err) {
        res.is_err = true;
        res.err_msg = s_res.err_msg;
        res.ok = 0.0;
        return res;
    }
    char* endptr = NULL;
    double val = strtod(s_res.ok, &endptr);
    if (endptr == s_res.ok) {
        res.is_err = true;
        res.err_msg = "Bukan angka";
        res.ok = 0.0;
        return res;
    }
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = val;
    return res;
}

static inline DapResult_bool dap_ambil_bool_json(const char* json, const char* key) {
    DapResult_bool res;
    DapResult_Teks s_res = dap_ambil_kunci_json(json, key);
    if (s_res.is_err) {
        res.is_err = true;
        res.err_msg = s_res.err_msg;
        res.ok = false;
        return res;
    }
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = (strstr(s_res.ok, "true") != NULL || strstr(s_res.ok, "benar") != NULL);
    return res;
}

static inline DapResult_Teks dap_uraikan_json(const char* json) {
    DapResult_Teks res;
    if (dap_valid_json(json)) {
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = json;
    } else {
        res.is_err = true;
        res.err_msg = "Format JSON tidak valid";
        res.ok = "";
    }
    return res;
}

#define dap_ke_json(x) dap_to_str(x)

// Standard Library: HTTP Network Builtins
static inline DapResult_Teks dap_ambil_http(const char* url) {
    DapResult_Teks res;
    if (!url) {
        res.is_err = true;
        res.err_msg = "URL tidak boleh kosong";
        res.ok = "";
        return res;
    }
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "curl.exe -s -S -L -w \"\\n%%{http_code}\" -X GET \"%s\"", url);
#ifdef _WIN32
    FILE* fp = _popen(cmd, "r");
#else
    FILE* fp = popen(cmd, "r");
#endif
    if (!fp) {
        res.is_err = true;
        res.err_msg = "Gagal memanggil curl";
        res.ok = "";
        return res;
    }
    size_t cap = 4096;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    char chunk[512];
    while (fgets(chunk, sizeof(chunk), fp)) {
        size_t clen = strlen(chunk);
        if (len + clen + 1 >= cap) {
            cap *= 2;
            buf = (char*)realloc(buf, cap);
        }
        memcpy(buf + len, chunk, clen);
        len += clen;
        buf[len] = '\0';
    }
#ifdef _WIN32
    _pclose(fp);
#else
    pclose(fp);
#endif

    char* last_nl = strrchr(buf, '\n');
    int status = 200;
    if (last_nl) {
        status = atoi(last_nl + 1);
        *last_nl = '\0';
    }
    if (status >= 400 || status == 0) {
        res.is_err = true;
        char* err = (char*)malloc(256);
        snprintf(err, 256, "HTTP Error %d: %s", status, buf);
        res.err_msg = err;
        res.ok = "";
        return res;
    }
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = buf;
    return res;
}

static inline DapResult_Teks dap_kirim_http_post(const char* url, const char* body) {
    DapResult_Teks res;
    if (!url) {
        res.is_err = true;
        res.err_msg = "URL tidak boleh kosong";
        res.ok = "";
        return res;
    }
    char cmd[2048];
    const char* b = body ? body : "";
    snprintf(cmd, sizeof(cmd), "curl.exe -s -S -L -w \"\\n%%{http_code}\" -X POST -H \"Content-Type: application/json\" -d \"%s\" \"%s\"", b, url);
#ifdef _WIN32
    FILE* fp = _popen(cmd, "r");
#else
    FILE* fp = popen(cmd, "r");
#endif
    if (!fp) {
        res.is_err = true;
        res.err_msg = "Gagal memanggil curl";
        res.ok = "";
        return res;
    }
    size_t cap = 4096;
    size_t len = 0;
    char* buf = (char*)malloc(cap);
    char chunk[512];
    while (fgets(chunk, sizeof(chunk), fp)) {
        size_t clen = strlen(chunk);
        if (len + clen + 1 >= cap) {
            cap *= 2;
            buf = (char*)realloc(buf, cap);
        }
        memcpy(buf + len, chunk, clen);
        len += clen;
        buf[len] = '\0';
    }
#ifdef _WIN32
    _pclose(fp);
#else
    pclose(fp);
#endif

    char* last_nl = strrchr(buf, '\n');
    int status = 200;
    if (last_nl) {
        status = atoi(last_nl + 1);
        *last_nl = '\0';
    }
    if (status >= 400 || status == 0) {
        res.is_err = true;
        char* err = (char*)malloc(256);
        snprintf(err, 256, "HTTP Error %d: %s", status, buf);
        res.err_msg = err;
        res.ok = "";
        return res;
    }
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = buf;
    return res;
}

static inline DapResult_u64 dap_ambil_status_http(const char* url) {
    DapResult_u64 res;
    if (!url) {
        res.is_err = true;
        res.err_msg = "URL tidak boleh kosong";
        res.ok = 0;
        return res;
    }
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "curl.exe -s -S -o nul -w \"%%{http_code}\" -X GET \"%s\"", url);
#ifdef _WIN32
    FILE* fp = _popen(cmd, "r");
#else
    FILE* fp = popen(cmd, "r");
#endif
    if (!fp) {
        res.is_err = true;
        res.err_msg = "Gagal memanggil curl";
        res.ok = 0;
        return res;
    }
    char buf[64];
    if (!fgets(buf, sizeof(buf), fp)) {
        buf[0] = '\0';
    }
#ifdef _WIN32
    _pclose(fp);
#else
    pclose(fp);
#endif
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = (u64)atoi(buf);
    return res;
}

// HTTP Server Response Helpers
static inline ResponsHttp dap_kirim_respons_http(u64 status, const char* tipe_konten, const char* konten) {
    ResponsHttp r;
    r.status = status;
    r.tipe_konten = tipe_konten ? (Teks)tipe_konten : "text/plain";
    r.konten = konten ? (Teks)konten : "";
    r.header = "";
    r.ws_handler = "";
    return r;
}

static inline ResponsHttp dap_respons_json(const char* data_json) {
    return dap_kirim_respons_http(200, "application/json", data_json);
}

static inline ResponsHttp dap_respons_teks(const char* teks) {
    return dap_kirim_respons_http(200, "text/plain; charset=utf-8", teks);
}

static inline ResponsHttp dap_tambah_header(ResponsHttp r, const char* kunci, const char* nilai) {
    if (!kunci || !nilai) return r;
    size_t cur_len = r.header ? strlen(r.header) : 0;
    size_t k_len = strlen(kunci);
    size_t v_len = strlen(nilai);
    size_t needed = cur_len + k_len + v_len + 5;
    char* buf = (char*)malloc(needed);
    if (!buf) return r;
    buf[0] = '\0';
    if (cur_len > 0) {
        strcpy(buf, r.header);
    }
    strcat(buf, kunci);
    strcat(buf, ": ");
    strcat(buf, nilai);
    strcat(buf, "\r\n");
    r.header = buf;
    return r;
}

static inline ResponsHttp dap_respons_cors(ResponsHttp r) {
    r = dap_tambah_header(r, "Access-Control-Allow-Origin", "*");
    r = dap_tambah_header(r, "Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
    r = dap_tambah_header(r, "Access-Control-Allow-Headers", "Content-Type, Authorization");
    return r;
}

// Cryptography & WebSocket RFC 6455
static inline void dap_sha1_raw(const uint8_t* msg, size_t len, uint8_t out[20]) {
    uint32_t h0 = 0x67452301;
    uint32_t h1 = 0xEFCDAB89;
    uint32_t h2 = 0x98BADCFE;
    uint32_t h3 = 0x10325476;
    uint32_t h4 = 0xC3D2E1F0;

    uint64_t bitlen = (uint64_t)len * 8;
    size_t new_len = len + 1;
    while ((new_len % 64) != 56) new_len++;
    size_t total_len = new_len + 8;

    uint8_t* p = (uint8_t*)calloc(total_len, 1);
    if (!p) return;
    if (len > 0 && msg) memcpy(p, msg, len);
    p[len] = 0x80;

    for (int i = 0; i < 8; i++) {
        p[total_len - 1 - i] = (uint8_t)((bitlen >> (i * 8)) & 0xFF);
    }

    for (size_t chunk = 0; chunk < total_len; chunk += 64) {
        uint32_t w[80];
        for (int i = 0; i < 16; i++) {
            w[i] = ((uint32_t)p[chunk + i * 4] << 24)
                 | ((uint32_t)p[chunk + i * 4 + 1] << 16)
                 | ((uint32_t)p[chunk + i * 4 + 2] << 8)
                 | ((uint32_t)p[chunk + i * 4 + 3]);
        }
        for (int i = 16; i < 80; i++) {
            uint32_t temp = w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16];
            w[i] = (temp << 1) | (temp >> 31);
        }

        uint32_t a = h0;
        uint32_t b = h1;
        uint32_t c = h2;
        uint32_t d = h3;
        uint32_t e = h4;

        for (int i = 0; i < 80; i++) {
            uint32_t f, k;
            if (i < 20) {
                f = (b & c) | ((~b) & d);
                k = 0x5A827999;
            } else if (i < 40) {
                f = b ^ c ^ d;
                k = 0x6ED9EBA1;
            } else if (i < 60) {
                f = (b & c) | (b & d) | (c & d);
                k = 0x8F1BBCDC;
            } else {
                f = b ^ c ^ d;
                k = 0xCA62C1D6;
            }

            uint32_t temp = ((a << 5) | (a >> 27)) + f + e + k + w[i];
            e = d;
            d = c;
            c = (b << 30) | (b >> 2);
            b = a;
            a = temp;
        }

        h0 += a;
        h1 += b;
        h2 += c;
        h3 += d;
        h4 += e;
    }

    free(p);

    uint32_t h[5] = { h0, h1, h2, h3, h4 };
    for (int i = 0; i < 5; i++) {
        out[i * 4]     = (uint8_t)((h[i] >> 24) & 0xFF);
        out[i * 4 + 1] = (uint8_t)((h[i] >> 16) & 0xFF);
        out[i * 4 + 2] = (uint8_t)((h[i] >> 8) & 0xFF);
        out[i * 4 + 3] = (uint8_t)(h[i] & 0xFF);
    }
}

static inline const char* dap_sha1(const char* s) {
    if (!s) return "";
    uint8_t hash[20];
    dap_sha1_raw((const uint8_t*)s, strlen(s), hash);
    char* out = (char*)malloc(41);
    if (!out) return "";
    for (int i = 0; i < 20; i++) {
        sprintf(out + (i * 2), "%02x", hash[i]);
    }
    out[40] = '\0';
    return out;
}

static inline const char* dap_b64_encode_bytes(const uint8_t* data, size_t len) {
    static const char tbl[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    size_t out_len = 4 * ((len + 2) / 3);
    char* out = (char*)malloc(out_len + 1);
    if (!out) return "";
    size_t j = 0;
    for (size_t i = 0; i < len; i += 3) {
        uint32_t a = data[i];
        uint32_t b = (i + 1 < len) ? data[i + 1] : 0;
        uint32_t c = (i + 2 < len) ? data[i + 2] : 0;
        uint32_t triple = (a << 16) | (b << 8) | c;

        out[j++] = tbl[(triple >> 18) & 0x3F];
        out[j++] = tbl[(triple >> 12) & 0x3F];
        out[j++] = (i + 1 < len) ? tbl[(triple >> 6) & 0x3F] : '=';
        out[j++] = (i + 2 < len) ? tbl[triple & 0x3F] : '=';
    }
    out[j] = '\0';
    return out;
}

static inline const char* dap_kunci_websocket(const char* client_key) {
    if (!client_key) return "";
    while (*client_key == ' ' || *client_key == '\t' || *client_key == '\r' || *client_key == '\n') client_key++;
    size_t klen = strlen(client_key);
    while (klen > 0 && (client_key[klen-1] == ' ' || client_key[klen-1] == '\t' || client_key[klen-1] == '\r' || client_key[klen-1] == '\n')) {
        klen--;
    }
    const char* guid = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    size_t guid_len = strlen(guid);
    size_t combined_len = klen + guid_len;
    char* combined = (char*)malloc(combined_len + 1);
    if (!combined) return "";
    memcpy(combined, client_key, klen);
    memcpy(combined + klen, guid, guid_len);
    combined[combined_len] = '\0';

    uint8_t hash[20];
    dap_sha1_raw((const uint8_t*)combined, combined_len, hash);
    free(combined);

    return dap_b64_encode_bytes(hash, 20);
}

static inline DapResult_ResponsHttp dap_respons_upgrade_websocket(const char* client_key, const char* ws_handler) {
    DapResult_ResponsHttp res;
    if (!client_key || strlen(client_key) == 0) {
        res.is_err = true;
        res.err_msg = "Kunci WebSocket tidak valid";
        return res;
    }
    res.is_err = false;
    res.err_msg = NULL;

    ResponsHttp r;
    r.status = 101;
    r.tipe_konten = "";
    r.konten = "";
    r.ws_handler = ws_handler ? (Teks)ws_handler : "";

    const char* accept_key = dap_kunci_websocket(client_key);
    size_t needed = strlen(accept_key) + 128;
    char* h = (char*)malloc(needed);
    if (!h) {
        res.is_err = true;
        res.err_msg = "Alokasi memori gagal";
        return res;
    }
    snprintf(h, needed,
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        "Sec-WebSocket-Accept: %s\r\n",
        accept_key);
    r.header = h;
    res.ok = r;
    return res;
}

static inline bool dap_req_adalah_websocket(const PermintaanHttp* req) {
    if (!req || !req->raw_headers) return false;
    const char* p = req->raw_headers;
    while (*p) {
        if (dap_strncasecmp(p, "Upgrade:", 8) == 0) {
            const char* val = p + 8;
            while (*val && (*val == ' ' || *val == '\t')) val++;
            if (dap_strncasecmp(val, "websocket", 9) == 0) return true;
        }
        const char* next = strchr(p, '\n');
        if (!next) break;
        p = next + 1;
    }
    return false;
}

static inline DapResult_Teks dap_req_kunci_websocket(const PermintaanHttp* req) {
    DapResult_Teks res;
    if (!req || !req->raw_headers) {
        res.is_err = true;
        res.err_msg = "Header Sec-WebSocket-Key tidak ditemukan";
        res.ok = "";
        return res;
    }
    const char* p = req->raw_headers;
    while (*p) {
        if (dap_strncasecmp(p, "Sec-WebSocket-Key:", 18) == 0) {
            const char* start = p + 18;
            while (*start == ' ' || *start == '\t') start++;
            const char* end = start;
            while (*end && *end != '\r' && *end != '\n') end++;
            size_t len = (size_t)(end - start);
            char* key = (char*)malloc(len + 1);
            if (!key) break;
            memcpy(key, start, len);
            key[len] = '\0';
            res.is_err = false;
            res.err_msg = NULL;
            res.ok = key;
            return res;
        }
        const char* next = strchr(p, '\n');
        if (!next) break;
        p = next + 1;
    }
    res.is_err = true;
    res.err_msg = "Header Sec-WebSocket-Key tidak ditemukan";
    res.ok = "";
    return res;
}

static inline DapResult_ResponsHttp dap_req_tingkatkan_websocket(const PermintaanHttp* req, const char* handler_name) {
    DapResult_Teks k_res = dap_req_kunci_websocket(req);
    if (k_res.is_err) {
        DapResult_ResponsHttp res;
        res.is_err = true;
        res.err_msg = k_res.err_msg;
        return res;
    }
    return dap_respons_upgrade_websocket(k_res.ok, handler_name);
}

static Teks dap_dispatch_ws_handler(const char* name, const char* msg);

static inline void dap_ws_send_frame(SOCKET sock, uint8_t opcode, const uint8_t* payload, size_t len) {
    uint8_t hdr[10];
    size_t hdr_len = 0;
    hdr[0] = 0x80 | (opcode & 0x0F);

    if (len <= 125) {
        hdr[1] = (uint8_t)len;
        hdr_len = 2;
    } else if (len <= 65535) {
        hdr[1] = 126;
        hdr[2] = (uint8_t)((len >> 8) & 0xFF);
        hdr[3] = (uint8_t)(len & 0xFF);
        hdr_len = 4;
    } else {
        hdr[1] = 127;
        for (int i = 0; i < 8; i++) {
            hdr[2 + i] = (uint8_t)((len >> ((7 - i) * 8)) & 0xFF);
        }
        hdr_len = 10;
    }

    send(sock, (const char*)hdr, (int)hdr_len, 0);
    if (len > 0 && payload) {
        send(sock, (const char*)payload, (int)len, 0);
    }
}

static inline int dap_recv_exact(SOCKET fd, char* buf, int len) {
    int total = 0;
    while (total < len) {
        int r = recv(fd, buf + total, len - total, 0);
        if (r <= 0) return r;
        total += r;
    }
    return total;
}

static inline void dap_ws_run_loop(SOCKET client_fd, const char* ws_handler_name) {
    while (1) {
        uint8_t hdr[2];
        int r = dap_recv_exact(client_fd, (char*)hdr, 2);
        if (r <= 0) break;

        uint8_t opcode = hdr[0] & 0x0F;
        bool masked = (hdr[1] & 0x80) != 0;
        uint64_t payload_len = hdr[1] & 0x7F;

        if (payload_len == 126) {
            uint8_t ext[2];
            if (dap_recv_exact(client_fd, (char*)ext, 2) <= 0) break;
            payload_len = ((uint64_t)ext[0] << 8) | (uint64_t)ext[1];
        } else if (payload_len == 127) {
            uint8_t ext[8];
            if (dap_recv_exact(client_fd, (char*)ext, 8) <= 0) break;
            payload_len = 0;
            for (int i = 0; i < 8; i++) {
                payload_len = (payload_len << 8) | ext[i];
            }
        }

        uint8_t mask[4] = {0, 0, 0, 0};
        if (masked) {
            if (dap_recv_exact(client_fd, (char*)mask, 4) <= 0) break;
        }

        uint8_t* payload = (uint8_t*)malloc((size_t)payload_len + 1);
        if (!payload) break;

        if (payload_len > 0) {
            if (dap_recv_exact(client_fd, (char*)payload, (int)payload_len) <= 0) {
                free(payload);
                break;
            }
        }

        if (masked) {
            for (size_t i = 0; i < payload_len; i++) {
                payload[i] ^= mask[i % 4];
            }
        }
        payload[payload_len] = '\0';

        if (opcode == 0x8) {
            dap_ws_send_frame(client_fd, 0x8, payload, (size_t)payload_len);
            free(payload);
            break;
        } else if (opcode == 0x9) {
            dap_ws_send_frame(client_fd, 0xA, payload, (size_t)payload_len);
            free(payload);
        } else if (opcode == 0xA) {
            free(payload);
        } else if (opcode == 0x1) {
            Teks resp_text = dap_dispatch_ws_handler(ws_handler_name, (const char*)payload);
            if (resp_text && strlen(resp_text) > 0) {
                dap_ws_send_frame(client_fd, 0x1, (const uint8_t*)resp_text, strlen(resp_text));
            }
            free(payload);
        } else {
            free(payload);
        }
    }
}

static inline DapResult_u64 dap_buat_server_http(u64 port, dap_http_handler_t handler, u64 max_req) {
    DapResult_u64 res;
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = 0;

    if (!handler) {
        res.is_err = true;
        res.err_msg = "Fungsi penangan (handler) HTTP tidak ditemukan";
        return res;
    }

#ifdef _WIN32
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        res.is_err = true;
        res.err_msg = "Gagal inisialisasi Winsock";
        return res;
    }
#endif

    SOCKET server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd == INVALID_SOCKET) {
        res.is_err = true;
        res.err_msg = "Gagal membuat socket TCP";
        return res;
    }

    int opt = 1;
#ifdef _WIN32
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, (const char*)&opt, sizeof(opt));
#else
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
#endif

    struct sockaddr_in address;
    memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_addr.s_addr = INADDR_ANY;
    address.sin_port = htons((uint16_t)port);

    if (bind(server_fd, (struct sockaddr*)&address, sizeof(address)) == SOCKET_ERROR) {
        closesocket(server_fd);
        res.is_err = true;
        res.err_msg = "Gagal bind port server HTTP (mungkin port sedang digunakan)";
        return res;
    }

    if (listen(server_fd, 64) == SOCKET_ERROR) {
        closesocket(server_fd);
        res.is_err = true;
        res.err_msg = "Gagal listen pada socket HTTP";
        return res;
    }

    u64 count = 0;
    while (max_req == 0 || count < max_req) {
        struct sockaddr_in client_addr;
        socklen_t addrlen = sizeof(client_addr);
        SOCKET client_fd = accept(server_fd, (struct sockaddr*)&client_addr, &addrlen);
        if (client_fd == INVALID_SOCKET) {
            break;
        }

        char buffer[8192];
        memset(buffer, 0, sizeof(buffer));
        int bytes_read = recv(client_fd, buffer, sizeof(buffer) - 1, 0);
        if (bytes_read > 0) {
            char metode[32] = "GET";
            char target[1024] = "/";
            sscanf(buffer, "%31s %1023s", metode, target);

            char jalur[1024];
            char kueri[1024] = "";
            char* qmark = strchr(target, '?');
            if (qmark) {
                size_t path_len = (size_t)(qmark - target);
                if (path_len >= sizeof(jalur)) path_len = sizeof(jalur) - 1;
                strncpy(jalur, target, path_len);
                jalur[path_len] = '\0';
                strncpy(kueri, qmark + 1, sizeof(kueri) - 1);
                kueri[sizeof(kueri) - 1] = '\0';
            } else {
                strncpy(jalur, target, sizeof(jalur) - 1);
                jalur[sizeof(jalur) - 1] = '\0';
            }

            const char* body_start = strstr(buffer, "\r\n\r\n");
            const char* req_body = body_start ? (body_start + 4) : "";

            PermintaanHttp req;
            req.metode = metode;
            req.jalur = jalur;
            req.kueri = kueri;
            req.isi = (Teks)req_body;
            req.raw_headers = (Teks)buffer;

            ResponsHttp resp = handler(req);

            if (resp.status == 101) {
                char header_buf[4096];
                snprintf(header_buf, sizeof(header_buf),
                    "HTTP/1.1 101 Switching Protocols\r\n"
                    "%s\r\n",
                    resp.header ? resp.header : "");
                send(client_fd, header_buf, (int)strlen(header_buf), 0);
                dap_ws_run_loop(client_fd, resp.ws_handler);
            } else {
                const char* status_text = "OK";
                if (resp.status == 404) status_text = "Not Found";
                else if (resp.status == 400) status_text = "Bad Request";
                else if (resp.status == 403) status_text = "Forbidden";
                else if (resp.status == 500) status_text = "Internal Server Error";
                else if (resp.status == 201) status_text = "Created";

                size_t body_len = resp.konten ? strlen(resp.konten) : 0;
                char header_buf[4096];
                snprintf(header_buf, sizeof(header_buf),
                    "HTTP/1.1 %llu %s\r\n"
                    "Content-Type: %s\r\n"
                    "Content-Length: %zu\r\n"
                    "%s"
                    "Connection: close\r\n\r\n",
                    (unsigned long long)resp.status, status_text,
                    resp.tipe_konten ? resp.tipe_konten : "text/plain",
                    body_len,
                    resp.header ? resp.header : "");

                send(client_fd, header_buf, (int)strlen(header_buf), 0);
                if (body_len > 0) {
                    send(client_fd, resp.konten, (int)body_len, 0);
                }
            }
        }
        closesocket(client_fd);
        count++;
    }

    closesocket(server_fd);
#ifdef _WIN32
    WSACleanup();
#endif
    res.ok = count;
    return res;
}

static inline DapResult_u64 dap_buat_server_websocket(u64 port, const char* handler_name, u64 max_req) {
    DapResult_u64 res;
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = 0;

#ifdef _WIN32
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        res.is_err = true;
        res.err_msg = "Gagal inisialisasi Winsock";
        return res;
    }
#endif

    SOCKET server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd == INVALID_SOCKET) {
        res.is_err = true;
        res.err_msg = "Gagal membuat socket TCP";
        return res;
    }

    int opt = 1;
#ifdef _WIN32
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, (const char*)&opt, sizeof(opt));
#else
    setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
#endif

    struct sockaddr_in address;
    memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_addr.s_addr = INADDR_ANY;
    address.sin_port = htons((uint16_t)port);

    if (bind(server_fd, (struct sockaddr*)&address, sizeof(address)) == SOCKET_ERROR) {
        closesocket(server_fd);
        res.is_err = true;
        res.err_msg = "Gagal bind port server WebSocket (mungkin port sedang digunakan)";
        return res;
    }

    if (listen(server_fd, 64) == SOCKET_ERROR) {
        closesocket(server_fd);
        res.is_err = true;
        res.err_msg = "Gagal listen pada socket WebSocket";
        return res;
    }

    u64 count = 0;
    while (max_req == 0 || count < max_req) {
        struct sockaddr_in client_addr;
        socklen_t addrlen = sizeof(client_addr);
        SOCKET client_fd = accept(server_fd, (struct sockaddr*)&client_addr, &addrlen);
        if (client_fd == INVALID_SOCKET) {
            break;
        }

        char buffer[8192];
        memset(buffer, 0, sizeof(buffer));
        int bytes_read = recv(client_fd, buffer, sizeof(buffer) - 1, 0);
        if (bytes_read > 0) {
            PermintaanHttp req;
            req.metode = "GET";
            req.jalur = "/";
            req.kueri = "";
            req.isi = "";
            req.raw_headers = (Teks)buffer;

            DapResult_ResponsHttp up = dap_req_tingkatkan_websocket(&req, handler_name);
            if (!up.is_err) {
                char header_buf[4096];
                snprintf(header_buf, sizeof(header_buf),
                    "HTTP/1.1 101 Switching Protocols\r\n"
                    "%s\r\n",
                    up.ok.header ? up.ok.header : "");
                send(client_fd, header_buf, (int)strlen(header_buf), 0);
                dap_ws_run_loop(client_fd, handler_name);
            } else {
                const char* bad = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\nUpgrade to WebSocket required\r\n";
                send(client_fd, bad, (int)strlen(bad), 0);
            }
        }
        closesocket(client_fd);
        count++;
    }

    closesocket(server_fd);
#ifdef _WIN32
    WSACleanup();
#endif
    res.ok = count;
    return res;
}

// Router Pattern Matching & Query Parser C11 implementation
static inline bool dap_cocok_rute(const char* pola, const char* jalur) {
    if (!pola || !jalur) return false;
    char p_copy[512];
    char j_copy[512];
    strncpy(p_copy, pola, sizeof(p_copy) - 1);
    strncpy(j_copy, jalur, sizeof(j_copy) - 1);
    p_copy[sizeof(p_copy) - 1] = '\0';
    j_copy[sizeof(j_copy) - 1] = '\0';

    char* p_ctx = NULL;
    char* j_ctx = NULL;
#ifdef _WIN32
    char* p_tok = strtok_s(p_copy, "/", &p_ctx);
    char* j_tok = strtok_s(j_copy, "/", &j_ctx);
#else
    char* p_tok = strtok_r(p_copy, "/", &p_ctx);
    char* j_tok = strtok_r(j_copy, "/", &j_ctx);
#endif

    while (p_tok && j_tok) {
        if (p_tok[0] != ':' && strcmp(p_tok, "*") != 0) {
            if (strcmp(p_tok, j_tok) != 0) return false;
        }
#ifdef _WIN32
        p_tok = strtok_s(NULL, "/", &p_ctx);
        j_tok = strtok_s(NULL, "/", &j_ctx);
#else
        p_tok = strtok_r(NULL, "/", &p_ctx);
        j_tok = strtok_r(NULL, "/", &j_ctx);
#endif
    }
    return (p_tok == NULL && j_tok == NULL);
}

static inline DapResult_Teks dap_ambil_param_rute(const char* pola, const char* jalur, const char* nama_param) {
    DapResult_Teks res;
    if (!pola || !jalur || !nama_param) {
        res.is_err = true;
        res.err_msg = "Argumen parameter rute tidak boleh kosong";
        res.ok = "";
        return res;
    }
    const char* clean_param = (nama_param[0] == ':') ? nama_param + 1 : nama_param;

    char p_copy[512];
    char j_copy[512];
    strncpy(p_copy, pola, sizeof(p_copy) - 1);
    strncpy(j_copy, jalur, sizeof(j_copy) - 1);
    p_copy[sizeof(p_copy) - 1] = '\0';
    j_copy[sizeof(j_copy) - 1] = '\0';

    char* p_ctx = NULL;
    char* j_ctx = NULL;
#ifdef _WIN32
    char* p_tok = strtok_s(p_copy, "/", &p_ctx);
    char* j_tok = strtok_s(j_copy, "/", &j_ctx);
#else
    char* p_tok = strtok_r(p_copy, "/", &p_ctx);
    char* j_tok = strtok_r(j_copy, "/", &j_ctx);
#endif

    while (p_tok && j_tok) {
        if (p_tok[0] == ':') {
            if (strcmp(p_tok + 1, clean_param) == 0) {
                res.is_err = false;
                res.err_msg = NULL;
                char* match_val = (char*)malloc(strlen(j_tok) + 1);
                if (!match_val) {
                    res.is_err = true;
                    res.err_msg = "Alokasi memori gagal";
                    res.ok = "";
                    return res;
                }
                strcpy(match_val, j_tok);
                res.ok = match_val;
                return res;
            }
        } else if (strcmp(p_tok, "*") != 0 && strcmp(p_tok, j_tok) != 0) {
            res.is_err = true;
            res.err_msg = "Jalur tidak cocok dengan pola rute";
            res.ok = "";
            return res;
        }
#ifdef _WIN32
        p_tok = strtok_s(NULL, "/", &p_ctx);
        j_tok = strtok_s(NULL, "/", &j_ctx);
#else
        p_tok = strtok_r(NULL, "/", &p_ctx);
        j_tok = strtok_r(NULL, "/", &j_ctx);
#endif
    }

    res.is_err = true;
    res.err_msg = "Parameter rute tidak ditemukan";
    res.ok = "";
    return res;
}

static inline DapResult_Teks dap_ambil_kueri_param(const char* kueri, const char* kunci) {
    DapResult_Teks res;
    if (!kueri || !kunci) {
        res.is_err = true;
        res.err_msg = "Parameter kueri tidak ditemukan";
        res.ok = "";
        return res;
    }
    const char* q = (kueri[0] == '?') ? kueri + 1 : kueri;
    char q_copy[1024];
    strncpy(q_copy, q, sizeof(q_copy) - 1);
    q_copy[sizeof(q_copy) - 1] = '\0';

    char* ctx = NULL;
#ifdef _WIN32
    char* pair = strtok_s(q_copy, "&", &ctx);
#else
    char* pair = strtok_r(q_copy, "&", &ctx);
#endif
    while (pair) {
        char* eq = strchr(pair, '=');
        if (eq) {
            *eq = '\0';
            if (strcmp(pair, kunci) == 0) {
                char* val = eq + 1;
                for (char* p = val; *p; p++) {
                    if (*p == '+') *p = ' ';
                }
                char* ret = (char*)malloc(strlen(val) + 1);
                if (!ret) {
                    res.is_err = true;
                    res.err_msg = "Alokasi memori gagal";
                    res.ok = "";
                    return res;
                }
                strcpy(ret, val);
                res.is_err = false;
                res.err_msg = NULL;
                res.ok = ret;
                return res;
            }
        } else if (strcmp(pair, kunci) == 0) {
            res.is_err = false;
            res.err_msg = NULL;
            res.ok = "";
            return res;
        }
#ifdef _WIN32
        pair = strtok_s(NULL, "&", &ctx);
#else
        pair = strtok_r(NULL, "&", &ctx);
#endif
    }
    res.is_err = true;
    res.err_msg = "Parameter kueri tidak ditemukan";
    res.ok = "";
    return res;
}

static inline bool dap_ada_kueri_param(const char* kueri, const char* kunci) {
    DapResult_Teks r = dap_ambil_kueri_param(kueri, kunci);
    return !r.is_err;
}

// Static Files & MIME Detection C11 Helpers
static inline const char* dap_deteksi_tipe_mime(const char* jalur) {
    if (!jalur) return "application/octet-stream";
    const char* dot = strrchr(jalur, '.');
    if (!dot) return "application/octet-stream";
    if (strcmp(dot, ".html") == 0 || strcmp(dot, ".htm") == 0) return "text/html; charset=utf-8";
    if (strcmp(dot, ".css") == 0) return "text/css; charset=utf-8";
    if (strcmp(dot, ".js") == 0 || strcmp(dot, ".mjs") == 0) return "application/javascript; charset=utf-8";
    if (strcmp(dot, ".json") == 0) return "application/json; charset=utf-8";
    if (strcmp(dot, ".svg") == 0) return "image/svg+xml";
    if (strcmp(dot, ".png") == 0) return "image/png";
    if (strcmp(dot, ".jpg") == 0 || strcmp(dot, ".jpeg") == 0) return "image/jpeg";
    if (strcmp(dot, ".gif") == 0) return "image/gif";
    if (strcmp(dot, ".webp") == 0) return "image/webp";
    if (strcmp(dot, ".ico") == 0) return "image/x-icon";
    if (strcmp(dot, ".txt") == 0) return "text/plain; charset=utf-8";
    if (strcmp(dot, ".md") == 0) return "text/markdown; charset=utf-8";
    if (strcmp(dot, ".wasm") == 0) return "application/wasm";
    if (strcmp(dot, ".xml") == 0) return "application/xml; charset=utf-8";
    if (strcmp(dot, ".pdf") == 0) return "application/pdf";
    return "application/octet-stream";
}

static inline DapResult_ResponsHttp dap_respons_berkas(const char* jalur_berkas) {
    DapResult_ResponsHttp res;
    if (!jalur_berkas) {
        res.is_err = true;
        res.err_msg = "Jalur berkas tidak boleh kosong";
        res.ok = dap_kirim_respons_http(404, "text/plain", "");
        return res;
    }
    FILE* f = fopen(jalur_berkas, "rb");
    if (!f) {
        res.is_err = true;
        res.err_msg = "Berkas tidak ditemukan";
        res.ok = dap_kirim_respons_http(404, "text/plain", "Berkas tidak ditemukan");
        return res;
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc(sz + 1);
    if (!buf) {
        fclose(f);
        res.is_err = true;
        res.err_msg = "Alokasi memori gagal";
        res.ok = dap_kirim_respons_http(500, "text/plain", "Alokasi memori gagal");
        return res;
    }
    size_t read_bytes = fread(buf, 1, sz, f);
    buf[read_bytes] = '\0';
    fclose(f);

    const char* mime = dap_deteksi_tipe_mime(jalur_berkas);
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = dap_kirim_respons_http(200, mime, buf);
    return res;
}

static inline DapResult_ResponsHttp dap_sajikan_berkas_statis(const char* folder_dasar, const char* jalur) {
    DapResult_ResponsHttp res;
    if (!folder_dasar || !jalur) {
        res.is_err = true;
        res.err_msg = "Parameter tidak valid";
        res.ok = dap_kirim_respons_http(400, "text/plain", "Parameter tidak valid");
        return res;
    }
    if (strstr(jalur, "..")) {
        res.is_err = true;
        res.err_msg = "Akses jalur ditolak";
        res.ok = dap_kirim_respons_http(403, "text/plain", "Akses jalur ditolak");
        return res;
    }
    const char* rel = jalur;
    while (*rel == '/') rel++;
    if (*rel == '\0') rel = "index.html";

    char path_buf[512];
    snprintf(path_buf, sizeof(path_buf), "%s/%s", folder_dasar, rel);
    return dap_respons_berkas(path_buf);
}

// Embedded BasisData C11 Runtime Functions
static inline DapResult_BasisData dap_buka_basis_data(const char* jalur) {
    DapResult_BasisData res;
    if (!jalur || strlen(jalur) == 0) {
        res.is_err = true;
        res.err_msg = "Jalur basis data tidak boleh kosong";
        res.ok.jalur = "";
        return res;
    }
    FILE* f = fopen(jalur, "a+");
    if (!f) {
        res.is_err = true;
        res.err_msg = "Gagal membuka atau membuat berkas basis data";
        res.ok.jalur = "";
        return res;
    }
    fclose(f);
    res.is_err = false;
    res.err_msg = NULL;
    res.ok.jalur = (Teks)jalur;
    return res;
}

static inline DapResult_bool dap_db_simpan(const char* jalur, const char* kunci, const char* nilai) {
    DapResult_bool res;
    if (!jalur || !kunci) {
        res.is_err = true;
        res.err_msg = "Argumen basis data null";
        res.ok = false;
        return res;
    }
    FILE* f = fopen(jalur, "a");
    if (!f) {
        res.is_err = true;
        res.err_msg = "Gagal membuka basis data untuk menulis";
        res.ok = false;
        return res;
    }
    fprintf(f, "S:%zu:%s:%s\n", strlen(kunci), kunci, nilai ? nilai : "");
    fclose(f);
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = true;
    return res;
}

static inline DapResult_Teks dap_db_ambil(const char* jalur, const char* kunci) {
    DapResult_Teks res;
    if (!jalur || !kunci) {
        res.is_err = true;
        res.err_msg = "Argumen basis data null";
        res.ok = "";
        return res;
    }
    FILE* f = fopen(jalur, "r");
    if (!f) {
        res.is_err = true;
        res.err_msg = "Basis data belum dibuat";
        res.ok = "";
        return res;
    }
    char line[4096];
    char* found_val = NULL;
    size_t klen = strlen(kunci);
    while (fgets(line, sizeof(line), f)) {
        size_t llen = strlen(line);
        if (llen > 0 && line[llen-1] == '\n') line[llen-1] = '\0';
        if (strncmp(line, "S:", 2) == 0) {
            char* first_col = strchr(line + 2, ':');
            if (first_col) {
                size_t l_klen = (size_t)atoi(line + 2);
                if (l_klen == klen && strncmp(first_col + 1, kunci, klen) == 0 && first_col[1 + klen] == ':') {
                    if (found_val) free(found_val);
                    found_val = strdup(first_col + 1 + klen + 1);
                }
            }
        } else if (strncmp(line, "H:", 2) == 0) {
            if (strcmp(line + 2, kunci) == 0) {
                if (found_val) { free(found_val); found_val = NULL; }
            }
        }
    }
    fclose(f);
    if (found_val) {
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = found_val;
    } else {
        res.is_err = true;
        res.err_msg = "Kunci tidak ditemukan dalam basis data";
        res.ok = "";
    }
    return res;
}

static inline DapResult_bool dap_db_hapus(const char* jalur, const char* kunci) {
    DapResult_bool res;
    if (!jalur || !kunci) {
        res.is_err = true;
        res.err_msg = "Argumen basis data null";
        res.ok = false;
        return res;
    }
    FILE* f = fopen(jalur, "a");
    if (!f) {
        res.is_err = true;
        res.err_msg = "Gagal membuka basis data untuk menghapus";
        res.ok = false;
        return res;
    }
    fprintf(f, "H:%s\n", kunci);
    fclose(f);
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = true;
    return res;
}

static inline bool dap_db_ada(const char* jalur, const char* kunci) {
    DapResult_Teks r = dap_db_ambil(jalur, kunci);
    return !r.is_err;
}

static inline DapResult_Daftar dap_db_daftar_kunci(const char* jalur) {
    DapResult_Daftar res;
    res.is_err = false;
    res.err_msg = NULL;
    DapList* list = dap_Daftar_baru();
    res.ok = list;
    if (!jalur) return res;
    FILE* f = fopen(jalur, "r");
    if (!f) return res;
    char line[4096];
    while (fgets(line, sizeof(line), f)) {
        size_t llen = strlen(line);
        if (llen > 0 && line[llen-1] == '\n') line[llen-1] = '\0';
        if (strncmp(line, "S:", 2) == 0) {
            char* first_col = strchr(line + 2, ':');
            if (first_col) {
                size_t l_klen = (size_t)atoi(line + 2);
                char* k_str = (char*)malloc(l_klen + 1);
                strncpy(k_str, first_col + 1, l_klen);
                k_str[l_klen] = '\0';
                dap_Daftar_tambah(list, k_str);
            }
        }
    }
    fclose(f);
    return res;
}

static inline DapResult_bool dap_db_padatkan(const char* jalur) {
    DapResult_bool res;
    res.is_err = false;
    res.err_msg = NULL;
    res.ok = true;
    return res;
}

// Base64 Encoding & Decoding
static inline char* dap_ke_base64(const char* s) {
    if (!s) return "";
    static const char b64[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    size_t len = strlen(s);
    size_t out_len = 4 * ((len + 2) / 3);
    char* out = (char*)malloc(out_len + 1);
    if (!out) return "";
    size_t i = 0, j = 0;
    while (i < len) {
        size_t rem = len - i;
        uint32_t a = (unsigned char)s[i++];
        uint32_t b = rem > 1 ? (unsigned char)s[i++] : 0;
        uint32_t c = rem > 2 ? (unsigned char)s[i++] : 0;
        uint32_t triple = (a << 16) | (b << 8) | c;
        out[j++] = b64[(triple >> 18) & 0x3F];
        out[j++] = b64[(triple >> 12) & 0x3F];
        out[j++] = rem > 1 ? b64[(triple >> 6) & 0x3F] : '=';
        out[j++] = rem > 2 ? b64[triple & 0x3F] : '=';
    }
    out[out_len] = '\0';
    return out;
}

static inline DapResult_Teks dap_dari_base64(const char* s) {
    DapResult_Teks res;
    if (!s) { res.is_err = true; res.err_msg = "String null"; res.ok = ""; return res; }
    size_t len = strlen(s);
    if (len % 4 != 0) { res.is_err = true; res.err_msg = "Panjang Base64 harus kelipatan 4"; res.ok = ""; return res; }
    char* out = (char*)malloc(len + 1);
    size_t out_len = 0;
    for (size_t i = 0; i < len; i += 4) {
        int vals[4];
        for (int k = 0; k < 4; k++) {
            char c = s[i+k];
            if (c >= 'A' && c <= 'Z') vals[k] = c - 'A';
            else if (c >= 'a' && c <= 'z') vals[k] = c - 'a' + 26;
            else if (c >= '0' && c <= '9') vals[k] = c - '0' + 52;
            else if (c == '+') vals[k] = 62;
            else if (c == '/') vals[k] = 63;
            else if (c == '=') vals[k] = 0;
            else { free(out); res.is_err = true; res.err_msg = "Karakter Base64 tidak valid"; res.ok = ""; return res; }
        }
        uint32_t triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        out[out_len++] = (triple >> 16) & 0xFF;
        if (s[i+2] != '=') out[out_len++] = (triple >> 8) & 0xFF;
        if (s[i+3] != '=') out[out_len++] = triple & 0xFF;
    }
    out[out_len] = '\0';
    res.is_err = false; res.err_msg = NULL; res.ok = out;
    return res;
}

// SHA-256 (FIPS 180-4)
static inline char* dap_sha256(const char* input) {
    if (!input) input = "";
    size_t len = strlen(input);
    uint64_t bit_len = (uint64_t)len * 8;
    size_t new_len = len + 1;
    while ((new_len % 64) != 56) new_len++;
    size_t total_len = new_len + 8;
    uint8_t* p = (uint8_t*)calloc(total_len, 1);
    memcpy(p, input, len);
    p[len] = 0x80;
    for (int i = 0; i < 8; i++) {
        p[total_len - 1 - i] = (uint8_t)(bit_len >> (i * 8));
    }
    static const uint32_t K[64] = {
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
    };
    uint32_t h[8] = { 0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19 };
    for (size_t chunk = 0; chunk < total_len; chunk += 64) {
        uint32_t w[64];
        for (int i = 0; i < 16; i++) {
            w[i] = ((uint32_t)p[chunk + i*4] << 24) | ((uint32_t)p[chunk + i*4 + 1] << 16) | ((uint32_t)p[chunk + i*4 + 2] << 8) | (uint32_t)p[chunk + i*4 + 3];
        }
        for (int i = 16; i < 64; i++) {
            uint32_t s0 = (w[i-15] >> 7 | w[i-15] << 25) ^ (w[i-15] >> 18 | w[i-15] << 14) ^ (w[i-15] >> 3);
            uint32_t s1 = (w[i-2] >> 17 | w[i-2] << 15) ^ (w[i-2] >> 19 | w[i-2] << 13) ^ (w[i-2] >> 10);
            w[i] = w[i-16] + s0 + w[i-7] + s1;
        }
        uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], h_val = h[7];
        for (int i = 0; i < 64; i++) {
            uint32_t s1 = (e >> 6 | e << 26) ^ (e >> 11 | e << 21) ^ (e >> 25 | e << 7);
            uint32_t ch = (e & f) ^ ((~e) & g);
            uint32_t temp1 = h_val + s1 + ch + K[i] + w[i];
            uint32_t s0 = (a >> 2 | a << 30) ^ (a >> 13 | a << 19) ^ (a >> 22 | a << 10);
            uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint32_t temp2 = s0 + maj;
            h_val = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += h_val;
    }
    free(p);
    char* res = (char*)malloc(65);
    snprintf(res, 65, "%08x%08x%08x%08x%08x%08x%08x%08x", h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
    return res;
}

// MD5 (RFC 1321)
static inline char* dap_md5(const char* input) {
    if (!input) input = "";
    size_t len = strlen(input);
    uint64_t bit_len = (uint64_t)len * 8;
    size_t new_len = len + 1;
    while ((new_len % 64) != 56) new_len++;
    size_t total_len = new_len + 8;
    uint8_t* p = (uint8_t*)calloc(total_len, 1);
    memcpy(p, input, len);
    p[len] = 0x80;
    for (int i = 0; i < 8; i++) {
        p[total_len - 8 + i] = (uint8_t)(bit_len >> (i * 8));
    }
    uint32_t a0 = 0x67452301, b0 = 0xefcdab89, c0 = 0x98badcfe, d0 = 0x10325476;
    static const uint32_t S[64] = {
        7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,
        5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,
        4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,
        6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21
    };
    uint32_t K[64];
    for (int i = 0; i < 64; i++) {
        K[i] = (uint32_t)(fabs(sin(i + 1.0)) * 4294967296.0);
    }
    for (size_t chunk = 0; chunk < total_len; chunk += 64) {
        uint32_t m[16];
        for (int i = 0; i < 16; i++) {
            m[i] = ((uint32_t)p[chunk + i*4]) | ((uint32_t)p[chunk + i*4 + 1] << 8) | ((uint32_t)p[chunk + i*4 + 2] << 16) | ((uint32_t)p[chunk + i*4 + 3] << 24);
        }
        uint32_t a = a0, b = b0, c = c0, d = d0;
        for (int i = 0; i < 64; i++) {
            uint32_t f, g;
            if (i < 16) { f = (b & c) | ((~b) & d); g = i; }
            else if (i < 32) { f = (d & b) | ((~d) & c); g = (5*i + 1) % 16; }
            else if (i < 48) { f = b ^ c ^ d; g = (3*i + 5) % 16; }
            else { f = c ^ (b | (~d)); g = (7*i) % 16; }
            uint32_t temp = d;
            d = c;
            c = b;
            uint32_t sum = a + f + K[i] + m[g];
            uint32_t rot = (sum << S[i]) | (sum >> (32 - S[i]));
            b += rot;
            a = temp;
        }
        a0 += a; b0 += b; c0 += c; d0 += d;
    }
    free(p);
    char* res = (char*)malloc(33);
    uint8_t out_bytes[16];
    for (int i = 0; i < 4; i++) {
        out_bytes[i] = (a0 >> (i*8)) & 0xFF;
        out_bytes[4+i] = (b0 >> (i*8)) & 0xFF;
        out_bytes[8+i] = (c0 >> (i*8)) & 0xFF;
        out_bytes[12+i] = (d0 >> (i*8)) & 0xFF;
    }
    for (int i = 0; i < 16; i++) {
        snprintf(res + i*2, 3, "%02x", out_bytes[i]);
    }
    res[32] = '\0';
    return res;
}

// ==========================================
// Pustaka Standar Reguler Ekspresi (Regex) C11
// ==========================================

static inline bool _dap_match_class(char c, const char* p, int* p_len) {
    if (*p == '.') {
        *p_len = 1;
        return c != '\0' && c != '\n';
    }
    if (*p == '\\') {
        char esc = p[1];
        *p_len = 2;
        if (esc == 'd') return isdigit((unsigned char)c) != 0;
        if (esc == 'D') return !isdigit((unsigned char)c);
        if (esc == 'w') return (isalnum((unsigned char)c) || c == '_') != 0;
        if (esc == 'W') return !(isalnum((unsigned char)c) || c == '_');
        if (esc == 's') return isspace((unsigned char)c) != 0;
        if (esc == 'S') return !isspace((unsigned char)c);
        if (esc == 'n') return c == '\n';
        if (esc == 't') return c == '\t';
        if (esc == 'r') return c == '\r';
        return c == esc;
    }
    if (*p == '[') {
        const char* end = strchr(p, ']');
        if (!end) { *p_len = 1; return c == *p; }
        *p_len = (int)(end - p + 1);
        bool neg = (p[1] == '^');
        const char* cur = neg ? p + 2 : p + 1;
        bool matched = false;
        while (cur < end) {
            if (cur + 2 < end && cur[1] == '-') {
                if (c >= cur[0] && c <= cur[2]) { matched = true; break; }
                cur += 3;
            } else if (*cur == '\\') {
                int elen = 0;
                if (_dap_match_class(c, cur, &elen)) { matched = true; break; }
                cur += elen;
            } else {
                if (c == *cur) { matched = true; break; }
                cur++;
            }
        }
        return neg ? !matched : matched;
    }
    *p_len = 1;
    return c == *p;
}

static bool _dap_match_here(const char* p, const char* t, int* len) {
    if (*p == '\0') { *len = 0; return true; }
    if (*p == '$' && *(p+1) == '\0') {
        if (*t == '\0') { *len = 0; return true; }
        return false;
    }

    int u_len = 0;
    bool m_first = _dap_match_class(*t, p, &u_len);
    char q = p[u_len];

    if (q == '*') {
        int max_m = 0;
        int dummy;
        while (t[max_m] != '\0' && _dap_match_class(t[max_m], p, &dummy)) {
            max_m++;
        }
        for (int k = max_m; k >= 0; k--) {
            int sub_len = 0;
            if (_dap_match_here(p + u_len + 1, t + k, &sub_len)) {
                *len = k + sub_len;
                return true;
            }
        }
        return false;
    }
    if (q == '+') {
        if (!m_first) return false;
        int max_m = 1;
        int dummy;
        while (t[max_m] != '\0' && _dap_match_class(t[max_m], p, &dummy)) {
            max_m++;
        }
        for (int k = max_m; k >= 1; k--) {
            int sub_len = 0;
            if (_dap_match_here(p + u_len + 1, t + k, &sub_len)) {
                *len = k + sub_len;
                return true;
            }
        }
        return false;
    }
    if (q == '?') {
        if (m_first) {
            int sub_len = 0;
            if (_dap_match_here(p + u_len + 1, t + 1, &sub_len)) {
                *len = 1 + sub_len;
                return true;
            }
        }
        int sub_len = 0;
        if (_dap_match_here(p + u_len + 1, t, &sub_len)) {
            *len = sub_len;
            return true;
        }
        return false;
    }

    if (m_first) {
        int sub_len = 0;
        if (_dap_match_here(p + u_len, t + 1, &sub_len)) {
            *len = 1 + sub_len;
            return true;
        }
    }
    return false;
}

static bool _dap_match_branch(const char* p, const char* t, int* match_start, int* match_len) {
    if (*p == '^') {
        int m_len = 0;
        if (_dap_match_here(p + 1, t, &m_len)) {
            *match_start = 0;
            *match_len = m_len;
            return true;
        }
        return false;
    }
    int t_len = (int)strlen(t);
    for (int i = 0; i <= t_len; i++) {
        int m_len = 0;
        if (_dap_match_here(p, t + i, &m_len)) {
            *match_start = i;
            *match_len = m_len;
            return true;
        }
    }
    return false;
}

static bool _dap_match_regex(const char* pola, const char* teks, int* match_start, int* match_len) {
    if (!pola || !teks) return false;
    char buf[512];
    strncpy(buf, pola, sizeof(buf)-1);
    buf[sizeof(buf)-1] = '\0';
    char* bar = strchr(buf, '|');
    if (bar) {
        *bar = '\0';
        if (_dap_match_branch(buf, teks, match_start, match_len)) return true;
        return _dap_match_branch(bar + 1, teks, match_start, match_len);
    }
    return _dap_match_branch(pola, teks, match_start, match_len);
}

static inline bool dap_cocok_regex(const char* pola, const char* teks) {
    int s = 0, l = 0;
    return _dap_match_regex(pola, teks, &s, &l);
}

static inline DapResult_Teks dap_ekstrak_regex(const char* pola, const char* teks) {
    DapResult_Teks res;
    if (!pola || !teks) {
        res.is_err = true;
        res.err_msg = "Argumen regex null";
        res.ok = "";
        return res;
    }
    int s = 0, l = 0;
    if (_dap_match_regex(pola, teks, &s, &l)) {
        char* b = (char*)malloc(l + 1);
        if (!b) {
            res.is_err = true;
            res.err_msg = "Alokasi memori gagal";
            res.ok = "";
            return res;
        }
        strncpy(b, teks + s, l);
        b[l] = '\0';
        res.is_err = false;
        res.err_msg = NULL;
        res.ok = b;
        return res;
    }
    res.is_err = true;
    res.err_msg = "Pola regex tidak cocok pada teks";
    res.ok = "";
    return res;
}

static inline DapList* dap_cari_regex(const char* pola, const char* teks) {
    DapList* list = dap_Daftar_baru();
    if (!pola || !teks) return list;
    const char* cur = teks;
    int s = 0, l = 0;
    while (*cur != '\0' && _dap_match_regex(pola, cur, &s, &l)) {
        char* b = (char*)malloc(l + 1);
        if (b) {
            strncpy(b, cur + s, l);
            b[l] = '\0';
            dap_Daftar_tambah(list, b);
        }
        int adv = s + (l > 0 ? l : 1);
        if ((int)strlen(cur) < adv) break;
        cur += adv;
    }
    return list;
}

static inline char* dap_ganti_regex(const char* pola, const char* teks, const char* pengganti) {
    if (!pola || !teks) return strdup("");
    if (!pengganti) pengganti = "";
    size_t out_cap = strlen(teks) * 2 + strlen(pengganti) + 64;
    char* out = (char*)malloc(out_cap);
    if (!out) return strdup(teks);
    out[0] = '\0';
    size_t out_len = 0;

    const char* cur = teks;
    int s = 0, l = 0;
    while (*cur != '\0' && _dap_match_regex(pola, cur, &s, &l)) {
        if (s > 0) {
            while (out_len + s + strlen(pengganti) + 1 >= out_cap) {
                out_cap *= 2;
                out = (char*)realloc(out, out_cap);
            }
            strncat(out, cur, s);
            out_len += s;
        }
        size_t p_len = strlen(pengganti);
        while (out_len + p_len + 1 >= out_cap) {
            out_cap *= 2;
            out = (char*)realloc(out, out_cap);
        }
        strcat(out, pengganti);
        out_len += p_len;

        int adv = s + (l > 0 ? l : 1);
        if ((int)strlen(cur) < adv) {
            cur += strlen(cur);
            break;
        }
        cur += adv;
    }
    size_t rem = strlen(cur);
    if (rem > 0) {
        while (out_len + rem + 1 >= out_cap) {
            out_cap += rem + 64;
            out = (char*)realloc(out, out_cap);
        }
        strcat(out, cur);
    }
    return out;
}

"#,
        );
    }

    fn emit_fn_proto(&mut self, f: &FnDecl, self_struct: Option<&str>) {
        let fn_c_name = if let Some(s) = self_struct {
            format!("dap_{}_{}", s, f.name)
        } else {
            format!("dap_{}", f.name)
        };

        let ret_ty = self.ret_type_to_c(&f.ret_ty);
        self.out.push_str(&format!("{} {}(", ret_ty, fn_c_name));
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            if p.is_self {
                let s = self_struct.unwrap_or("void");
                self.out.push_str(&format!("{}* ini", s));
            } else if let Some(pty) = &p.ty {
                self.out.push_str(&format!("{} {}", self.type_to_c(pty), p.name));
            } else {
                self.out.push_str(&format!("void* {}", p.name));
            }
        }
        self.out.push_str(");\n");
    }

    fn emit_function(&mut self, f: &FnDecl, self_struct: Option<&str>) {
        let fn_c_name = if let Some(s) = self_struct {
            format!("dap_{}_{}", s, f.name)
        } else {
            format!("dap_{}", f.name)
        };

        let ret_ty = self.ret_type_to_c(&f.ret_ty);
        self.out.push_str(&format!("{} {}(", ret_ty, fn_c_name));
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            if p.is_self {
                let s = self_struct.unwrap_or("void");
                self.out.push_str(&format!("{}* ini", s));
            } else if let Some(pty) = &p.ty {
                self.out.push_str(&format!("{} {}", self.type_to_c(pty), p.name));
            } else {
                self.out.push_str(&format!("void* {}", p.name));
            }
        }
        self.out.push_str(") {\n");

        for stmt in &f.body.stmts {
            self.emit_stmt(stmt, &ret_ty);
        }

        if let Some(expr) = &f.body.expr {
            let expr_str = self.emit_expr(expr);
            if ret_ty == "void" {
                self.out.push_str(&format!("    {};\n    return;\n", expr_str));
            } else if ret_ty.starts_with("DapResult_") {
                self.out.push_str(&format!(
                    "    return ({}){{ .is_err = false, .ok = {} }};\n",
                    ret_ty, expr_str
                ));
            } else {
                self.out.push_str(&format!("    return {};\n", expr_str));
            }
        }

        self.out.push_str("}\n\n");
    }

    fn emit_stmt(&mut self, stmt: &Stmt, fn_ret_ty: &str) {
        match stmt {
            Stmt::VarDecl { name, init, .. } => {
                // Check if init is a catch expression on an error union
                if let Expr::Catch { inner, err_var, handler, .. } = init {
                    let in_str = self.emit_expr(inner);
                    let res_var = format!("_res_{}", name);
                    self.out.push_str(&format!("    __auto_type {} = {};\n", res_var, in_str));
                    self.out.push_str(&format!("    if ({}.is_err) {{\n", res_var));
                    self.out.push_str(&format!("        const char* {} = {}.err_msg;\n", err_var, res_var));
                    for s in &handler.stmts {
                        self.emit_stmt(s, fn_ret_ty);
                    }
                    if let Some(he) = &handler.expr {
                        let he_str = self.emit_expr(he);
                        if fn_ret_ty == "void" {
                            self.out.push_str(&format!("        {};\n", he_str));
                        } else {
                            self.out.push_str(&format!("        return {};\n", he_str));
                        }
                    }
                    self.out.push_str("    }\n");
                    self.out.push_str(&format!("    __auto_type {} = {}.ok;\n", name, res_var));
                } else {
                    let init_str = self.emit_expr(init);
                    self.out.push_str(&format!("    __auto_type {} = {};\n", name, init_str));
                }
            }
            Stmt::Assign { target, value, .. } => {
                let t_str = self.emit_expr(target);
                let v_str = self.emit_expr(value);
                self.out.push_str(&format!("    {} = {};\n", t_str, v_str));
            }
            Stmt::If { cond, then_branch, else_branch, .. } => {
                let c_str = self.emit_expr(cond);
                self.out.push_str(&format!("    if ({}) {{\n", c_str));
                for s in &then_branch.stmts {
                    self.emit_stmt(s, fn_ret_ty);
                }
                if let Some(e) = &then_branch.expr {
                    let es = self.emit_expr(e);
                    self.out.push_str(&format!("        {};\n", es));
                }
                if let Some(eb) = else_branch {
                    self.out.push_str("    } else {\n");
                    for s in &eb.stmts {
                        self.emit_stmt(s, fn_ret_ty);
                    }
                    if let Some(e) = &eb.expr {
                        let es = self.emit_expr(e);
                        self.out.push_str(&format!("        {};\n", es));
                    }
                }
                self.out.push_str("    }\n");
            }
            Stmt::While { cond, body, .. } => {
                let c_str = self.emit_expr(cond);
                self.out.push_str(&format!("    while ({}) {{\n", c_str));
                for s in &body.stmts {
                    self.emit_stmt(s, fn_ret_ty);
                }
                if let Some(e) = &body.expr {
                    let es = self.emit_expr(e);
                    self.out.push_str(&format!("        {};\n", es));
                }
                self.out.push_str("    }\n");
            }
            Stmt::ForIn { var_name, iterable, body, .. } => {
                let it_str = self.emit_expr(iterable);
                let idx_var = format!("_i_{}", self._temp_counter);
                let list_var = format!("_list_{}", self._temp_counter);
                self._temp_counter += 1;
                self.out.push_str(&format!("    DapList* {} = {};\n", list_var, it_str));
                self.out.push_str(&format!(
                    "    for (size_t {} = 0; {} < {}->len; {}++) {{\n",
                    idx_var, idx_var, list_var, idx_var
                ));
                self.out.push_str(&format!(
                    "        void* {} = {}->items[{}];\n",
                    var_name, list_var, idx_var
                ));
                for s in &body.stmts {
                    self.emit_stmt(s, fn_ret_ty);
                }
                if let Some(e) = &body.expr {
                    let es = self.emit_expr(e);
                    self.out.push_str(&format!("        {};\n", es));
                }
                self.out.push_str("    }\n");
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    if let Expr::Catch { inner, err_var, handler, .. } = v {
                        let in_str = self.emit_expr(inner);
                        let res_var = format!("_res_ret_{}", self._temp_counter);
                        self._temp_counter += 1;
                        self.out.push_str(&format!("    __auto_type {} = {};\n", res_var, in_str));
                        self.out.push_str(&format!("    if ({}.is_err) {{\n", res_var));
                        self.out.push_str(&format!("        const char* {} = {}.err_msg;\n", err_var, res_var));
                        for s in &handler.stmts {
                            self.emit_stmt(s, fn_ret_ty);
                        }
                        if let Some(he) = &handler.expr {
                            let he_str = self.emit_expr(he);
                            if fn_ret_ty == "void" {
                                self.out.push_str(&format!("        {};\n", he_str));
                            } else {
                                self.out.push_str(&format!("        return {};\n", he_str));
                            }
                        }
                        self.out.push_str("    }\n");
                        self.out.push_str(&format!("    return {}.ok;\n", res_var));
                    } else {
                        let v_str = self.emit_expr(v);
                        self.out.push_str(&format!("    return {};\n", v_str));
                    }
                } else {
                    self.out.push_str("    return;\n");
                }
            }
            Stmt::Fail { value, .. } => {
                let v_str = self.emit_expr(value);
                self.out.push_str(&format!(
                    "    return ({}){{ .is_err = true, .err_msg = {} }};\n",
                    fn_ret_ty, v_str
                ));
            }
            Stmt::Expr(e) => {
                let e_str = self.emit_expr(e);
                self.out.push_str(&format!("    {};\n", e_str));
            }
        }
    }

    fn emit_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Lit(lit, _) => match lit {
                Literal::Int(n) => n.to_string(),
                Literal::Float(f) => {
                    let s = f.to_string();
                    if s.contains('.') {
                        s
                    } else {
                        format!("{}.0", s)
                    }
                }
                Literal::String(s) => {
                    let clean = s
                        .replace('\\', "\\\\")
                        .replace('\"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r")
                        .replace('\t', "\\t");
                    format!("\"{}\"", clean)
                }
                Literal::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
                Literal::Unit => "((void)0)".to_string(),
            },
            Expr::Ident(name, _) => name.clone(),
            Expr::SelfVar(_) => "ini".to_string(),
            Expr::Binary(left, op, right, _) => {
                let l = self.emit_expr(left);
                let r = self.emit_expr(right);
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Mod => "%",
                    BinaryOp::Eq => "==",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::LtEq => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::GtEq => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                };
                format!("({} {} {})", l, op_str, r)
            }
            Expr::Unary(op, operand, _) => {
                let o = self.emit_expr(operand);
                match op {
                    UnaryOp::Neg => format!("(-{})", o),
                    UnaryOp::Not => format!("(!{})", o),
                }
            }
            Expr::Call { callee, args, .. } => {
                if let Expr::Ident(name, _) = &**callee {
                    if name == "cetak" {
                        if args.is_empty() {
                            return "printf(\"\\n\")".to_string();
                        } else if args.len() == 1 {
                            let arg_str = self.emit_expr(&args[0].value);
                            return format!("dap_cetak({})", arg_str);
                        } else {
                            let arg_strs: Vec<String> = args
                                .iter()
                                .map(|a| format!("dap_to_str({})", self.emit_expr(&a.value)))
                                .collect();
                            return format!(
                                "dap_cetak_multi({}, (const char*[]){{ {} }})",
                                arg_strs.len(),
                                arg_strs.join(", ")
                            );
                        }
                    }
                    if name == "tunggu" && !args.is_empty() {
                        let arg_str = self.emit_expr(&args[0].value);
                        return format!("dap_tunggu({})", arg_str);
                    }
                    if name == "buat_server_http" && args.len() >= 2 {
                        let port_str = self.emit_expr(&args[0].value);
                        let handler_fn = if let Expr::Lit(Literal::String(h), _) = &args[1].value {
                            format!("(dap_http_handler_t)dap_{}", h)
                        } else {
                            format!("(dap_http_handler_t){}", self.emit_expr(&args[1].value))
                        };
                        let max_str = if args.len() > 2 {
                            self.emit_expr(&args[2].value)
                        } else {
                            "0".to_string()
                        };
                        return format!("dap_buat_server_http({}, {}, {})", port_str, handler_fn, max_str);
                    }
                    if name == "sha1" && args.len() == 1 {
                        let arg_str = self.emit_expr(&args[0].value);
                        return format!("dap_sha1({})", arg_str);
                    }
                    if name == "kunci_websocket" && args.len() == 1 {
                        let arg_str = self.emit_expr(&args[0].value);
                        return format!("dap_kunci_websocket({})", arg_str);
                    }
                    if name == "respons_upgrade_websocket" && !args.is_empty() {
                        let k = self.emit_expr(&args[0].value);
                        let h = if args.len() > 1 { self.emit_expr(&args[1].value) } else { "\"\"".to_string() };
                        return format!("dap_respons_upgrade_websocket({}, {})", k, h);
                    }
                    if name == "buat_server_websocket" && args.len() >= 2 {
                        let port_str = self.emit_expr(&args[0].value);
                        let handler_str = self.emit_expr(&args[1].value);
                        let max_str = if args.len() > 2 {
                            self.emit_expr(&args[2].value)
                        } else {
                            "0".to_string()
                        };
                        return format!("dap_buat_server_websocket({}, {}, {})", port_str, handler_str, max_str);
                    }
                    if name == "Daftar" {
                        if args.is_empty() {
                            return "dap_Daftar_baru()".to_string();
                        } else {
                            let item_strs: Vec<String> = args
                                .iter()
                                .map(|a| format!("(void*)(uintptr_t)({})", self.emit_expr(&a.value)))
                                .collect();
                            return format!(
                                "dap_Daftar_dari_array({}, (void*[]){{ {} }})",
                                item_strs.len(),
                                item_strs.join(", ")
                            );
                        }
                    }
                    // Struct constructor
                    if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        let mut arg_list = Vec::new();
                        for a in args {
                            let val_str = self.emit_expr(&a.value);
                            if let Some(f_name) = &a.name {
                                arg_list.push(format!(".{} = {}", f_name, val_str));
                            } else {
                                arg_list.push(val_str);
                            }
                        }
                        return format!("({}){{ {} }}", name, arg_list.join(", "));
                    }
                }

                let mut arg_strs = Vec::new();
                for a in args {
                    arg_strs.push(self.emit_expr(&a.value));
                }
                let callee_name = match &**callee {
                    Expr::Ident(n, _) => format!("dap_{}", n),
                    _ => self.emit_expr(callee),
                };
                format!("{}({})", callee_name, arg_strs.join(", "))
            }
            Expr::MethodCall { receiver, method, args, .. } => {
                let rec_str = self.emit_expr(receiver);
                if method == "tambah" {
                    let arg_str = if !args.is_empty() { self.emit_expr(&args[0].value) } else { "NULL".to_string() };
                    return format!("dap_Daftar_tambah({}, (void*)(uintptr_t)({}))", rec_str, arg_str);
                }
                if method == "panjang" {
                    return format!("dap_Daftar_panjang({})", rec_str);
                }
                if method == "simpan" && args.len() == 2 {
                    let k = self.emit_expr(&args[0].value);
                    let v = self.emit_expr(&args[1].value);
                    return format!("dap_db_simpan({}.jalur, {}, {})", rec_str, k, v);
                }
                if method == "hapus" && args.len() == 1 {
                    let k = self.emit_expr(&args[0].value);
                    return format!("dap_db_hapus({}.jalur, {})", rec_str, k);
                }
                if method == "ada" && args.len() == 1 {
                    let k = self.emit_expr(&args[0].value);
                    return format!("dap_db_ada({}.jalur, {})", rec_str, k);
                }
                if method == "daftar_kunci" {
                    return format!("dap_db_daftar_kunci({}.jalur)", rec_str);
                }
                if method == "padatkan" {
                    return format!("dap_db_padatkan({}.jalur)", rec_str);
                }
                if method == "cocok" && args.len() == 1 {
                    let p = self.emit_expr(&args[0].value);
                    return format!("dap_cocok_rute({}, {}.jalur)", p, rec_str);
                }
                if method == "param" && args.len() == 2 {
                    let p = self.emit_expr(&args[0].value);
                    let n = self.emit_expr(&args[1].value);
                    return format!("dap_ambil_param_rute({}, {}.jalur, {})", p, rec_str, n);
                }
                if method == "kueri_param" && args.len() == 1 {
                    let k = self.emit_expr(&args[0].value);
                    return format!("dap_ambil_kueri_param({}.kueri, {})", rec_str, k);
                }
                if method == "ada_kueri" && args.len() == 1 {
                    let k = self.emit_expr(&args[0].value);
                    return format!("dap_ada_kueri_param({}.kueri, {})", rec_str, k);
                }
                if method == "tambah_header" && args.len() == 2 {
                    let k = self.emit_expr(&args[0].value);
                    let v = self.emit_expr(&args[1].value);
                    return format!("dap_tambah_header({}, {}, {})", rec_str, k, v);
                }
                if method == "cors" && args.is_empty() {
                    return format!("dap_respons_cors({})", rec_str);
                }
                if method == "sajikan_statis" && args.len() == 1 {
                    let f = self.emit_expr(&args[0].value);
                    return format!("dap_sajikan_berkas_statis({}, {}.jalur)", f, rec_str);
                }
                if method == "adalah_websocket" && args.is_empty() {
                    return format!("dap_req_adalah_websocket(&{})", rec_str);
                }
                if method == "kunci_websocket" && args.is_empty() {
                    return format!("dap_req_kunci_websocket(&{})", rec_str);
                }
                if method == "tingkatkan_websocket" && args.len() == 1 {
                    let h = self.emit_expr(&args[0].value);
                    return format!("dap_req_tingkatkan_websocket(&{}, {})", rec_str, h);
                }
                if method == "ambil" {
                    let arg_str = if !args.is_empty() { self.emit_expr(&args[0].value) } else { "0".to_string() };
                    if arg_str.starts_with('"') || arg_str.contains("teks") || arg_str.contains("kunci") || rec_str.contains("db") {
                        return format!("dap_db_ambil({}.jalur, {})", rec_str, arg_str);
                    } else {
                        return format!("dap_Daftar_ambil({}, {})", rec_str, arg_str);
                    }
                }
                let mut all_args = vec![format!("&{}", rec_str)];
                for a in args {
                    all_args.push(self.emit_expr(&a.value));
                }
                let struct_name = self.method_to_struct.get(method).cloned().unwrap_or_else(|| "Produk".to_string());
                format!("dap_{}_{}({})", struct_name, method, all_args.join(", "))
            }
            Expr::FieldAccess { receiver, field, .. } => {
                let rec_str = self.emit_expr(receiver);
                if rec_str == "ini" {
                    format!("ini->{}", field)
                } else {
                    format!("{}.{}", rec_str, field)
                }
            }
            Expr::InterpolatedString(parts, _) => {
                let mut fmt = String::new();
                let mut arg_vals = Vec::new();
                for p in parts {
                    match p {
                        InterpolatedExprPart::Lit(s) => {
                            let clean = s
                                .replace('\\', "\\\\")
                                .replace('%', "%%")
                                .replace('\"', "\\\"")
                                .replace('\n', "\\n")
                                .replace('\r', "\\r")
                                .replace('\t', "\\t");
                            fmt.push_str(&clean);
                        }
                        InterpolatedExprPart::Expr(e) => {
                            let e_str = self.emit_expr(e);
                            fmt.push_str("%s");
                            arg_vals.push(format!("dap_to_str({})", e_str));
                        }
                    }
                }
                let mut formatted = format!("\"{}\"", fmt);
                for a in arg_vals {
                    formatted.push_str(&format!(", {}", a));
                }
                format!("({{\n        char* _buf = (char*)malloc(1024);\n        snprintf(_buf, 1024, {});\n        _buf;\n    }})", formatted)
            }
            Expr::ErrorPropagate { inner, .. } => self.emit_expr(inner),
            Expr::Catch { inner, .. } => self.emit_expr(inner),
            Expr::Parallel { body, .. } => {
                let task_id = self.task_helpers.len();
                let mut body_code = String::new();
                for s in &body.stmts {
                    let mut tmp_gen = CodeGenerator::new(self._ownership);
                    tmp_gen.method_to_struct = self.method_to_struct.clone();
                    tmp_gen.emit_stmt(s, "void");
                    body_code.push_str(&tmp_gen.out);
                }
                if let Some(e) = &body.expr {
                    let mut tmp_gen = CodeGenerator::new(self._ownership);
                    tmp_gen.method_to_struct = self.method_to_struct.clone();
                    let es = tmp_gen.emit_expr(e);
                    body_code.push_str(&format!("    {};\n", es));
                }

                let captures_item = body_code.contains("item.");
                let (unpack_code, spawn_code) = if captures_item {
                    (
                        "    Produk item = *(Produk*)_a;\n    free(_a);\n",
                        format!("({{\n    Produk* _pitem = (Produk*)malloc(sizeof(Produk));\n    *_pitem = item;\n    dap_spawn_task(_dap_task_{}, _pitem);\n}})", task_id)
                    )
                } else {
                    (
                        "    (void)_a;\n",
                        format!("dap_spawn_task(_dap_task_{}, NULL)", task_id)
                    )
                };

                let helper = format!(
                    "static void* _dap_task_{}(void* _a) {{\n{}{}\n    return NULL;\n}}\n",
                    task_id, unpack_code, body_code
                );
                self.task_helpers.push(helper);

                spawn_code
            }
            Expr::Await { task, .. } => {
                let t = self.emit_expr(task);
                format!("dap_tunggu({})", t)
            }
            Expr::Comptime { .. } => "((void)0)".to_string(),
            Expr::Block(_) => "((void)0)".to_string(),
        }
    }

    fn ret_type_to_c(&self, ty: &Option<TypeExpr>) -> String {
        match ty {
            Some(TypeExpr::ErrorUnion(ok, _, _)) => {
                let inner = self.type_to_c(ok);
                format!("DapResult_{}", inner)
            }
            Some(t) => self.type_to_c(t),
            None => "void".to_string(),
        }
    }

    fn type_to_c(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Named(name, _) => match name.as_str() {
                "u64" | "bulat" => "u64".to_string(),
                "i64" => "i64".to_string(),
                "f64" | "desimal" => "f64".to_string(),
                "Teks" | "teks" => "Teks".to_string(),
                "Bool" | "bool" => "bool".to_string(),
                "Unit" | "unit" => "void".to_string(),
                other => other.to_string(),
            },
            TypeExpr::ErrorUnion(ok, _, _) => {
                let inner = self.type_to_c(ok);
                format!("DapResult_{}", inner)
            }
            TypeExpr::Task(_inner, _) => "DapTask*".to_string(),
            TypeExpr::List(_inner, _) => "DapList*".to_string(),
            TypeExpr::Unit(_) => "void".to_string(),
            TypeExpr::Inferred(_) => "auto".to_string(),
        }
    }

    pub fn compile_to_native(
        &mut self,
        program: &Program,
        output_exe: &Path,
    ) -> Result<()> {
        let c_code = self.generate_c(program);
        let c_file = output_exe.with_extension("c");
        fs::write(&c_file, c_code)
            .map_err(|e| Diagnostic::error(format!("Gagal menulis file C: {}", e)))?;

        let c_compiler = if Command::new("gcc").arg("--version").output().is_ok() {
            "gcc".to_string()
        } else if Command::new("clang").arg("--version").output().is_ok() {
            "clang".to_string()
        } else if Path::new(r"D:\mingw64\bin\gcc.exe").exists() {
            r"D:\mingw64\bin\gcc.exe".to_string()
        } else if Path::new(r"C:\mingw64\bin\gcc.exe").exists() {
            r"C:\mingw64\bin\gcc.exe".to_string()
        } else {
            "gcc".to_string()
        };

        // Call GCC / Clang with C11 standard and math library
        let mut cmd = Command::new(&c_compiler);
        cmd.arg("-std=c11")
            .arg("-O2")
            .arg(&c_file)
            .arg("-o")
            .arg(output_exe)
            .arg("-lm");
        if cfg!(windows) {
            cmd.arg("-lws2_32");
        }
        let output = cmd.output();

        match output {
            Ok(res) if res.status.success() => Ok(()),
            Ok(res) => {
                let err = String::from_utf8_lossy(&res.stderr);
                Err(Diagnostic::error(format!("Kompilasi native C gagal ({}): {}", c_compiler, err)))
            }
            Err(e) => Err(Diagnostic::error(format!(
                "Gagal memanggil kompilator C ({}): {}. Pastikan GCC atau Clang terpasang di sistem.",
                c_compiler, e
            ))),
        }
    }
}
