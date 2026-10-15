use crate::ast::*;
use crate::span::SourceMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum DocItemKind {
    Function,
    Struct,
    Trait,
    Impl,
    Test,
}

#[derive(Debug, Clone)]
pub struct DocField {
    pub name: String,
    pub ty: String,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocParam {
    pub name: String,
    pub ty: Option<String>,
    pub is_self: bool,
}

#[derive(Debug, Clone)]
pub struct DocItem {
    pub kind: DocItemKind,
    pub name: String,
    pub signature: String,
    pub doc: Option<String>,
    pub ret_type: Option<String>,
    pub params: Vec<DocParam>,
    pub fields: Vec<DocField>,
    pub methods: Vec<DocItem>,
    pub file_name: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ModuleDoc {
    pub name: String,
    pub file_path: String,
    pub doc: Option<String>,
    pub items: Vec<DocItem>,
}

#[derive(Debug, Clone)]
pub struct ProjectDoc {
    pub name: String,
    pub version: String,
    pub description: String,
    pub modules: Vec<ModuleDoc>,
}

pub fn format_type_expr(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(name, _) => name.clone(),
        TypeExpr::ErrorUnion(ok, err, _) => {
            format!("{} ! {}", format_type_expr(ok), format_type_expr(err))
        }
        TypeExpr::Task(inner, _) => format!("Tugas<{}>", format_type_expr(inner)),
        TypeExpr::List(inner, _) => format!("Daftar<{}>", format_type_expr(inner)),
        TypeExpr::Unit(_) => "()".to_string(),
        TypeExpr::Inferred(_) => "_".to_string(),
    }
}

pub struct DocExtractor;

impl DocExtractor {
    pub fn extract_module(
        src: &str,
        file_path: &Path,
        program: &Program,
        _sm: &SourceMap,
    ) -> ModuleDoc {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "modul.dap".to_string());
        let module_name = file_path
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "modul".to_string());

