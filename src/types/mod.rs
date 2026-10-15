use crate::ast::*;
use crate::diag::{Diagnostic, Result};
use crate::span::Span;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    U64,
    I64,
    F64,
    String, // Teks
    Bool,
    Unit,
    Struct(String),
    Trait(String),
    ErrorUnion(Box<Type>, Box<Type>), // T ! E
    Task(Box<Type>),                  // Tugas<T>
    List(Box<Type>),                  // Daftar<T>
    Fn(Vec<Type>, Box<Type>),
    Var(TypeVarId),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::U64 => write!(f, "u64"),
            Type::I64 => write!(f, "i64"),
            Type::F64 => write!(f, "f64"),
            Type::String => write!(f, "Teks"),
            Type::Bool => write!(f, "Bool"),
            Type::Unit => write!(f, "()"),
            Type::Struct(name) => write!(f, "{}", name),
            Type::Trait(name) => write!(f, "sifat {}", name),
            Type::ErrorUnion(ok, err) => write!(f, "{} ! {}", ok, err),
            Type::Task(inner) => write!(f, "Tugas<{}>", inner),
            Type::List(inner) => write!(f, "Daftar<{}>", inner),
            Type::Fn(params, ret) => {
                write!(f, "fungsi(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Type::Var(id) => write!(f, "?T{}", id.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: HashMap<String, (usize, Type)>, // field_name -> (index, type)
    pub field_order: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TraitInfo {
    pub name: String,
    pub methods: HashMap<String, (Vec<Type>, Type)>, // method_name -> (param_types, ret_type)
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub param_names: Vec<String>,
    pub param_types: Vec<Type>,
    pub ret_type: Type,
    pub span: Span,
}

pub struct TypeChecker {
    next_var_id: usize,
    subst: HashMap<TypeVarId, Type>,
    structs: HashMap<String, StructInfo>,
    traits: HashMap<String, TraitInfo>,
    // struct_name -> (trait_name -> Vec<method_names>)
    impls: HashMap<String, HashMap<Option<String>, HashMap<String, MethodInfo>>>,
    // global function signatures: fn_name -> (param_types, ret_type)
    functions: HashMap<String, (Vec<Type>, Type)>,
    // Scopes for local variables: var_name -> Type
    scopes: Vec<HashMap<String, Type>>,
    // Current function return type being checked
    current_fn_ret: Option<Type>,
    _file_id: usize,
}

impl TypeChecker {
    pub fn new(file_id: usize) -> Self {
        let mut tc = TypeChecker {
            next_var_id: 0,
            subst: HashMap::new(),
            structs: HashMap::new(),
            traits: HashMap::new(),
            impls: HashMap::new(),
            functions: HashMap::new(),
            scopes: vec![HashMap::new()],
            current_fn_ret: None,
            _file_id: file_id,
        };
        tc.register_builtins();
        tc
    }

    pub fn fresh_var(&mut self) -> Type {
        let id = TypeVarId(self.next_var_id);
        self.next_var_id += 1;
        Type::Var(id)
    }

    pub fn resolve_type(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(id) => {
                if let Some(concrete) = self.subst.get(id) {
                    self.resolve_type(concrete)
                } else {
                    ty.clone()
                }
            }
            Type::ErrorUnion(ok, err) => Type::ErrorUnion(
                Box::new(self.resolve_type(ok)),
                Box::new(self.resolve_type(err)),
            ),
            Type::Task(inner) => Type::Task(Box::new(self.resolve_type(inner))),
            Type::List(inner) => Type::List(Box::new(self.resolve_type(inner))),
            Type::Fn(params, ret) => Type::Fn(
                params.iter().map(|p| self.resolve_type(p)).collect(),
                Box::new(self.resolve_type(ret)),
            ),
            _ => ty.clone(),
        }
    }

    pub fn register_builtins(&mut self) {
        if self.functions.contains_key("cetak") {
            return;
        }
        // Register built-in functions
        let cetak_arg = self.fresh_var();
        self.functions.insert(
            "cetak".to_string(),
            (vec![cetak_arg], Type::Unit),
        );

        // Standard Library: File I/O
        self.functions.insert(
            "baca_berkas".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "tulis_berkas".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ada_berkas".to_string(),
            (vec![Type::String], Type::Bool),
        );
        self.functions.insert(
            "buat_direktori".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "daftar_direktori".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(
                    Box::new(Type::List(Box::new(Type::String))),
                    Box::new(Type::String),
                ),
            ),
        );
        self.functions.insert(
            "hapus_berkas".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "hapus_direktori".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "gabung_jalur".to_string(),
            (vec![Type::String, Type::String], Type::String),
        );

        // Standard Library: String Utilities
        self.functions.insert(
            "panjang_teks".to_string(),
            (vec![Type::String], Type::U64),
        );
        self.functions.insert(
            "ke_huruf_besar".to_string(),
            (vec![Type::String], Type::String),
        );
        self.functions.insert(
            "ke_huruf_kecil".to_string(),
            (vec![Type::String], Type::String),
        );

        // Standard Library: Scientific Math, Trigonometry & Constants
        self.functions.insert(
            "sin".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "cos".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "tan".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "asin".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "acos".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "atan".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "atan2".to_string(),
            (vec![Type::F64, Type::F64], Type::F64),
        );
        self.functions.insert(
            "akar_kuadrat".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "akar_kubik".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "pangkat".to_string(),
            (vec![Type::F64, Type::F64], Type::F64),
        );
        self.functions.insert(
            "nilai_mutlak".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "bulat".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "lantai".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "langit_langit".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "tanda".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "log".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "log10".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "log2".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "exp".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "min".to_string(),
            (vec![Type::F64, Type::F64], Type::F64),
        );
        self.functions.insert(
            "max".to_string(),
            (vec![Type::F64, Type::F64], Type::F64),
        );
        self.functions.insert(
            "ke_radian".to_string(),
            (vec![Type::F64], Type::F64),
        );
        self.functions.insert(
            "ke_derajat".to_string(),
            (vec![Type::F64], Type::F64),
        );

        // Standard Library: Time
        self.functions.insert(
            "waktu_sekarang_ms".to_string(),
            (vec![], Type::U64),
        );
        self.functions.insert(
            "tidur".to_string(),
            (vec![Type::U64], Type::Unit),
        );

        // Standard Library: Environment & CLI Arguments
        self.functions.insert(
            "ambil_argumen_cli".to_string(),
            (
                vec![],
                Type::List(Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ambil_lingkungan".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "atur_lingkungan".to_string(),
            (
                vec![Type::String, Type::String],
                Type::Unit,
            ),
        );

        // Standard Library: JSON Utilities
        let json_val_arg = self.fresh_var();
        self.functions.insert(
            "ke_json".to_string(),
            (vec![json_val_arg], Type::String),
        );
        self.functions.insert(
            "uraikan_json".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ambil_kunci_json".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ambil_angka_json".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::F64), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ambil_bool_json".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ada_kunci_json".to_string(),
            (vec![Type::String, Type::String], Type::Bool),
        );
        self.functions.insert(
            "valid_json".to_string(),
            (vec![Type::String], Type::Bool),
        );

        // Standard Library: HTTP Network Utilities
        self.functions.insert(
            "ambil_http".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "kirim_http_post".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ambil_status_http".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::U64), Box::new(Type::String)),
            ),
        );

        // Standard Library: Test Assertions
        self.functions.insert(
            "pastikan".to_string(),
            (vec![Type::Bool], Type::Unit),
        );
        let pastikan_arg1 = self.fresh_var();
        let pastikan_arg2 = pastikan_arg1.clone();
        self.functions.insert(
            "pastikan_sama".to_string(),
            (vec![pastikan_arg1, pastikan_arg2], Type::Unit),
        );
        let pastikan_tdk1 = self.fresh_var();
        let pastikan_tdk2 = pastikan_tdk1.clone();
        self.functions.insert(
            "pastikan_tidak_sama".to_string(),
            (vec![pastikan_tdk1, pastikan_tdk2], Type::Unit),
        );

        // Standard Library: Cryptography & Encoding
        self.functions.insert(
            "sha1".to_string(),
            (vec![Type::String], Type::String),
        );
        self.functions.insert(
            "sha256".to_string(),
            (vec![Type::String], Type::String),
        );
        self.functions.insert(
            "md5".to_string(),
            (vec![Type::String], Type::String),
        );
        self.functions.insert(
            "ke_base64".to_string(),
            (vec![Type::String], Type::String),
        );
        self.functions.insert(
            "dari_base64".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );

        // Standard Library: HTTP Web Server
        let mut req_fields = HashMap::new();
        req_fields.insert("metode".to_string(), (0, Type::String));
        req_fields.insert("jalur".to_string(), (1, Type::String));
        req_fields.insert("kueri".to_string(), (2, Type::String));
        req_fields.insert("isi".to_string(), (3, Type::String));
        self.structs.insert(
            "PermintaanHttp".to_string(),
            StructInfo {
                name: "PermintaanHttp".to_string(),
                fields: req_fields,
                field_order: vec!["metode".to_string(), "jalur".to_string(), "kueri".to_string(), "isi".to_string()],
                span: Span::DUMMY,
            },
        );

        let mut resp_fields = HashMap::new();
        resp_fields.insert("status".to_string(), (0, Type::U64));
        resp_fields.insert("tipe_konten".to_string(), (1, Type::String));
        resp_fields.insert("konten".to_string(), (2, Type::String));
        resp_fields.insert("header".to_string(), (3, Type::String));
        self.structs.insert(
            "ResponsHttp".to_string(),
            StructInfo {
                name: "ResponsHttp".to_string(),
                fields: resp_fields,
                field_order: vec![
                    "status".to_string(),
                    "tipe_konten".to_string(),
                    "konten".to_string(),
                    "header".to_string(),
                ],
                span: Span::DUMMY,
            },
        );

        self.functions.insert(
            "kirim_respons_http".to_string(),
            (
                vec![Type::U64, Type::String, Type::String],
                Type::Struct("ResponsHttp".to_string()),
            ),
        );
        self.functions.insert(
            "respons_json".to_string(),
            (
                vec![Type::String],
                Type::Struct("ResponsHttp".to_string()),
            ),
        );
        self.functions.insert(
            "respons_teks".to_string(),
            (
                vec![Type::String],
                Type::Struct("ResponsHttp".to_string()),
            ),
        );
        self.functions.insert(
            "tambah_header".to_string(),
            (
                vec![
                    Type::Struct("ResponsHttp".to_string()),
                    Type::String,
                    Type::String,
                ],
                Type::Struct("ResponsHttp".to_string()),
            ),
        );
        self.functions.insert(
            "respons_cors".to_string(),
            (
                vec![Type::Struct("ResponsHttp".to_string())],
                Type::Struct("ResponsHttp".to_string()),
            ),
        );

        // Standard Library: Web Router & URL Query Primitives
        self.functions.insert(
            "cocok_rute".to_string(),
            (vec![Type::String, Type::String], Type::Bool),
        );
        self.functions.insert(
            "ambil_param_rute".to_string(),
            (
                vec![Type::String, Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ambil_kueri_param".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ada_kueri_param".to_string(),
            (vec![Type::String, Type::String], Type::Bool),
        );
        self.functions.insert(
            "respons_berkas".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(
                    Box::new(Type::Struct("ResponsHttp".to_string())),
                    Box::new(Type::String),
                ),
            ),
        );
        self.functions.insert(
            "sajikan_berkas_statis".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(
                    Box::new(Type::Struct("ResponsHttp".to_string())),
                    Box::new(Type::String),
                ),
            ),
        );
        self.functions.insert(
            "kunci_websocket".to_string(),
            (vec![Type::String], Type::String),
        );
        self.functions.insert(
            "respons_upgrade_websocket".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(
                    Box::new(Type::Struct("ResponsHttp".to_string())),
                    Box::new(Type::String),
                ),
            ),
        );
        self.functions.insert(
            "buat_server_websocket".to_string(),
            (
                vec![Type::U64, Type::String],
                Type::ErrorUnion(Box::new(Type::U64), Box::new(Type::String)),
            ),
        );

        // Standard Library: Embedded Database (BasisData)
        let mut db_fields = HashMap::new();
        db_fields.insert("jalur".to_string(), (0, Type::String));
        self.structs.insert(
            "BasisData".to_string(),
            StructInfo {
                name: "BasisData".to_string(),
                fields: db_fields,
                field_order: vec!["jalur".to_string()],
                span: Span::DUMMY,
            },
        );

        self.functions.insert(
            "buka_basis_data".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(
                    Box::new(Type::Struct("BasisData".to_string())),
                    Box::new(Type::String),
                ),
            ),
        );
        self.functions.insert(
            "db_simpan".to_string(),
            (
                vec![Type::String, Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_ambil".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_hapus".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_ada".to_string(),
            (
                vec![Type::String, Type::String],
                Type::Bool,
            ),
        );
        self.functions.insert(
            "db_daftar_kunci".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(
                    Box::new(Type::List(Box::new(Type::String))),
                    Box::new(Type::String),
                ),
            ),
        );
        self.functions.insert(
            "db_padatkan".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_ambil_awalan".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(
                    Box::new(Type::List(Box::new(Type::String))),
                    Box::new(Type::String),
                ),
            ),
        );
        self.functions.insert(
            "db_mulai_transaksi".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_sedang_transaksi".to_string(),
            (vec![Type::String], Type::Bool),
        );
        self.functions.insert(
            "db_komit".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_batal".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_ekspor_json".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_ekspor_json_awalan".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "db_impor_json".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::U64), Box::new(Type::String)),
            ),
        );

        // Standard Library: Reguler Ekspresi (Regex)
        self.functions.insert(
            "cocok_regex".to_string(),
            (vec![Type::String, Type::String], Type::Bool),
        );
        self.functions.insert(
            "cari_regex".to_string(),
            (
                vec![Type::String, Type::String],
                Type::List(Box::new(Type::String)),
            ),
        );
        self.functions.insert(
            "ganti_regex".to_string(),
            (
                vec![Type::String, Type::String, Type::String],
                Type::String,
            ),
        );
        self.functions.insert(
            "ekstrak_regex".to_string(),
            (
                vec![Type::String, Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );

        // Standard Library: WebAssembly HTML5 Canvas 2D
        self.functions.insert(
            "kanvas_bersihkan".to_string(),
            (vec![Type::F64, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_warna_isi".to_string(),
            (vec![Type::F64, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_warna_garis".to_string(),
            (vec![Type::F64, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_tebal_garis".to_string(),
            (vec![Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_persegi".to_string(),
            (vec![Type::F64, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_bingkai_persegi".to_string(),
            (vec![Type::F64, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_lingkaran".to_string(),
            (vec![Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_bingkai_lingkaran".to_string(),
            (vec![Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_garis".to_string(),
            (vec![Type::F64, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_titik".to_string(),
            (vec![Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_teks".to_string(),
            (vec![Type::String, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "kanvas_lebar".to_string(),
            (vec![], Type::F64),
        );
        self.functions.insert(
            "kanvas_tinggi".to_string(),
            (vec![], Type::F64),
        );
        self.functions.insert(
            "kanvas_mouse_x".to_string(),
            (vec![], Type::F64),
        );
        self.functions.insert(
            "kanvas_mouse_y".to_string(),
            (vec![], Type::F64),
        );
        self.functions.insert(
            "kanvas_mouse_klik".to_string(),
            (vec![], Type::U64),
        );
        self.functions.insert(
            "kanvas_tombol_ditekan".to_string(),
            (vec![Type::U64], Type::U64),
        );
        self.functions.insert(
            "kanvas_delta_waktu".to_string(),
            (vec![], Type::F64),
        );

        // Standard Library: Web Audio Synthesizer
        self.functions.insert(
            "audio_mainkan_nada".to_string(),
            (vec![Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "audio_mainkan_gelombang".to_string(),
            (vec![Type::U64, Type::F64, Type::F64, Type::F64], Type::Unit),
        );
        self.functions.insert(
            "audio_efek_suara".to_string(),
            (vec![Type::U64], Type::Unit),
        );
        self.functions.insert(
            "audio_frekuensi_nada".to_string(),
            (vec![Type::String], Type::F64),
        );
        self.functions.insert(
            "audio_set_volume".to_string(),
            (vec![Type::F64], Type::Unit),
        );
        self.functions.insert(
            "audio_hentikan_semua".to_string(),
            (vec![], Type::Unit),
        );

        // Compiler Service: In-Memory WebAssembly Compilation
        self.functions.insert(
            "kompilasi_ke_wasm".to_string(),
            (
                vec![Type::String],
                Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)),
            ),
        );
    }

    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        self.register_builtins();

        // 1. First pass: Collect all struct declarations
        for decl in &program.decls {
            if let Decl::Struct(s) = decl {
                if self.structs.contains_key(&s.name) {
                    return Err(Diagnostic::error(format!("Struktur '{}' sudah dideklarasikan", s.name))
                        .with_span(s.span));
                }
                let mut fields = HashMap::new();
                let mut field_order = Vec::new();
                for (idx, f) in s.fields.iter().enumerate() {
                    let fty = self.type_expr_to_type(&f.ty)?;
                    fields.insert(f.name.clone(), (idx, fty));
                    field_order.push(f.name.clone());
                }
                self.structs.insert(
                    s.name.clone(),
                    StructInfo {
                        name: s.name.clone(),
                        fields,
                        field_order,
                        span: s.span,
                    },
                );
            }
        }

        // 2. Second pass: Collect all traits
        for decl in &program.decls {
            if let Decl::Trait(t) = decl {
                if self.traits.contains_key(&t.name) {
                    return Err(Diagnostic::error(format!("Sifat '{}' sudah dideklarasikan", t.name))
                        .with_span(t.span));
                }
                let mut methods = HashMap::new();
                for m in &t.methods {
                    let mut param_types = Vec::new();
                    for p in &m.params {
                        if p.is_self {
                            // Self type placeholder
                            param_types.push(Type::Trait(t.name.clone()));
                        } else if let Some(pty) = &p.ty {
                            param_types.push(self.type_expr_to_type(pty)?);
                        } else {
                            param_types.push(self.fresh_var());
                        }
                    }
                    let ret_ty = if let Some(r) = &m.ret_ty {
                        self.type_expr_to_type(r)?
                    } else {
                        Type::Unit
                    };
                    methods.insert(m.name.clone(), (param_types, ret_ty));
                }
                self.traits.insert(
                    t.name.clone(),
                    TraitInfo {
                        name: t.name.clone(),
                        methods,
                        span: t.span,
                    },
                );
            }
        }

        // 3. Third pass: Collect global function signatures and impl blocks
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    let mut param_types = Vec::new();
                    for p in &f.params {
                        if let Some(pty) = &p.ty {
                            param_types.push(self.type_expr_to_type(pty)?);
                        } else {
                            param_types.push(self.fresh_var());
                        }
                    }
                    let ret_ty = if let Some(r) = &f.ret_ty {
                        self.type_expr_to_type(r)?
                    } else {
                        Type::Unit
                    };
                    self.functions.insert(f.name.clone(), (param_types, ret_ty));
                }
                Decl::Impl(i) => {
                    if !self.structs.contains_key(&i.target_struct) {
                        return Err(Diagnostic::error(format!(
                            "Struktur '{}' tidak ditemukan untuk blok terapkan",
                            i.target_struct
                        ))
                        .with_span(i.span));
                    }
                    if let Some(trait_name) = &i.trait_name {
                        if !self.traits.contains_key(trait_name) {
                            return Err(Diagnostic::error(format!(
                                "Sifat '{}' tidak ditemukan untuk blok terapkan",
                                trait_name
                            ))
                            .with_span(i.span));
                        }
                    }

                    let mut collected_methods = Vec::new();
                    for m in &i.methods {
                        let mut param_names = Vec::new();
                        let mut param_types = Vec::new();
                        for p in &m.params {
                            param_names.push(p.name.clone());
                            if p.is_self {
                                param_types.push(Type::Struct(i.target_struct.clone()));
                            } else if let Some(pty) = &p.ty {
                                param_types.push(self.type_expr_to_type(pty)?);
                            } else {
                                param_types.push(self.fresh_var());
                            }
                        }
                        let ret_type = if let Some(r) = &m.ret_ty {
                            self.type_expr_to_type(r)?
                        } else {
                            Type::Unit
                        };
                        collected_methods.push((
                            m.name.clone(),
                            MethodInfo {
                                name: m.name.clone(),
                                param_names,
                                param_types,
                                ret_type,
                                span: m.span,
                            },
                        ));
                    }

                    let struct_impls = self
                        .impls
                        .entry(i.target_struct.clone())
                        .or_default();
                    let trait_methods = struct_impls.entry(i.trait_name.clone()).or_default();
                    for (m_name, m_info) in collected_methods {
                        trait_methods.insert(m_name, m_info);
                    }
                }
                _ => {}
            }
        }

        // 4. Verify Trait Implementation completeness
        for decl in &program.decls {
            if let Decl::Impl(i) = decl {
                if let Some(trait_name) = &i.trait_name {
                    let trait_info = self.traits.get(trait_name).unwrap().clone();
                    for (req_m_name, (req_params, req_ret)) in &trait_info.methods {
                        let struct_impls = self.impls.get(&i.target_struct).unwrap();
                        let methods = struct_impls.get(&i.trait_name).unwrap();
                        match methods.get(req_m_name) {
                            None => {
                                return Err(Diagnostic::error(format!(
                                    "Struktur '{}' belum mengimplementasikan fungsi '{}' yang diwajibkan oleh sifat '{}'",
                                    i.target_struct, req_m_name, trait_name
                                ))
                                .with_span(i.span));
                            }
                            Some(impl_m) => {
                                if impl_m.param_types.len() != req_params.len() {
                                    return Err(Diagnostic::error(format!(
                                        "Jumlah parameter fungsi '{}' ({}) tidak cocok dengan sifat '{}' ({})",
                                        req_m_name, impl_m.param_types.len(), trait_name, req_params.len()
                                    ))
                                    .with_span(impl_m.span));
                                }
                                // Check return type
                                if !self.types_match(&impl_m.ret_type, req_ret) {
                                    return Err(Diagnostic::error(format!(
                                        "Tipe kembalian fungsi '{}' ({}) tidak cocok dengan sifat '{}' ({})",
                                        req_m_name, impl_m.ret_type, trait_name, req_ret
                                    ))
                                    .with_span(impl_m.span));
                                }
                            }
                        }
                    }
                }
            }
        }

        // 5. Check all function bodies and method bodies
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    self.check_fn(f, None)?;
                }
                Decl::Impl(i) => {
                    for m in &i.methods {
                        self.check_fn(m, Some(&i.target_struct))?;
                    }
                }
                Decl::Test(t) => {
                    self.check_test(t)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn check_fn(&mut self, f: &FnDecl, self_struct: Option<&str>) -> Result<()> {
        let (param_types, ret_ty) = if let Some(s_name) = self_struct {
            let struct_impls = self.impls.get(s_name).unwrap();
            let mut found = None;
            for methods in struct_impls.values() {
                if let Some(m) = methods.get(&f.name) {
                    found = Some((m.param_types.clone(), m.ret_type.clone()));
                    break;
                }
            }
            found.unwrap()
        } else {
            self.functions.get(&f.name).unwrap().clone()
        };

        self.current_fn_ret = Some(ret_ty.clone());
        self.enter_scope();

        for (idx, p) in f.params.iter().enumerate() {
            let p_ty = param_types[idx].clone();
            self.insert_var(&p.name, p_ty);
        }

        let body_ty = self.check_block(&f.body)?;

        // If body has a trailing expression, check if it matches return type or wraps in ErrorUnion
        if let Some(final_expr) = &f.body.expr {
            self.unify_return(&body_ty, &ret_ty, final_expr.span())?;
        } else if !self.types_match(&body_ty, &Type::Unit) && !self.types_match(&ret_ty, &Type::Unit) {
            // If body diverges or returns Unit when non-Unit is expected
            // (Functions without trailing expr or return returning non-unit)
            if !matches!(ret_ty, Type::Unit) {
                // Check if any statement returns
                let has_return = f.body.stmts.iter().any(|s| matches!(s, Stmt::Return { .. }));
                if !has_return {
                    return Err(Diagnostic::error(format!(
                        "Fungsi '{}' diharapkan mengembalikan tipe '{}', tetapi tidak mengembalikan nilai apapun",
                        f.name, ret_ty
                    ))
                    .with_span(f.span));
                }
            }
        }

        self.exit_scope();
        self.current_fn_ret = None;
        Ok(())
    }

    fn check_test(&mut self, t: &TestDecl) -> Result<()> {
        self.current_fn_ret = Some(Type::Unit);
        self.enter_scope();
        let _ = self.check_block(&t.body)?;
        self.exit_scope();
        self.current_fn_ret = None;
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<Type> {
        self.enter_scope();
        let mut last_ty = Type::Unit;

        for stmt in &block.stmts {
            last_ty = self.check_stmt(stmt)?;
        }

        if let Some(expr) = &block.expr {
            last_ty = self.check_expr(expr)?;
        }

        self.exit_scope();
        Ok(last_ty)
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<Type> {
        match stmt {
            Stmt::VarDecl {
                name,
                explicit_ty,
                init,
                span,
            } => {
                let init_ty = self.check_expr(init)?;
                let final_ty = if let Some(e_ty) = explicit_ty {
                    let expected = self.type_expr_to_type(e_ty)?;
                    self.unify(&init_ty, &expected, *span)?;
                    expected
                } else {
                    init_ty
                };
                self.insert_var(name, final_ty);
                Ok(Type::Unit)
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                let target_ty = self.check_expr(target)?;
                let val_ty = self.check_expr(value)?;
                self.unify(&target_ty, &val_ty, *span)?;
                Ok(Type::Unit)
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_ty = self.check_expr(cond)?;
                self.unify(&cond_ty, &Type::Bool, cond.span())?;
                let then_ty = self.check_block(then_branch)?;
                if let Some(eb) = else_branch {
                    let else_ty = self.check_block(eb)?;
                    self.unify(&then_ty, &else_ty, *span)?;
                }
                Ok(Type::Unit)
            }
            Stmt::While { cond, body, .. } => {
                let cond_ty = self.check_expr(cond)?;
                self.unify(&cond_ty, &Type::Bool, cond.span())?;
                self.check_block(body)?;
                Ok(Type::Unit)
            }
            Stmt::ForIn { var_name, iterable, body, span } => {
                let iter_ty = self.check_expr(iterable)?;
                let resolved = self.resolve_type(&iter_ty);
                let elem_ty = match resolved {
                    Type::List(elem) => *elem,
                    other => {
                        return Err(Diagnostic::error(format!(
                            "Hanya dapat melakukan perulangan 'untuk' pada 'Daftar', tetapi menemukan '{}'",
                            other
                        )).with_span(*span));
                    }
                };
                self.enter_scope();
                self.insert_var(var_name, elem_ty);
                self.check_block(body)?;
                self.exit_scope();
                Ok(Type::Unit)
            }
            Stmt::Return { value, span } => {
                let ret_expected = self.current_fn_ret.clone().unwrap_or(Type::Unit);
                if let Some(expr) = value {
                    let expr_ty = self.check_expr(expr)?;
                    self.unify_return(&expr_ty, &ret_expected, *span)?;
                } else {
                    self.unify(&Type::Unit, &ret_expected, *span)?;
                }
                Ok(Type::Unit)
            }
            Stmt::Fail { value, span } => {
                // gagal <expr>: verifies that current fn return type is ErrorUnion(T, E) and expr unifies with E
                let err_ty = self.check_expr(value)?;
                let ret_expected = self.current_fn_ret.clone().unwrap_or(Type::Unit);
                match self.resolve_type(&ret_expected) {
                    Type::ErrorUnion(_, expected_err) => {
                        self.unify(&err_ty, &expected_err, *span)?;
                    }
                    Type::Unit => {
                        // Diizinkan pada blok pengujian (uji) atau fungsi Unit untuk memicu kegagalan
                    }
                    other => {
                        return Err(Diagnostic::error(format!(
                            "'gagal' hanya dapat digunakan pada fungsi dengan tipe kembalian Error Union (T ! E), tetapi tipe saat ini adalah '{}'",
                            other
                        ))
                        .with_span(*span));
                    }
                }
                Ok(Type::Unit)
            }
            Stmt::Expr(expr) => self.check_expr(expr),
        }
    }

    pub fn check_expr(&mut self, expr: &Expr) -> Result<Type> {
        match expr {
            Expr::Lit(lit, _) => Ok(match lit {
                Literal::Int(_) => Type::U64,
                Literal::Float(_) => Type::F64,
                Literal::String(_) => Type::String,
                Literal::Bool(_) => Type::Bool,
                Literal::Unit => Type::Unit,
            }),
            Expr::InterpolatedString(parts, _) => {
                for part in parts {
                    if let InterpolatedExprPart::Expr(e) = part {
                        self.check_expr(e)?;
                    }
                }
                Ok(Type::String)
            }
            Expr::Ident(name, span) => {
                if let Some(ty) = self.lookup_var(name) {
                    Ok(ty)
                } else if self.structs.contains_key(name) {
                    Ok(Type::Struct(name.clone()))
                } else if let Some((param_types, ret_ty)) = self.functions.get(name).cloned() {
                    Ok(Type::Fn(param_types, Box::new(ret_ty)))
                } else if name == "PI" || name == "TAU" || name == "E" {
                    Ok(Type::F64)
                } else {
                    Err(Diagnostic::error(format!("Variabel atau fungsi '{}' tidak ditemukan", name))
                        .with_span(*span))
                }
            }
            Expr::SelfVar(span) => {
                if let Some(ty) = self.lookup_var("ini") {
                    Ok(ty)
                } else {
                    Err(Diagnostic::error("'ini' hanya dapat digunakan di dalam fungsi anggota (metode)")
                        .with_span(*span))
                }
            }
            Expr::Binary(left, op, right, span) => {
                let l_ty = self.check_expr(left)?;
                let r_ty = self.check_expr(right)?;
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                        self.unify(&l_ty, &r_ty, *span)?;
                        let resolved = self.resolve_type(&l_ty);
                        if matches!(resolved, Type::U64 | Type::I64 | Type::F64) {
                            Ok(resolved)
                        } else if matches!(resolved, Type::String) && *op == BinaryOp::Add {
                            Ok(Type::String)
                        } else {
                            Err(Diagnostic::error(format!(
                                "Operator '{}' tidak didukung untuk tipe '{}'",
                                op, resolved
                            ))
                            .with_span(*span))
                        }
                    }
                    BinaryOp::Eq | BinaryOp::NotEq => {
                        self.unify(&l_ty, &r_ty, *span)?;
                        Ok(Type::Bool)
                    }
                    BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                        self.unify(&l_ty, &r_ty, *span)?;
                        Ok(Type::Bool)
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        self.unify(&l_ty, &Type::Bool, left.span())?;
                        self.unify(&r_ty, &Type::Bool, right.span())?;
                        Ok(Type::Bool)
                    }
                }
            }
            Expr::Unary(op, operand, span) => {
                let op_ty = self.check_expr(operand)?;
                match op {
                    UnaryOp::Neg => {
                        let resolved = self.resolve_type(&op_ty);
                        if matches!(resolved, Type::U64 | Type::I64 | Type::F64) {
                            Ok(resolved)
                        } else {
                            Err(Diagnostic::error(format!("Operator '-' tidak didukung untuk tipe '{}'", resolved))
                                .with_span(*span))
                        }
                    }
                    UnaryOp::Not => {
                        self.unify(&op_ty, &Type::Bool, *span)?;
                        Ok(Type::Bool)
                    }
                }
            }
            Expr::Call { callee, args, span } => {
                // Check if callee is `cetak` (variadic builtin)
                if let Expr::Ident(name, _) = &**callee {
                    if name == "cetak" {
                        for arg in args {
                            self.check_expr(&arg.value)?;
                        }
                        return Ok(Type::Unit);
                    }
                }

                // Check if callee is Daftar constructor
                if let Expr::Ident(name, _) = &**callee {
                    if name == "Daftar" {
                        let elem_ty = self.fresh_var();
                        for arg in args {
                            let arg_ty = self.check_expr(&arg.value)?;
                            self.unify(&arg_ty, &elem_ty, arg.span)?;
                        }
                        return Ok(Type::List(Box::new(elem_ty)));
                    }
                }

                // Check if callee is a Struct instantiation: `Produk(id: id, nama: "...", harga: ...)`
                if let Expr::Ident(name, _) = &**callee {
                    if let Some(s_info) = self.structs.get(name).cloned() {
                        return self.check_struct_instantiation(&s_info, args, *span);
                    }
                }

                // Check if callee is `tunggu`, `ke_json`, or `cetak`
                if let Expr::Ident(name, _) = &**callee {
                    if name == "tunggu" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'tunggu' membutuhkan tepat 1 argumen (tugas)")
                                .with_span(*span));
                        }
                        let task_ty = self.check_expr(&args[0].value)?;
                        match self.resolve_type(&task_ty) {
                            Type::Task(inner) => return Ok(*inner),
                            other => {
                                return Err(Diagnostic::error(format!(
                                    "'tunggu' membutuhkan objek 'Tugas', tetapi menemukan '{}'",
                                    other
                                ))
                                .with_span(*span));
                            }
                        }
                    }
                    if name == "ke_json" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'ke_json' membutuhkan tepat 1 argumen")
                                .with_span(*span));
                        }
                        self.check_expr(&args[0].value)?;
                        return Ok(Type::String);
                    }
                    if name == "cetak" {
                        for a in args {
                            self.check_expr(&a.value)?;
                        }
                        return Ok(Type::Unit);
                    }
                    if name == "pastikan" {
                        if args.len() != 1 {
                            return Err(Diagnostic::error("'pastikan' membutuhkan tepat 1 argumen (kondisi: bool)")
                                .with_span(*span));
                        }
                        let cond_ty = self.check_expr(&args[0].value)?;
                        self.unify(&cond_ty, &Type::Bool, args[0].span)?;
                        return Ok(Type::Unit);
                    }
                    if name == "pastikan_sama" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'pastikan_sama' membutuhkan tepat 2 argumen (sebenarnya, diharapkan)")
                                .with_span(*span));
                        }
                        let a_ty = self.check_expr(&args[0].value)?;
                        let b_ty = self.check_expr(&args[1].value)?;
                        self.unify(&a_ty, &b_ty, *span)?;
                        return Ok(Type::Unit);
                    }
                    if name == "pastikan_tidak_sama" {
                        if args.len() != 2 {
                            return Err(Diagnostic::error("'pastikan_tidak_sama' membutuhkan tepat 2 argumen (a, b)")
                                .with_span(*span));
                        }
                        let a_ty = self.check_expr(&args[0].value)?;
                        let b_ty = self.check_expr(&args[1].value)?;
                        self.unify(&a_ty, &b_ty, *span)?;
                        return Ok(Type::Unit);
                    }
                    if name == "buat_server_http" || name == "buat_server_websocket" {
                        if args.len() < 2 || args.len() > 3 {
                            return Err(Diagnostic::error(
                                format!("'{}' membutuhkan 2 atau 3 argumen: (port, nama_penangan, maks_permintaan: opsional)", name),
                            )
                            .with_span(*span));
                        }
                        let port_ty = self.check_expr(&args[0].value)?;
                        self.unify(&port_ty, &Type::U64, args[0].span)?;
                        let handler_ty = self.check_expr(&args[1].value)?;
                        self.unify(&handler_ty, &Type::String, args[1].span)?;
                        if args.len() == 3 {
                            let max_ty = self.check_expr(&args[2].value)?;
                            self.unify(&max_ty, &Type::U64, args[2].span)?;
                        }
                        return Ok(Type::ErrorUnion(Box::new(Type::U64), Box::new(Type::String)));
                    }
                }

                let callee_ty = self.check_expr(callee)?;
                match self.resolve_type(&callee_ty) {
                    Type::Fn(param_types, ret_ty) => {
                        if args.len() != param_types.len() {
                            return Err(Diagnostic::error(format!(
                                "Jumlah argumen ({}) tidak sesuai dengan parameter yang diharapkan ({})",
                                args.len(),
                                param_types.len()
                            ))
                            .with_span(*span));
                        }
                        for (idx, arg) in args.iter().enumerate() {
                            let arg_ty = self.check_expr(&arg.value)?;
                            self.unify(&arg_ty, &param_types[idx], arg.span)?;
                        }
                        Ok(*ret_ty)
                    }
                    other => Err(Diagnostic::error(format!("'{}' bukan fungsi yang dapat dipanggil", other))
                        .with_span(*span)),
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => {
                let rec_ty = self.check_expr(receiver)?;
                let resolved_rec = self.resolve_type(&rec_ty);

                match &resolved_rec {
                    Type::List(elem_ty) => {
                        match method.as_str() {
                            "tambah" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'tambah' membutuhkan tepat 1 argumen")
                                        .with_span(*span));
                                }
                                let arg_ty = self.check_expr(&args[0].value)?;
                                self.unify(&arg_ty, elem_ty, args[0].span)?;
                                Ok(Type::Unit)
                            }
                            "panjang" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'panjang' tidak menerima argumen")
                                        .with_span(*span));
                                }
                                Ok(Type::U64)
                            }
                            "ambil" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'ambil' membutuhkan tepat 1 argumen (indeks)")
                                        .with_span(*span));
                                }
                                let idx_ty = self.check_expr(&args[0].value)?;
                                self.unify(&idx_ty, &Type::U64, args[0].span)?;
                                Ok(Type::ErrorUnion(elem_ty.clone(), Box::new(Type::String)))
                            }
                            other => Err(Diagnostic::error(format!(
                                "Metode '{}' tidak ditemukan pada Daftar<{}>",
                                other, elem_ty
                            ))
                            .with_span(*span)),
                        }
                    }
                    Type::Struct(s_name) if s_name == "BasisData" => {
                        match method.as_str() {
                            "simpan" => {
                                if args.len() != 2 {
                                    return Err(Diagnostic::error("Metode 'simpan' membutuhkan 2 argumen (kunci: Teks, nilai: Teks)").with_span(*span));
                                }
                                let k_ty = self.check_expr(&args[0].value)?;
                                self.unify(&k_ty, &Type::String, args[0].span)?;
                                let v_ty = self.check_expr(&args[1].value)?;
                                self.unify(&v_ty, &Type::String, args[1].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)))
                            }
                            "ambil" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'ambil' membutuhkan 1 argumen (kunci: Teks)").with_span(*span));
                                }
                                let k_ty = self.check_expr(&args[0].value)?;
                                self.unify(&k_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)))
                            }
                            "hapus" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'hapus' membutuhkan 1 argumen (kunci: Teks)").with_span(*span));
                                }
                                let k_ty = self.check_expr(&args[0].value)?;
                                self.unify(&k_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)))
                            }
                            "ada" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'ada' membutuhkan 1 argumen (kunci: Teks)").with_span(*span));
                                }
                                let k_ty = self.check_expr(&args[0].value)?;
                                self.unify(&k_ty, &Type::String, args[0].span)?;
                                Ok(Type::Bool)
                            }
                            "daftar_kunci" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'daftar_kunci' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::ErrorUnion(Box::new(Type::List(Box::new(Type::String))), Box::new(Type::String)))
                            }
                            "padatkan" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'padatkan' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)))
                            }
                            "ambil_awalan" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'ambil_awalan' membutuhkan 1 argumen (awalan: Teks)").with_span(*span));
                                }
                                let p_ty = self.check_expr(&args[0].value)?;
                                self.unify(&p_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::List(Box::new(Type::String))), Box::new(Type::String)))
                            }
                            "mulai_transaksi" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'mulai_transaksi' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)))
                            }
                            "sedang_transaksi" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'sedang_transaksi' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::Bool)
                            }
                            "komit" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'komit' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)))
                            }
                            "batal" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'batal' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::ErrorUnion(Box::new(Type::Bool), Box::new(Type::String)))
                            }
                            "ekspor_json" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'ekspor_json' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)))
                            }
                            "ekspor_json_awalan" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'ekspor_json_awalan' membutuhkan 1 argumen (awalan: Teks)").with_span(*span));
                                }
                                let p_ty = self.check_expr(&args[0].value)?;
                                self.unify(&p_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)))
                            }
                            "impor_json" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'impor_json' membutuhkan 1 argumen (json_teks: Teks)").with_span(*span));
                                }
                                let j_ty = self.check_expr(&args[0].value)?;
                                self.unify(&j_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::U64), Box::new(Type::String)))
                            }
                            other => Err(Diagnostic::error(format!("Metode '{}' tidak ditemukan pada BasisData", other)).with_span(*span)),
                        }
                    }
                    Type::Struct(s_name) if s_name == "PermintaanHttp" => {
                        match method.as_str() {
                            "cocok" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'cocok' membutuhkan 1 argumen (pola: Teks)").with_span(*span));
                                }
                                let p_ty = self.check_expr(&args[0].value)?;
                                self.unify(&p_ty, &Type::String, args[0].span)?;
                                Ok(Type::Bool)
                            }
                            "param" => {
                                if args.len() != 2 {
                                    return Err(Diagnostic::error("Metode 'param' membutuhkan 2 argumen (pola: Teks, nama_param: Teks)").with_span(*span));
                                }
                                let p_ty = self.check_expr(&args[0].value)?;
                                self.unify(&p_ty, &Type::String, args[0].span)?;
                                let n_ty = self.check_expr(&args[1].value)?;
                                self.unify(&n_ty, &Type::String, args[1].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)))
                            }
                            "kueri_param" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'kueri_param' membutuhkan 1 argumen (kunci: Teks)").with_span(*span));
                                }
                                let k_ty = self.check_expr(&args[0].value)?;
                                self.unify(&k_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)))
                            }
                            "ada_kueri" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'ada_kueri' membutuhkan 1 argumen (kunci: Teks)").with_span(*span));
                                }
                                let k_ty = self.check_expr(&args[0].value)?;
                                self.unify(&k_ty, &Type::String, args[0].span)?;
                                Ok(Type::Bool)
                            }
                            "sajikan_statis" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'sajikan_statis' membutuhkan 1 argumen (folder_dasar: Teks)").with_span(*span));
                                }
                                let f_ty = self.check_expr(&args[0].value)?;
                                self.unify(&f_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(
                                    Box::new(Type::Struct("ResponsHttp".to_string())),
                                    Box::new(Type::String),
                                ))
                            }
                            "adalah_websocket" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'adalah_websocket' tidak membutuhkan argumen").with_span(*span));
                                }
                                Ok(Type::Bool)
                            }
                            "kunci_websocket" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'kunci_websocket' tidak membutuhkan argumen").with_span(*span));
                                }
                                Ok(Type::ErrorUnion(Box::new(Type::String), Box::new(Type::String)))
                            }
                            "tingkatkan_websocket" => {
                                if args.len() != 1 {
                                    return Err(Diagnostic::error("Metode 'tingkatkan_websocket' membutuhkan 1 argumen (nama_penangan: Teks)").with_span(*span));
                                }
                                let h_ty = self.check_expr(&args[0].value)?;
                                self.unify(&h_ty, &Type::String, args[0].span)?;
                                Ok(Type::ErrorUnion(
                                    Box::new(Type::Struct("ResponsHttp".to_string())),
                                    Box::new(Type::String),
                                ))
                            }
                            other => Err(Diagnostic::error(format!("Metode '{}' tidak ditemukan pada PermintaanHttp", other)).with_span(*span)),
                        }
                    }
                    Type::Struct(s_name) if s_name == "ResponsHttp" => {
                        match method.as_str() {
                            "tambah_header" => {
                                if args.len() != 2 {
                                    return Err(Diagnostic::error("Metode 'tambah_header' membutuhkan 2 argumen (kunci: Teks, nilai: Teks)").with_span(*span));
                                }
                                let k_ty = self.check_expr(&args[0].value)?;
                                self.unify(&k_ty, &Type::String, args[0].span)?;
                                let v_ty = self.check_expr(&args[1].value)?;
                                self.unify(&v_ty, &Type::String, args[1].span)?;
                                Ok(Type::Struct("ResponsHttp".to_string()))
                            }
                            "cors" => {
                                if !args.is_empty() {
                                    return Err(Diagnostic::error("Metode 'cors' tidak menerima argumen").with_span(*span));
                                }
                                Ok(Type::Struct("ResponsHttp".to_string()))
                            }
                            other => Err(Diagnostic::error(format!("Metode '{}' tidak ditemukan pada ResponsHttp", other)).with_span(*span)),
                        }
                    }
                    Type::Struct(s_name) => {
                        let method_info = self.find_method(s_name, method, *span)?;
                        // Check arguments
                        // First param is `ini`, so call args should match remaining params
                        let expected_params = &method_info.param_types[1..];
                        if args.len() != expected_params.len() {
                            return Err(Diagnostic::error(format!(
                                "Metode '{}.{}' membutuhkan {} argumen, tetapi menerima {}",
                                s_name,
                                method,
                                expected_params.len(),
                                args.len()
                            ))
                            .with_span(*span));
                        }
                        for (i, arg) in args.iter().enumerate() {
                            let arg_ty = self.check_expr(&arg.value)?;
                            self.unify(&arg_ty, &expected_params[i], arg.span)?;
                        }
                        Ok(method_info.ret_type.clone())
                    }
                    other => Err(Diagnostic::error(format!(
                        "Tidak dapat memanggil metode '{}' pada tipe '{}'",
                        method, other
                    ))
                    .with_span(*span)),
                }
            }
            Expr::FieldAccess {
                receiver,
                field,
                span,
            } => {
                let rec_ty = self.check_expr(receiver)?;
                let resolved_rec = self.resolve_type(&rec_ty);

                match &resolved_rec {
                    Type::Struct(s_name) => {
                        if let Some(s_info) = self.structs.get(s_name) {
                            if let Some((_, f_ty)) = s_info.fields.get(field) {
                                Ok(f_ty.clone())
                            } else {
                                Err(Diagnostic::error(format!(
                                    "Field '{}' tidak ada pada struktur '{}'",
                                    field, s_name
                                ))
                                .with_span(*span))
                            }
                        } else {
                            Err(Diagnostic::error(format!("Struktur '{}' tidak ditemukan", s_name))
                                .with_span(*span))
                        }
                    }
                    other => Err(Diagnostic::error(format!(
                        "Akses field '{}' tidak didukung untuk tipe non-struktur '{}'",
                        field, other
                    ))
                    .with_span(*span)),
                }
            }
            Expr::ErrorPropagate { inner, span } => {
                let inner_ty = self.check_expr(inner)?;
                match self.resolve_type(&inner_ty) {
                    Type::ErrorUnion(ok_ty, err_ty) => {
                        // Propagate error if current fn return type expects ErrorUnion with matching err
                        if let Some(curr_ret) = &self.current_fn_ret {
                            match self.resolve_type(curr_ret) {
                                Type::ErrorUnion(_, expected_err) => {
                                    self.unify(&err_ty, &expected_err, *span)?;
                                }
                                other => {
                                    return Err(Diagnostic::error(format!(
                                        "Operator '?' hanya dapat digunakan di dalam fungsi yang mengembalikan Error Union (T ! E), tetapi tipe fungsi saat ini adalah '{}'",
                                        other
                                    ))
                                    .with_span(*span));
                                }
                            }
                        }
                        Ok(*ok_ty)
                    }
                    other => Err(Diagnostic::error(format!(
                        "Operator '?' hanya dapat digunakan pada Error Union (T ! E), tetapi menemukan '{}'",
                        other
                    ))
                    .with_span(*span)),
                }
            }
            Expr::Catch {
                inner,
                err_var,
                handler,
                span,
            } => {
                let inner_ty = self.check_expr(inner)?;
                let resolved = self.resolve_type(&inner_ty);
                let (ok_ty, err_ty) = match resolved {
                    Type::ErrorUnion(ok, err) => (*ok, *err),
                    // If inner already had '?', it might be unwrapped, or error union directly
                    other => {
                        // Allow catch on unwrapped or error union
                        (other.clone(), Type::String)
                    }
                };

                // In catch block, bind err_var to err_ty
                self.enter_scope();
                self.insert_var(err_var, err_ty);
                let handler_ty = self.check_block(handler)?;
                self.exit_scope();

                // If handler diverges (e.g. contains return), the overall expression has type ok_ty
                let diverges = handler.stmts.iter().any(|s| matches!(s, Stmt::Return { .. } | Stmt::Fail { .. }));
                if !diverges && !self.types_match(&handler_ty, &Type::Unit) {
                    self.unify(&handler_ty, &ok_ty, *span)?;
                }

                Ok(ok_ty)
            }
            Expr::Parallel { body, .. } => {
                let body_ty = self.check_block(body)?;
                Ok(Type::Task(Box::new(body_ty)))
            }
            Expr::Await { task, span } => {
                let t_ty = self.check_expr(task)?;
                match self.resolve_type(&t_ty) {
                    Type::Task(inner) => Ok(*inner),
                    other => Err(Diagnostic::error(format!(
                        "'tunggu' membutuhkan tipe 'Tugas', tetapi menemukan '{}'",
                        other
                    ))
                    .with_span(*span)),
                }
            }
            Expr::Comptime { body, .. } => {
                let body_ty = self.check_block(body)?;
                Ok(body_ty)
            }
            Expr::Block(b) => self.check_block(b),
        }
    }

    fn check_struct_instantiation(
        &mut self,
        info: &StructInfo,
        args: &[CallArg],
        span: Span,
    ) -> Result<Type> {
        let is_named = args.iter().any(|a| a.name.is_some());
        if is_named {
            let mut provided_fields = HashMap::new();
            for arg in args {
                let name = match &arg.name {
                    Some(n) => n.clone(),
                    None => {
                        return Err(Diagnostic::error(
                            "Semua argumen harus bernama jika menggunakan instansiasi dengan nama field",
                        )
                        .with_span(arg.span));
                    }
                };
                if provided_fields.contains_key(&name) {
                    return Err(Diagnostic::error(format!(
                        "Field '{}' diberikan lebih dari satu kali",
                        name
                    ))
                    .with_span(arg.span));
                }
                let val_ty = self.check_expr(&arg.value)?;
                provided_fields.insert(name, (val_ty, arg.span));
            }

            for (req_name, (_, req_ty)) in &info.fields {
                if let Some((val_ty, arg_span)) = provided_fields.remove(req_name) {
                    self.unify(&val_ty, req_ty, arg_span)?;
                } else {
                    return Err(Diagnostic::error(format!(
                        "Field '{}' wajib diisi untuk struktur '{}'",
                        req_name, info.name
                    ))
                    .with_span(span));
                }
            }

            if let Some((extra_name, (_, extra_span))) = provided_fields.into_iter().next() {
                return Err(Diagnostic::error(format!(
                    "Field '{}' tidak dikenal pada struktur '{}'",
                    extra_name, info.name
                ))
                .with_span(extra_span));
            }
        } else {
            if args.len() != info.field_order.len() {
                return Err(Diagnostic::error(format!(
                    "Struktur '{}' membutuhkan {} field, tetapi menerima {}",
                    info.name,
                    info.field_order.len(),
                    args.len()
                ))
                .with_span(span));
            }
            for (idx, field_name) in info.field_order.iter().enumerate() {
                let (_, req_ty) = info.fields.get(field_name).unwrap();
                let arg_ty = self.check_expr(&args[idx].value)?;
                self.unify(&arg_ty, req_ty, args[idx].span)?;
            }
        }

        Ok(Type::Struct(info.name.clone()))
    }

    fn find_method(&self, struct_name: &str, method_name: &str, span: Span) -> Result<MethodInfo> {
        if let Some(struct_impls) = self.impls.get(struct_name) {
            for methods in struct_impls.values() {
                if let Some(m) = methods.get(method_name) {
                    return Ok(m.clone());
                }
            }
        }
        Err(Diagnostic::error(format!(
            "Metode '{}' tidak ditemukan pada struktur '{}'",
            method_name, struct_name
        ))
        .with_span(span))
    }

    fn unify_return(&mut self, actual: &Type, expected: &Type, span: Span) -> Result<()> {
        let res_actual = self.resolve_type(actual);
        let res_expected = self.resolve_type(expected);

        // If expected is ErrorUnion(T, E):
        // returning T directly implicitly wraps into Ok(T)
        // returning T ! E also unifies
        if let Type::ErrorUnion(ok_ty, _) = &res_expected {
            if self.unify(&res_actual, ok_ty, span).is_ok() {
                return Ok(());
            }
        }

        self.unify(&res_actual, &res_expected, span)
    }

    pub fn unify(&mut self, t1: &Type, t2: &Type, span: Span) -> Result<()> {
        let t1 = self.resolve_type(t1);
        let t2 = self.resolve_type(t2);

        if t1 == t2 {
            return Ok(());
        }

        match (&t1, &t2) {
            (Type::U64, Type::F64) => Ok(()),
            (Type::Var(id), other) | (other, Type::Var(id)) => {
                self.bind_var(*id, other, span)?;
                Ok(())
            }
            (Type::ErrorUnion(ok1, err1), Type::ErrorUnion(ok2, err2)) => {
                self.unify(ok1, ok2, span)?;
                self.unify(err1, err2, span)?;
                Ok(())
            }
            (Type::Task(inner1), Type::Task(inner2)) => self.unify(inner1, inner2, span),
            (Type::List(inner1), Type::List(inner2)) => self.unify(inner1, inner2, span),
            (Type::Fn(p1, r1), Type::Fn(p2, r2)) => {
                if p1.len() != p2.len() {
                    return Err(Diagnostic::error(format!(
                        "Ketidakcocokan tipe: diharapkan '{}', tetapi menemukan '{}'",
                        t2, t1
                    ))
                    .with_span(span));
                }
                for (a, b) in p1.iter().zip(p2.iter()) {
                    self.unify(a, b, span)?;
                }
                self.unify(r1, r2, span)?;
                Ok(())
            }
            _ => Err(Diagnostic::error(format!(
                "Ketidakcocokan tipe: diharapkan '{}', tetapi menemukan '{}'",
                t2, t1
            ))
            .with_span(span)),
        }
    }

    fn bind_var(&mut self, id: TypeVarId, concrete: &Type, span: Span) -> Result<()> {
        if let Type::Var(other_id) = concrete {
            if *other_id == id {
                return Ok(());
            }
        }
        if self.occurs(id, concrete) {
            return Err(Diagnostic::error("Tipe rekursif tanpa batas (occurs check failed)")
                .with_span(span));
        }
        self.subst.insert(id, concrete.clone());
        Ok(())
    }

    fn occurs(&self, id: TypeVarId, ty: &Type) -> bool {
        match self.resolve_type(ty) {
            Type::Var(other) => other == id,
            Type::ErrorUnion(ok, err) => self.occurs(id, &ok) || self.occurs(id, &err),
            Type::Task(inner) => self.occurs(id, &inner),
            Type::List(inner) => self.occurs(id, &inner),
            Type::Fn(params, ret) => {
                params.iter().any(|p| self.occurs(id, p)) || self.occurs(id, &ret)
            }
            _ => false,
        }
    }

    fn types_match(&self, t1: &Type, t2: &Type) -> bool {
        let r1 = self.resolve_type(t1);
        let r2 = self.resolve_type(t2);
        r1 == r2
    }

    pub fn type_expr_to_type(&self, expr: &TypeExpr) -> Result<Type> {
        match expr {
            TypeExpr::Named(name, span) => match name.as_str() {
                "u64" | "bulat" => Ok(Type::U64),
                "i64" => Ok(Type::I64),
                "f64" | "desimal" => Ok(Type::F64),
                "Teks" | "teks" => Ok(Type::String),
                "Bool" | "bool" => Ok(Type::Bool),
                "Unit" | "unit" => Ok(Type::Unit),
                _ => {
                    if self.structs.contains_key(name) {
                        Ok(Type::Struct(name.clone()))
                    } else if self.traits.contains_key(name) {
                        Ok(Type::Trait(name.clone()))
                    } else {
                        Err(Diagnostic::error(format!("Tipe '{}' tidak dikenal", name))
                            .with_span(*span))
                    }
                }
            },
            TypeExpr::ErrorUnion(ok, err, _) => {
                let ok_ty = self.type_expr_to_type(ok)?;
                let err_ty = self.type_expr_to_type(err)?;
                Ok(Type::ErrorUnion(Box::new(ok_ty), Box::new(err_ty)))
            }
            TypeExpr::Task(inner, _) => {
                let inner_ty = self.type_expr_to_type(inner)?;
                Ok(Type::Task(Box::new(inner_ty)))
            }
            TypeExpr::List(inner, _) => {
                let inner_ty = self.type_expr_to_type(inner)?;
                Ok(Type::List(Box::new(inner_ty)))
            }
            TypeExpr::Unit(_) => Ok(Type::Unit),
            TypeExpr::Inferred(_) => Ok(Type::Var(TypeVarId(99999))),
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn insert_var(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup_var(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(self.resolve_type(ty));
            }
        }
        None
    }
}