        let module_doc = Self::extract_module_level_doc(src);
        let mut items = Vec::new();

        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => {
                    let doc = Self::extract_preceding_doc(src, f.span.start);
                    let line = Self::calculate_line(src, f.span.start);
                    let params = f
                        .params
                        .iter()
                        .map(|p| DocParam {
                            name: p.name.clone(),
                            ty: p.ty.as_ref().map(format_type_expr),
                            is_self: p.is_self,
                        })
                        .collect();
                    let ret_type = f.ret_ty.as_ref().map(format_type_expr);

                    let params_str = f
                        .params
                        .iter()
                        .map(|p| {
                            if p.is_self {
                                "ini".to_string()
                            } else if let Some(ty) = &p.ty {
                                format!("{}: {}", p.name, format_type_expr(ty))
                            } else {
                                p.name.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ");

                    let ret_str = if let Some(ret) = &ret_type {
                        format!(" -> {}", ret)
                    } else {
                        String::new()
                    };

                    let comptime_str = if f.is_comptime { "comptime " } else { "" };
                    let signature = format!("{}{}fungsi {}({}){}", comptime_str, "", f.name, params_str, ret_str);

                    items.push(DocItem {
                        kind: DocItemKind::Function,
                        name: f.name.clone(),
                        signature,
                        doc,
                        ret_type,
                        params,
                        fields: Vec::new(),
                        methods: Vec::new(),
                        file_name: file_name.clone(),
                        line,
                    });
                }
                Decl::Struct(s) => {
                    let doc = Self::extract_preceding_doc(src, s.span.start);
                    let line = Self::calculate_line(src, s.span.start);
                    let fields = s
                        .fields
                        .iter()
                        .map(|f| DocField {
                            name: f.name.clone(),
                            ty: format_type_expr(&f.ty),
                            doc: None,
                        })
                        .collect();

                    let fields_str = s
                        .fields
                        .iter()
                        .map(|f| format!("    {}: {}", f.name, format_type_expr(&f.ty)))
                        .collect::<Vec<_>>()
                        .join(",\n");

                    let signature = if s.fields.is_empty() {
                        format!("struktur {} {{}}", s.name)
                    } else {
                        format!("struktur {} {{\n{}\n}}", s.name, fields_str)
                    };

                    items.push(DocItem {
                        kind: DocItemKind::Struct,
                        name: s.name.clone(),
                        signature,
                        doc,
                        ret_type: None,
                        params: Vec::new(),
                        fields,
                        methods: Vec::new(),
                        file_name: file_name.clone(),
                        line,
                    });
                }
                Decl::Trait(t) => {
                    let doc = Self::extract_preceding_doc(src, t.span.start);
                    let line = Self::calculate_line(src, t.span.start);
                    let methods = t
                        .methods
                        .iter()
                        .map(|m| {
                            let m_params = m
                                .params
                                .iter()
                                .map(|p| {
                                    if p.is_self {
                                        "ini".to_string()
                                    } else if let Some(ty) = &p.ty {
                                        format!("{}: {}", p.name, format_type_expr(ty))
                                    } else {
                                        p.name.clone()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            let m_ret = if let Some(r) = &m.ret_ty {
                                format!(" -> {}", format_type_expr(r))
                            } else {
                                String::new()
                            };
                            let m_sig = format!("fungsi {}({}){}", m.name, m_params, m_ret);

                            DocItem {
                                kind: DocItemKind::Function,
                                name: m.name.clone(),
                                signature: m_sig,
                                doc: None,
                                ret_type: m.ret_ty.as_ref().map(format_type_expr),
                                params: m
                                    .params
                                    .iter()
                                    .map(|p| DocParam {
                                        name: p.name.clone(),
                                        ty: p.ty.as_ref().map(format_type_expr),
                                        is_self: p.is_self,
                                    })
                                    .collect(),
                                fields: Vec::new(),
                                methods: Vec::new(),
                                file_name: file_name.clone(),
                                line,
                            }
                        })
                        .collect();

                    let signature = format!("sifat {} {{ ... }}", t.name);

                    items.push(DocItem {
                        kind: DocItemKind::Trait,
                        name: t.name.clone(),
                        signature,
                        doc,
                        ret_type: None,
                        params: Vec::new(),
                        fields: Vec::new(),
                        methods,
                        file_name: file_name.clone(),
                        line,
                    });
                }
                Decl::Impl(i) => {
                    let doc = Self::extract_preceding_doc(src, i.span.start);
                    let line = Self::calculate_line(src, i.span.start);
                    let methods = i
                        .methods
                        .iter()
                        .map(|m| {
                            let m_params = m
                                .params
                                .iter()
                                .map(|p| {
                                    if p.is_self {
                                        "ini".to_string()
                                    } else if let Some(ty) = &p.ty {
                                        format!("{}: {}", p.name, format_type_expr(ty))
                                    } else {
                                        p.name.clone()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            let m_ret = if let Some(r) = &m.ret_ty {
                                format!(" -> {}", format_type_expr(r))
                            } else {
                                String::new()
                            };
                            let m_sig = format!("fungsi {}({}){}", m.name, m_params, m_ret);
                            let m_doc = Self::extract_preceding_doc(src, m.span.start);

                            DocItem {
                                kind: DocItemKind::Function,
                                name: m.name.clone(),
                                signature: m_sig,
                                doc: m_doc,
                                ret_type: m.ret_ty.as_ref().map(format_type_expr),
                                params: m
                                    .params
                                    .iter()
                                    .map(|p| DocParam {
                                        name: p.name.clone(),
                                        ty: p.ty.as_ref().map(format_type_expr),
                                        is_self: p.is_self,
                                    })
                                    .collect(),
                                fields: Vec::new(),
                                methods: Vec::new(),
                                file_name: file_name.clone(),
                                line,
                            }
                        })
                        .collect();

                    let signature = if let Some(tr) = &i.trait_name {
                        format!("terapkan {} untuk {}", tr, i.target_struct)
                    } else {
                        format!("terapkan untuk {}", i.target_struct)
                    };

                    items.push(DocItem {
                        kind: DocItemKind::Impl,
                        name: format!("terapkan {}", i.target_struct),
                        signature,
                        doc,
                        ret_type: None,
                        params: Vec::new(),
                        fields: Vec::new(),
                        methods,
                        file_name: file_name.clone(),
                        line,
                    });
                }
                Decl::Test(t) => {
                    let doc = Self::extract_preceding_doc(src, t.span.start);
                    let line = Self::calculate_line(src, t.span.start);
                    items.push(DocItem {
                        kind: DocItemKind::Test,
                        name: t.name.clone(),
                        signature: format!("uji \"{}\" {{ ... }}", t.name),
                        doc,
                        ret_type: None,
                        params: Vec::new(),
                        fields: Vec::new(),
                        methods: Vec::new(),
                        file_name: file_name.clone(),
                        line,
                    });
                }
                Decl::Import(_) => {}
            }
        }

        ModuleDoc {
            name: module_name,
            file_path: file_path.to_string_lossy().to_string(),
            doc: module_doc,
            items,
        }
    }

    pub fn extract_module_level_doc(src: &str) -> Option<String> {
        let mut lines = Vec::new();
        for line in src.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//!") {
                let content = if trimmed.starts_with("//! ") {
                    &trimmed[4..]
                } else {
                    &trimmed[3..]
                };
                lines.push(content.to_string());
            } else if trimmed.is_empty() {
                if !lines.is_empty() {
                    lines.push("".to_string());
                }
            } else {
                break;
            }
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n").trim().to_string())
        }
    }

    pub fn extract_preceding_doc(src: &str, byte_offset: usize) -> Option<String> {
        if byte_offset == 0 || byte_offset > src.len() {
            return None;
        }

        let prefix = &src[..byte_offset];
        let mut doc_lines = Vec::new();
        let mut found_doc = false;

        for line in prefix.lines().rev() {
            let trimmed = line.trim();
            if trimmed.starts_with("///") {
                found_doc = true;
                let content = if trimmed.starts_with("/// ") {
                    &trimmed[4..]
                } else {
                    &trimmed[3..]
                };
                doc_lines.push(content.to_string());
            } else if trimmed.is_empty() {
                if found_doc {
                    // Empty line inside doc block
                    doc_lines.push("".to_string());
                } else {
                    // Empty lines before declaration, keep looking upward
                    continue;
                }
            } else {
                // Non-doc line encountered, stop scanning
                break;
            }
        }

        if doc_lines.is_empty() {
            None
        } else {
            doc_lines.reverse();
            Some(doc_lines.join("\n").trim().to_string())
        }
    }

    fn calculate_line(src: &str, byte_offset: usize) -> usize {
        let clamped = byte_offset.min(src.len());
        src[..clamped].chars().filter(|&c| c == '\n').count() + 1
    }
}

pub struct DocGenerator<'a> {
    pub project: &'a ProjectDoc,
}

impl<'a> DocGenerator<'a> {
    pub fn new(project: &'a ProjectDoc) -> Self {
        DocGenerator { project }
    }

    pub fn generate_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# 📚 Dokumentasi Proyek: {}\n\n", self.project.name));
        md.push_str(&format!("**Versi:** `{}`  \n", self.project.version));
        if !self.project.description.is_empty() {
            md.push_str(&format!("**Deskripsi:** {}\n\n", self.project.description));
        }
        md.push_str("---\n\n");

        md.push_str("## 📑 Daftar Isi\n\n");
        for module in &self.project.modules {
            md.push_str(&format!("- [Modul: `{}`](#modul-{}) \n", module.name, module.name.to_lowercase()));
            for item in &module.items {
                let kind_label = match item.kind {
                    DocItemKind::Function => "fungsi",
                    DocItemKind::Struct => "struktur",
                    DocItemKind::Trait => "sifat",
                    DocItemKind::Impl => "implementasi",
                    DocItemKind::Test => "uji",
                };
                md.push_str(&format!("  - [{}: `{}`](#{}-{}) \n", kind_label, item.name, kind_label, item.name.to_lowercase().replace(' ', "-")));
            }
        }
        md.push_str("\n---\n\n");

        for module in &self.project.modules {
            md.push_str(&format!("## 📦 Modul `{}`\n\n", module.name));
            md.push_str(&format!("**Berkas:** `{}`  \n\n", module.file_path));
            if let Some(doc) = &module.doc {
                md.push_str(&format!("{}\n\n", doc));
            }

            // Functions
            let funcs: Vec<&DocItem> = module.items.iter().filter(|i| i.kind == DocItemKind::Function).collect();
            if !funcs.is_empty() {
                md.push_str("### ⚡ Fungsi\n\n");
                for f in funcs {
                    md.push_str(&format!("#### `fungsi {}`\n\n", f.name));
                    md.push_str("```dap\n");
                    md.push_str(&f.signature);
                    md.push_str("\n```\n\n");
                    if let Some(doc) = &f.doc {
                        md.push_str(&format!("{}\n\n", doc));
                    }
                    if !f.params.is_empty() {
                        md.push_str("**Parameter:**\n");
                        for p in &f.params {
                            if p.is_self {
                                md.push_str("- `ini`: Referensi penerima struktur\n");
                            } else if let Some(ty) = &p.ty {
                                md.push_str(&format!("- `{}`: `{}`\n", p.name, ty));
                            } else {
                                md.push_str(&format!("- `{}`\n", p.name));
                            }
                        }
                        md.push('\n');
                    }
                    if let Some(ret) = &f.ret_type {
                        md.push_str(&format!("**Nilai Kembalian:** `{}`\n\n", ret));
                    }
                }
            }

            // Structs
            let structs: Vec<&DocItem> = module.items.iter().filter(|i| i.kind == DocItemKind::Struct).collect();
            if !structs.is_empty() {
                md.push_str("### 🏗️ Struktur Data\n\n");
                for s in structs {
                    md.push_str(&format!("#### `struktur {}`\n\n", s.name));
                    md.push_str("```dap\n");
                    md.push_str(&s.signature);
                    md.push_str("\n```\n\n");
                    if let Some(doc) = &s.doc {
                        md.push_str(&format!("{}\n\n", doc));
                    }
                    if !s.fields.is_empty() {
                        md.push_str("| Bidang (*Field*) | Tipe Data |\n");
                        md.push_str("| :--- | :--- |\n");
                        for f in &s.fields {
                            md.push_str(&format!("| `{}` | `{}` |\n", f.name, f.ty));
                        }
                        md.push('\n');
                    }
                }
            }

            // Traits
            let traits: Vec<&DocItem> = module.items.iter().filter(|i| i.kind == DocItemKind::Trait).collect();
            if !traits.is_empty() {
                md.push_str("### 📐 Sifat (Trait)\n\n");
                for t in traits {
                    md.push_str(&format!("#### `sifat {}`\n\n", t.name));
                    if let Some(doc) = &t.doc {
                        md.push_str(&format!("{}\n\n", doc));
                    }
                    if !t.methods.is_empty() {
                        md.push_str("**Metode Kontrak:**\n");
                        for m in &t.methods {
                            md.push_str(&format!("- `{}`\n", m.signature));
                        }
                        md.push('\n');
                    }
                }
            }

            // Impls
            let impls: Vec<&DocItem> = module.items.iter().filter(|i| i.kind == DocItemKind::Impl).collect();
            if !impls.is_empty() {
                md.push_str("### 🧩 Implementasi Sifat / Metode\n\n");
                for imp in impls {
                    md.push_str(&format!("#### `{}`\n\n", imp.signature));
                    if let Some(doc) = &imp.doc {
                        md.push_str(&format!("{}\n\n", doc));
                    }
                    for m in &imp.methods {
                        md.push_str(&format!("- `{}`\n", m.signature));
                        if let Some(mdoc) = &m.doc {
                            md.push_str(&format!("  *{}*\n", mdoc.replace('\n', " ")));
                        }
                    }
                    md.push('\n');
                }
            }

            // Tests
            let tests: Vec<&DocItem> = module.items.iter().filter(|i| i.kind == DocItemKind::Test).collect();
            if !tests.is_empty() {
                md.push_str("### 🧪 Pengujian Unit (Test Cases)\n\n");
                for t in tests {
                    md.push_str(&format!("- `uji \"{}\"` (baris {})\n", t.name, t.line));
                    if let Some(tdoc) = &t.doc {
                        md.push_str(&format!("  *{}*\n", tdoc));
                    }
                }
                md.push('\n');
            }

            md.push_str("---\n\n");
        }

        md.push_str("Dokumentasi ini dibuat secara otomatis oleh kompilator **`dapcodec doc`**.\n");
        md
    }

    pub fn generate_html(&self) -> String {
        let mut html = String::new();
        let total_modules = self.project.modules.len();
        let mut total_funcs = 0;
        let mut total_structs = 0;
        let mut total_traits = 0;

        for m in &self.project.modules {
            for item in &m.items {
                match item.kind {
                    DocItemKind::Function => total_funcs += 1,
                    DocItemKind::Struct => total_structs += 1,
                    DocItemKind::Trait => total_traits += 1,
                    _ => {}
                }
            }
        }

        html.push_str(r##"<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>"##);
        html.push_str(&format!("{} - Dokumentasi Resmi DapCode", self.project.name));
        html.push_str(r##"</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600;700&family=Outfit:wght@300;400;500;600;700&display=swap" rel="stylesheet">
    <style>
        :root {
            --bg-base: #0b0f19;
            --bg-surface: #111827;
            --bg-surface-elevated: #1f2937;
            --border-color: #374151;
            --primary: #38bdf8;
            --primary-glow: rgba(56, 189, 248, 0.2);
            --accent: #818cf8;
            --success: #34d399;
            --warning: #fbbf24;
            --text-main: #f9fafb;
            --text-muted: #9ca3af;
            --code-bg: #030712;
            --sidebar-width: 320px;
        }
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: 'Outfit', sans-serif;
            background: var(--bg-base);
            color: var(--text-main);
            display: flex;
            min-height: 100vh;
            overflow-x: hidden;
        }
        /* Sidebar Navigation */
        #sidebar {
            width: var(--sidebar-width);
            background: var(--bg-surface);
            border-right: 1px solid var(--border-color);
            position: fixed;
            top: 0;
            bottom: 0;
            left: 0;
            display: flex;
            flex-direction: column;
            z-index: 100;
        }
        .sidebar-header {
            padding: 24px 20px 16px;
            border-bottom: 1px solid var(--border-color);
        }
        .brand {
            display: flex;
            align-items: center;
            gap: 10px;
            text-decoration: none;
        }
        .brand-logo {
            width: 36px;
            height: 36px;
            background: linear-gradient(135deg, var(--primary), var(--accent));
            border-radius: 8px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-weight: 700;
            color: #0b0f19;
            font-size: 1.1rem;
            box-shadow: 0 0 15px var(--primary-glow);
        }
        .brand-title {
            font-size: 1.25rem;
            font-weight: 700;
            color: var(--text-main);
        }
        .brand-version {
            font-size: 0.75rem;
            background: rgba(56, 189, 248, 0.15);
            color: var(--primary);
            padding: 2px 8px;
            border-radius: 12px;
            border: 1px solid rgba(56, 189, 248, 0.3);
            margin-left: 6px;
        }
        .search-box {
            padding: 16px 20px 8px;
        }
        .search-input {
            width: 100%;
            background: var(--code-bg);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 10px 14px;
            color: var(--text-main);
            font-family: inherit;
            font-size: 0.9rem;
            outline: none;
            transition: border-color 0.2s, box-shadow 0.2s;
        }
        .search-input:focus {
            border-color: var(--primary);
            box-shadow: 0 0 0 3px var(--primary-glow);
        }
        .nav-tree {
            flex: 1;
            overflow-y: auto;
            padding: 12px 14px;
        }
        .nav-group-title {
            font-size: 0.75rem;
            text-transform: uppercase;
            letter-spacing: 0.08em;
            color: var(--text-muted);
            margin: 16px 8px 8px;
            font-weight: 600;
        }
        .nav-item {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 8px 12px;
            border-radius: 6px;
            color: var(--text-muted);
            text-decoration: none;
            font-size: 0.9rem;
            transition: all 0.15s ease;
            margin-bottom: 2px;
        }
        .nav-item:hover {
            color: var(--primary);
            background: rgba(56, 189, 248, 0.08);
            transform: translateX(3px);
        }
        .item-tag {
            font-size: 0.7rem;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: 'JetBrains Mono', monospace;
            background: var(--bg-surface-elevated);
        }
        .tag-fn { color: var(--primary); }
        .tag-struct { color: var(--accent); }
        .tag-trait { color: var(--warning); }
        /* Main Content */
        #content {
            margin-left: var(--sidebar-width);
            flex: 1;
            padding: 48px 64px;
            max-width: 1100px;
        }
        .hero {
            background: linear-gradient(135deg, rgba(56, 189, 248, 0.05), rgba(129, 140, 248, 0.08));
            border: 1px solid var(--border-color);
            border-radius: 16px;
            padding: 36px 40px;
            margin-bottom: 48px;
            position: relative;
            overflow: hidden;
        }
        .hero h1 {
            font-size: 2.5rem;
            font-weight: 700;
            margin-bottom: 12px;
            background: linear-gradient(90deg, #f9fafb, #93c5fd);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        .hero p {
            color: var(--text-muted);
            font-size: 1.1rem;
            line-height: 1.6;
            margin-bottom: 20px;
        }
        .stats-bar {
            display: flex;
            gap: 24px;
            flex-wrap: wrap;
        }
        .stat-badge {
            background: var(--bg-surface);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 8px 16px;
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 0.9rem;
        }
        .stat-num {
            font-weight: 700;
            color: var(--primary);
        }
        .section-header {
            display: flex;
            align-items: center;
            gap: 12px;
            margin: 48px 0 24px;
            padding-bottom: 12px;
            border-bottom: 1px solid var(--border-color);
        }
        .section-header h2 {
            font-size: 1.75rem;
            font-weight: 700;
        }
        /* Item Card */
        .item-card {
            background: var(--bg-surface);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 24px;
            margin-bottom: 24px;
            transition: border-color 0.2s, box-shadow 0.2s;
        }
        .item-card:hover {
            border-color: rgba(56, 189, 248, 0.4);
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
        }
        .item-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-bottom: 16px;
        }
        .item-title {
            display: flex;
            align-items: center;
            gap: 12px;
            font-size: 1.35rem;
            font-weight: 600;
        }
        .code-block {
            background: var(--code-bg);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 16px 18px;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.95rem;
            color: #7dd3fc;
            overflow-x: auto;
            position: relative;
            margin-bottom: 16px;
        }
        .copy-btn {
            position: absolute;
            top: 10px;
            right: 10px;
            background: var(--bg-surface-elevated);
            border: 1px solid var(--border-color);
            color: var(--text-muted);
            border-radius: 6px;
            padding: 4px 8px;
            font-size: 0.75rem;
            cursor: pointer;
            transition: all 0.15s;
        }
        .copy-btn:hover {
            color: var(--text-main);
            border-color: var(--primary);
        }
        .item-doc {
            color: #d1d5db;
            line-height: 1.65;
            margin-bottom: 16px;
            font-size: 0.98rem;
            white-space: pre-wrap;
        }
        .params-table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 12px;
            font-size: 0.9rem;
        }
        .params-table th, .params-table td {
            text-align: left;
            padding: 8px 12px;
            border-bottom: 1px solid var(--border-color);
        }
        .params-table th {
            color: var(--text-muted);
            font-weight: 600;
            font-size: 0.8rem;
            text-transform: uppercase;
        }
        .param-name {
            font-family: 'JetBrains Mono', monospace;
            color: var(--primary);
        }
        .param-type {
            font-family: 'JetBrains Mono', monospace;
            color: var(--accent);
        }
        .return-badge {
            display: inline-flex;
            align-items: center;
            gap: 6px;
            background: rgba(52, 211, 153, 0.1);
            color: var(--success);
            border: 1px solid rgba(52, 211, 153, 0.25);
            border-radius: 6px;
            padding: 4px 10px;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.85rem;
            margin-top: 8px;
        }
        footer {
            margin-top: 64px;
            padding-top: 24px;
            border-top: 1px solid var(--border-color);
            color: var(--text-muted);
            font-size: 0.85rem;
            text-align: center;
        }
    </style>
</head>
<body>
    <aside id="sidebar">
        <div class="sidebar-header">
            <a href='#' class="brand">
                <div class="brand-logo">D</div>
                <div>
                    <span class="brand-title">"##);
        html.push_str(&self.project.name);
        html.push_str(r##"</span>
                    <span class="brand-version">v"##);
        html.push_str(&self.project.version);
        html.push_str(r##"</span>
                </div>
            </a>
        </div>
        <div class="search-box">
            <input type="text" id="search-input" class="search-input" placeholder="Cari fungsi, tipe..." oninput="filterDocs()">
        </div>
        <nav class="nav-tree">"##);

        for module in &self.project.modules {
            html.push_str(&format!(
                r##"<div class="nav-group-title">Modul: {}</div>"##,
                module.name
            ));
            for item in &module.items {
                let tag_class = match item.kind {
                    DocItemKind::Function => "tag-fn",
                    DocItemKind::Struct => "tag-struct",
                    DocItemKind::Trait => "tag-trait",
                    _ => "",
                };
                let tag_label = match item.kind {
                    DocItemKind::Function => "fn",
                    DocItemKind::Struct => "struct",
                    DocItemKind::Trait => "trait",
                    DocItemKind::Impl => "impl",
                    DocItemKind::Test => "uji",
                };
                let anchor_id = format!("{}-{}", tag_label, item.name.to_lowercase().replace(' ', "-"));
                html.push_str(&format!(
                    r##"<a href="#{}" class="nav-item doc-entry" data-name="{}">
                        <span>{}</span>
                        <span class="item-tag {}">{}</span>
                    </a>"##,
                    anchor_id,
                    item.name.to_lowercase(),
                    item.name,
                    tag_class,
                    tag_label
                ));
            }
        }

        html.push_str(r##"        </nav>
    </aside>

    <main id="content">
        <div class="hero">
            <h1>"##);
        html.push_str(&self.project.name);
        html.push_str(r##"</h1>
            <p>"##);
        if !self.project.description.is_empty() {
            html.push_str(&self.project.description);
        } else {
            html.push_str("Dokumentasi teknis resmi modul dan pustaka DapCode.");
        }
        html.push_str(r##"</p>
            <div class="stats-bar">
                <div class="stat-badge"><span class="stat-num">"##);
        html.push_str(&total_modules.to_string());
        html.push_str(r##"</span> Modul</div>
                <div class="stat-badge"><span class="stat-num">"##);
        html.push_str(&total_funcs.to_string());
        html.push_str(r##"</span> Fungsi</div>
                <div class="stat-badge"><span class="stat-num">"##);
        html.push_str(&total_structs.to_string());
        html.push_str(r##"</span> Struktur Data</div>
                <div class="stat-badge"><span class="stat-num">"##);
        html.push_str(&total_traits.to_string());
        html.push_str(r##"</span> Sifat / Trait</div>
            </div>
        </div>"##);

        for module in &self.project.modules {
            html.push_str(&format!(
                r##"<section class="module-section">
                <div class="section-header">
                    <h2>📦 Modul {}</h2>
                </div>"##,
                module.name
            ));

            if let Some(doc) = &module.doc {
                html.push_str(&format!(r##"<p class="item-doc">{}</p>"##, doc));
            }

            for item in &module.items {
                let tag_label = match item.kind {
                    DocItemKind::Function => "fn",
                    DocItemKind::Struct => "struct",
                    DocItemKind::Trait => "trait",
                    DocItemKind::Impl => "impl",
                    DocItemKind::Test => "uji",
                };
                let anchor_id = format!("{}-{}", tag_label, item.name.to_lowercase().replace(' ', "-"));

                html.push_str(&format!(
                    r##"<div id="{}" class="item-card doc-card" data-name="{}">
                        <div class="item-header">
                            <div class="item-title">
                                <span>{}</span>
                            </div>
                            <span class="item-tag tag-{}">{}</span>
                        </div>
                        <div class="code-block">
                            <button class="copy-btn" onclick="copySignature(this)">Salin</button>
                            <code>{}</code>
                        </div>"##,
                    anchor_id,
                    item.name.to_lowercase(),
                    item.name,
                    tag_label,
                    tag_label.to_uppercase(),
                    Self::escape_html(&item.signature)
                ));

                if let Some(doc) = &item.doc {
                    html.push_str(&format!(r##"<div class="item-doc">{}</div>"##, Self::escape_html(doc)));
                }

                if !item.fields.is_empty() {
                    html.push_str(r##"<table class="params-table">
                        <thead>
                            <tr><th>Bidang (Field)</th><th>Tipe Data</th></tr>
                        </thead>
                        <tbody>"##);
                    for f in &item.fields {
                        html.push_str(&format!(
                            r##"<tr><td class="param-name">{}</td><td class="param-type">{}</td></tr>"##,
                            f.name, f.ty
                        ));
                    }
                    html.push_str("</tbody></table>");
                }

                if !item.params.is_empty() {
                    html.push_str(r##"<table class="params-table">
                        <thead>
                            <tr><th>Parameter</th><th>Tipe Data</th></tr>
                        </thead>
                        <tbody>"##);
                    for p in &item.params {
                        let ty_str = if p.is_self {
                            "ini (penerima metode)".to_string()
                        } else {
                            p.ty.clone().unwrap_or_else(|| "auto".to_string())
                        };
                        html.push_str(&format!(
                            r##"<tr><td class="param-name">{}</td><td class="param-type">{}</td></tr>"##,
                            p.name, ty_str
                        ));
                    }
                    html.push_str("</tbody></table>");
                }

                if let Some(ret) = &item.ret_type {
                    html.push_str(&format!(
                        r##"<div class="return-badge">↳ Kembalian: {}</div>"##,
                        ret
                    ));
                }

                html.push_str("</div>");
            }

            html.push_str("</section>");
        }

        html.push_str(r##"        <footer>
            Dokumentasi dihasilkan secara otomatis oleh <strong>dapcodec doc</strong> &bull; Kompilator Resmi DapCode
        </footer>
    </main>

    <script>
        function filterDocs() {
            const query = document.getElementById('search-input').value.toLowerCase().trim();
            const cards = document.querySelectorAll('.doc-card');
            const navEntries = document.querySelectorAll('.doc-entry');

            cards.forEach(card => {
                const name = card.getAttribute('data-name');
                if (!query || name.includes(query)) {
                    card.style.display = 'block';
                } else {
                    card.style.display = 'none';
                }
            });

            navEntries.forEach(entry => {
                const name = entry.getAttribute('data-name');
                if (!query || name.includes(query)) {
                    entry.style.display = 'flex';
                } else {
                    entry.style.display = 'none';
                }
            });
        }

        function copySignature(btn) {
            const code = btn.nextElementSibling.innerText;
            navigator.clipboard.writeText(code).then(() => {
                const orig = btn.innerText;
                btn.innerText = 'Disalin!';
                btn.style.borderColor = 'var(--primary)';
                setTimeout(() => {
                    btn.innerText = orig;
                    btn.style.borderColor = '';
                }, 1800);
            });
        }
    </script>
</body>
</html>"##);

        html
    }

    fn escape_html(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }
}
