//! 轻量语法高亮：手写词法扫描，零依赖、零外部语法文件。
//! 输出按行切分的 span 列表，渲染时再拼成 LayoutJob（可缓存 galley）。

use egui::{Color32, FontFamily, FontId, TextFormat};
use egui::text::TextWrapping;

pub const P: u8 = 0; // plain
pub const K: u8 = 1; // keyword
pub const S: u8 = 2; // string
pub const N: u8 = 3; // number
pub const C: u8 = 4; // comment
pub const Y: u8 = 5; // type
pub const F: u8 = 6; // function
pub const A: u8 = 7; // attribute / property
pub const V: u8 = 8; // variable
pub const G: u8 = 9; // tag
pub const I: u8 = 10; // diff added
pub const D: u8 = 11; // diff removed
pub const H: u8 = 12; // heading

#[derive(Clone, Default)]
pub struct Line {
    pub spans: Vec<(u8, String)>,
}

pub fn color(t: u8) -> Color32 {
    crate::theme::tok(t)
}

fn bg(t: u8) -> Color32 {
    crate::theme::tok_bg(t)
}

/// 把可能含换行的 span 序列切成按行的结构
fn split_lines(spans: Vec<(u8, String)>) -> Vec<Line> {
    let mut out: Vec<Line> = vec![Line::default()];
    for (t, s) in spans {
        let mut rest = s.as_str();
        loop {
            match rest.find('\n') {
                Some(p) => {
                    if p > 0 {
                        let cur = out.last_mut().unwrap();
                        let mut text = String::with_capacity(p);
                        text.push_str(&rest[..p]);
                        if let Some(last) = cur.spans.last_mut() {
                            if last.0 == t {
                                last.1.push_str(&text);
                                rest = &rest[p + 1..];
                                out.push(Line::default());
                                if rest.is_empty() {
                                    break;
                                }
                                continue;
                            }
                        }
                        cur.spans.push((t, text));
                    }
                    out.push(Line::default());
                    rest = &rest[p + 1..];
                    if rest.is_empty() {
                        break;
                    }
                }
                None => {
                    if !rest.is_empty() {
                        let cur = out.last_mut().unwrap();
                        if let Some(last) = cur.spans.last_mut() {
                            if last.0 == t {
                                last.1.push_str(rest);
                                break;
                            }
                        }
                        cur.spans.push((t, rest.to_string()));
                    }
                    break;
                }
            }
        }
    }
    out
}

fn at(c: &[char], i: usize, pat: &str) -> bool {
    let mut k = i;
    for ch in pat.chars() {
        if k >= c.len() || c[k] != ch {
            return false;
        }
        k += 1;
    }
    true
}

fn slice(c: &[char], a: usize, b: usize) -> String {
    c[a..b].iter().collect()
}

fn ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '\\'
}

fn ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '\\'
}

// ---------------------------------------------------------------- 语言规格

struct Spec {
    line_com: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    kw: &'static [&'static str],
    ty: &'static [&'static str],
    dollar: bool, // PHP 风格 $var
    at_var: bool, // Ruby 风格 @var
    case_ins: bool,
}

const PHP_KW: &[&str] = &[
    "abstract", "and", "array", "as", "break", "callable", "case", "catch", "class", "clone", "const",
    "continue", "declare", "default", "die", "do", "echo", "else", "elseif", "empty", "enum", "enddeclare",
    "endfor", "endforeach", "endif", "endswitch", "endwhile", "eval", "exit", "extends", "final", "finally",
    "fn", "for", "foreach", "function", "global", "goto", "if", "implements", "include", "include_once",
    "instanceof", "insteadof", "interface", "isset", "list", "match", "namespace", "new", "or", "print",
    "private", "protected", "public", "readonly", "require", "require_once", "return", "static", "switch",
    "throw", "trait", "try", "unset", "use", "var", "while", "xor", "yield", "true", "false", "null",
    "self", "parent", "this", "int", "float", "string", "bool", "void", "iterable", "mixed", "object",
];

const JS_KW: &[&str] = &[
    "async", "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default",
    "delete", "do", "else", "export", "extends", "finally", "for", "function", "if", "import", "in",
    "instanceof", "let", "new", "of", "return", "static", "super", "switch", "this", "throw", "try",
    "typeof", "var", "void", "while", "with", "yield", "true", "false", "null", "undefined", "from", "as",
];

const TS_KW: &[&str] = &[
    "abstract", "any", "as", "async", "await", "boolean", "break", "case", "catch", "class", "const",
    "constructor", "continue", "declare", "default", "delete", "do", "else", "enum", "export", "extends",
    "false", "finally", "for", "from", "function", "get", "if", "implements", "import", "in", "infer",
    "instanceof", "interface", "is", "keyof", "let", "namespace", "never", "new", "null", "number",
    "object", "of", "private", "protected", "public", "readonly", "return", "set", "static", "string",
    "super", "switch", "this", "throw", "true", "try", "type", "typeof", "undefined", "unique", "unknown",
    "var", "void", "while", "yield",
];

const PY_KW: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
    "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not",
    "or", "pass", "raise", "return", "try", "while", "with", "yield", "match", "case", "True", "False",
    "None", "self", "cls",
];

const RS_KW: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "yield",
];

const GO_KW: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for",
    "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select",
    "struct", "switch", "type", "var", "nil", "true", "false",
];

const JAVA_KW: &[&str] = &[
    "abstract", "assert", "break", "case", "catch", "class", "const", "continue", "default", "do",
    "else", "enum", "extends", "final", "finally", "for", "goto", "if", "implements", "import",
    "instanceof", "interface", "native", "new", "package", "private", "protected", "public", "record",
    "return", "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws",
    "transient", "try", "var", "volatile", "while", "true", "false", "null",
];

const C_KW: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else", "enum",
    "extern", "float", "for", "goto", "if", "inline", "int", "long", "register", "return", "short",
    "signed", "sizeof", "static", "struct", "switch", "typedef", "union", "unsigned", "void", "volatile",
    "while", "class", "namespace", "template", "typename", "public", "private", "protected", "virtual",
    "override", "new", "delete", "nullptr", "true", "false", "using", "constexpr", "auto", "this",
];

const CS_KW: &[&str] = &[
    "abstract", "as", "base", "break", "case", "catch", "checked", "class", "const", "continue",
    "default", "delegate", "do", "else", "enum", "event", "explicit", "extern", "finally", "fixed",
    "for", "foreach", "goto", "if", "implicit", "in", "interface", "internal", "is", "lock", "namespace",
    "new", "null", "operator", "out", "override", "params", "private", "protected", "public", "readonly",
    "ref", "return", "sealed", "sizeof", "stackalloc", "static", "struct", "switch", "this", "throw",
    "true", "false", "try", "typeof", "unchecked", "unsafe", "using", "var", "virtual", "void",
    "volatile", "while", "async", "await", "string", "int", "bool", "object",
];

const KT_KW: &[&str] = &[
    "as", "break", "class", "continue", "do", "else", "false", "for", "fun", "if", "in", "interface",
    "is", "null", "object", "package", "return", "super", "this", "throw", "true", "try", "typealias",
    "typeof", "val", "var", "when", "while", "by", "catch", "constructor", "dynamic", "finally", "get",
    "import", "init", "param", "set", "where", "data", "private", "public", "internal", "override",
];

const SWIFT_KW: &[&str] = &[
    "associatedtype", "class", "deinit", "enum", "extension", "fileprivate", "func", "import", "init",
    "inout", "internal", "let", "open", "operator", "private", "protocol", "public", "rethrows",
    "static", "struct", "subscript", "typealias", "var", "break", "case", "continue", "default", "defer",
    "do", "else", "fallthrough", "for", "guard", "if", "in", "repeat", "return", "switch", "where",
    "while", "as", "catch", "false", "is", "nil", "super", "self", "throw", "throws", "true", "try",
];

const RB_KW: &[&str] = &[
    "alias", "and", "begin", "break", "case", "class", "def", "defined", "do", "else", "elsif", "end",
    "ensure", "false", "for", "if", "in", "module", "next", "nil", "not", "or", "redo", "rescue",
    "retry", "return", "self", "super", "then", "true", "undef", "unless", "until", "when", "while",
    "yield", "require", "attr_accessor", "attr_reader", "attr_writer", "puts",
];

const SH_KW: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "function",
    "return", "local", "export", "readonly", "declare", "echo", "cd", "exit", "set", "unset", "source",
    "shift", "trap", "in",
];

const SQL_KW: &[&str] = &[
    "select", "from", "where", "and", "or", "not", "insert", "into", "values", "update", "set", "delete",
    "create", "table", "drop", "alter", "add", "column", "index", "primary", "key", "foreign",
    "references", "join", "left", "right", "inner", "outer", "on", "group", "by", "order", "having",
    "limit", "offset", "union", "all", "distinct", "as", "null", "is", "in", "like", "between", "exists",
    "case", "when", "then", "end", "asc", "desc", "begin", "commit", "rollback", "default", "int",
    "varchar", "text", "timestamp", "boolean", "serial",
];

const TYPES: &[&str] = &[
    "string", "int", "float", "double", "bool", "boolean", "char", "byte", "long", "short", "void",
    "array", "object", "number", "any", "unknown", "never", "tuple", "usize", "isize", "u8", "u16",
    "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64", "str", "Vec", "String", "Option", "Result",
    "Map", "Set", "List", "HashMap", "int64", "float64",
];

fn spec_of(lang: &str) -> Option<Spec> {
    let c = Spec {
        line_com: &["//"],
        block: Some(("/*", "*/")),
        kw: &[],
        ty: TYPES,
        dollar: false,
        at_var: false,
        case_ins: false,
    };
    Some(match lang {
        "php" => Spec { kw: PHP_KW, dollar: true, ..c },
        "javascript" => Spec { kw: JS_KW, ..c },
        "typescript" => Spec { kw: TS_KW, block: Some(("/*", "*/")), ..c },
        "java" => Spec { kw: JAVA_KW, ..c },
        "c" | "cpp" | "objectivec" => Spec { kw: C_KW, ..c },
        "csharp" => Spec { kw: CS_KW, ..c },
        "kotlin" => Spec { kw: KT_KW, ..c },
        "swift" => Spec { kw: SWIFT_KW, ..c },
        "rust" => Spec { kw: RS_KW, ..c },
        "go" => Spec { kw: GO_KW, ..c },
        "json" => Spec { line_com: &[], block: None, kw: &[], ty: &[], ..c },
        "python" => Spec {
            line_com: &["#"],
            block: None,
            kw: PY_KW,
            ty: TYPES,
            dollar: false,
            at_var: false,
            case_ins: false,
        },
        "ruby" => Spec {
            line_com: &["#"],
            block: Some(("=begin", "=end")),
            kw: RB_KW,
            ty: TYPES,
            dollar: false,
            at_var: true,
            case_ins: false,
        },
        "bash" | "makefile" | "dockerfile" => Spec {
            line_com: &["#"],
            block: None,
            kw: SH_KW,
            ty: &[],
            dollar: true,
            at_var: false,
            case_ins: false,
        },
        "yaml" | "ini" | "toml" => Spec {
            line_com: &["#", ";"],
            block: None,
            kw: &["true", "false", "null", "yes", "no", "on", "off"],
            ty: &[],
            dollar: false,
            at_var: false,
            case_ins: true,
        },
        "sql" => Spec {
            line_com: &["--"],
            block: Some(("/*", "*/")),
            kw: SQL_KW,
            ty: TYPES,
            dollar: false,
            at_var: false,
            case_ins: true,
        },
        "nginx" | "apache" => Spec {
            line_com: &["#"],
            block: None,
            kw: &["server", "location", "listen", "server_name", "root", "index", "proxy_pass", "upstream", "if", "return", "rewrite"],
            ty: &[],
            dollar: true,
            at_var: false,
            case_ins: false,
        },
        "lua" => Spec {
            line_com: &["--"],
            block: Some(("--[[", "]]")),
            kw: &["and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while", "self"],
            ty: &[],
            dollar: false,
            at_var: false,
            case_ins: false,
        },
        "graphql" => Spec {
            line_com: &["#"],
            block: None,
            kw: &["query", "mutation", "subscription", "type", "input", "enum", "interface", "union", "fragment", "on", "schema", "scalar", "implements", "extend"],
            ty: &[],
            dollar: true,
            at_var: false,
            case_ins: false,
        },
        _ => return None,
    })
}

// ---------------------------------------------------------------- 通用词法

fn lex(src: &str, s: &Spec) -> Vec<(u8, String)> {
    let c: Vec<char> = src.chars().collect();
    let n = c.len();
    let mut out: Vec<(u8, String)> = Vec::new();
    let mut i = 0;

    while i < n {
        // 行注释
        let mut hit = false;
        for lc in s.line_com {
            if at(&c, i, lc) {
                let st = i;
                while i < n && c[i] != '\n' {
                    i += 1;
                }
                out.push((C, slice(&c, st, i)));
                hit = true;
                break;
            }
        }
        if hit {
            continue;
        }

        // 块注释
        if let Some((bs, be)) = s.block {
            if at(&c, i, bs) {
                let st = i;
                i += bs.chars().count();
                while i < n && !at(&c, i, be) {
                    i += 1;
                }
                i = (i + be.chars().count()).min(n);
                out.push((C, slice(&c, st, i)));
                continue;
            }
        }

        let ch = c[i];

        // 字符串
        if ch == '"' || ch == '\'' || ch == '`' {
            let st = i;
            let q = ch;
            i += 1;
            while i < n {
                if c[i] == '\\' && q != '`' {
                    i += 2;
                    continue;
                }
                if c[i] == q {
                    i += 1;
                    break;
                }
                if c[i] == '\n' && q != '`' {
                    break;
                }
                i += 1;
            }
            i = i.min(n);
            out.push((S, slice(&c, st, i)));
            continue;
        }

        // 数字
        if ch.is_ascii_digit() || (ch == '.' && i + 1 < n && c[i + 1].is_ascii_digit()) {
            let st = i;
            while i < n
                && (c[i].is_ascii_alphanumeric() || c[i] == '.' || c[i] == '_')
            {
                i += 1;
            }
            out.push((N, slice(&c, st, i)));
            continue;
        }

        // 变量前缀
        if (ch == '$' && s.dollar) || (ch == '@' && s.at_var) {
            let st = i;
            i += 1;
            while i < n && ident_char(c[i]) {
                i += 1;
            }
            if i > st + 1 {
                out.push((V, slice(&c, st, i)));
                continue;
            }
            out.push((P, slice(&c, st, i)));
            continue;
        }

        // 标识符
        if ident_start(ch) {
            let st = i;
            while i < n && ident_char(c[i]) {
                i += 1;
            }
            let word = slice(&c, st, i);
            let lw = if s.case_ins {
                word.to_ascii_lowercase()
            } else {
                word.clone()
            };
            let mut k = i;
            while k < n && (c[k] == ' ' || c[k] == '\t') {
                k += 1;
            }
            let is_call = k < n && c[k] == '(';
            let t = if s.kw.iter().any(|x| *x == lw) {
                K
            } else if s.ty.iter().any(|x| *x == lw) {
                Y
            } else if is_call {
                F
            } else if word.chars().next().map(|x| x.is_uppercase()).unwrap_or(false) {
                Y
            } else {
                P
            };
            out.push((t, word));
            continue;
        }

        // 其他：合并为一段 plain
        let st = i;
        while i < n {
            let x = c[i];
            if x == '\n' || x.is_whitespace() && x != ' ' {
                break;
            }
            if x.is_alphanumeric() || x == '"' || x == '\'' || x == '`' || x == '$' {
                break;
            }
            i += 1;
        }
        if i == st {
            i += 1;
        }
        out.push((P, slice(&c, st, i)));
    }
    out
}

// ---------------------------------------------------------------- HTML / XML

fn lex_markup(src: &str) -> Vec<(u8, String)> {
    let c: Vec<char> = src.chars().collect();
    let n = c.len();
    let mut out: Vec<(u8, String)> = Vec::new();
    let mut i = 0;

    while i < n {
        if at(&c, i, "<!--") {
            let st = i;
            i += 4;
            while i < n && !at(&c, i, "-->") {
                i += 1;
            }
            i = (i + 3).min(n);
            out.push((C, slice(&c, st, i)));
            continue;
        }
        if at(&c, i, "{{") {
            let st = i;
            i += 2;
            while i < n && !at(&c, i, "}}") {
                i += 1;
            }
            i = (i + 2).min(n);
            out.push((A, slice(&c, st, i)));
            continue;
        }
        if c[i] == '<' {
            let st = i;
            out.push((P, "<".to_string()));
            i += 1;
            if i < n && c[i] == '/' {
                out.push((P, "/".to_string()));
                i += 1;
            }
            let ns = i;
            while i < n && (c[i].is_alphanumeric() || c[i] == '-' || c[i] == ':' || c[i] == '_') {
                i += 1;
            }
            if i > ns {
                out.push((G, slice(&c, ns, i)));
            }
            // 属性区
            while i < n && c[i] != '>' {
                if c[i] == '"' || c[i] == '\'' {
                    let q = c[i];
                    let s2 = i;
                    i += 1;
                    while i < n && c[i] != q {
                        i += 1;
                    }
                    i = (i + 1).min(n);
                    out.push((S, slice(&c, s2, i)));
                    continue;
                }
                if ident_start(c[i]) {
                    let s2 = i;
                    while i < n && (ident_char(c[i]) || c[i] == '-' || c[i] == ':' || c[i] == '.') {
                        i += 1;
                    }
                    out.push((A, slice(&c, s2, i)));
                    continue;
                }
                if c[i] == '\n' {
                    out.push((P, "\n".into()));
                    i += 1;
                    continue;
                }
                let s2 = i;
                i += 1;
                out.push((P, slice(&c, s2, i)));
            }
            if i < n {
                out.push((P, ">".into()));
                i += 1;
            }
            let _ = st;
            continue;
        }
        let st = i;
        while i < n && c[i] != '<' && !at(&c, i, "{{") {
            i += 1;
        }
        out.push((P, slice(&c, st, i)));
    }
    out
}

// ---------------------------------------------------------------- CSS

fn lex_css(src: &str) -> Vec<(u8, String)> {
    let c: Vec<char> = src.chars().collect();
    let n = c.len();
    let mut out: Vec<(u8, String)> = Vec::new();
    let mut i = 0;
    let mut depth = 0;
    while i < n {
        if at(&c, i, "/*") {
            let st = i;
            i += 2;
            while i < n && !at(&c, i, "*/") {
                i += 1;
            }
            i = (i + 2).min(n);
            out.push((C, slice(&c, st, i)));
            continue;
        }
        if at(&c, i, "//") {
            let st = i;
            while i < n && c[i] != '\n' {
                i += 1;
            }
            out.push((C, slice(&c, st, i)));
            continue;
        }
        let ch = c[i];
        if ch == '"' || ch == '\'' {
            let q = ch;
            let st = i;
            i += 1;
            while i < n && c[i] != q {
                if c[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(n);
            out.push((S, slice(&c, st, i)));
            continue;
        }
        if ch == '{' {
            depth += 1;
            out.push((P, "{".into()));
            i += 1;
            continue;
        }
        if ch == '}' {
            depth = (depth - 1).max(0);
            out.push((P, "}".into()));
            i += 1;
            continue;
        }
        if ch == '@' {
            let st = i;
            i += 1;
            while i < n && ident_char(c[i]) {
                i += 1;
            }
            out.push((K, slice(&c, st, i)));
            continue;
        }
        if ch == '#' || ch == '.' {
            // 选择器或颜色
            let st = i;
            i += 1;
            while i < n && (ident_char(c[i]) || c[i] == '-') {
                i += 1;
            }
            let word = slice(&c, st, i);
            if depth > 0 && ch == '#' && word.len() >= 4 && word[1..].chars().all(|x| x.is_ascii_hexdigit()) {
                out.push((N, word));
            } else {
                out.push((Y, word));
            }
            continue;
        }
        if ch.is_ascii_digit() {
            let st = i;
            while i < n && (c[i].is_ascii_alphanumeric() || c[i] == '.' || c[i] == '%' || c[i] == '-') {
                i += 1;
            }
            out.push((N, slice(&c, st, i)));
            continue;
        }
        if ident_start(ch) || ch == '-' {
            let st = i;
            while i < n && (ident_char(c[i]) || c[i] == '-') {
                i += 1;
            }
            let word = slice(&c, st, i);
            let mut k = i;
            while k < n && (c[k] == ' ' || c[k] == '\t') {
                k += 1;
            }
            let t = if depth > 0 && k < n && c[k] == ':' {
                A
            } else if word.starts_with("--") {
                V
            } else {
                P
            };
            out.push((t, word));
            continue;
        }
        if ch == '!' {
            let st = i;
            i += 1;
            while i < n && c[i] != ';' && c[i] != '\n' {
                i += 1;
            }
            out.push((K, slice(&c, st, i)));
            continue;
        }
        out.push((P, ch.to_string()));
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- Markdown / diff

fn lex_markdown(src: &str) -> Vec<(u8, String)> {
    let mut out: Vec<(u8, String)> = Vec::new();
    let mut fence = false;
    for (idx, line) in src.split('\n').enumerate() {
        if idx > 0 {
            out.push((P, "\n".into()));
        }
        let t = line.trim_start();
        if t.starts_with("```") {
            fence = !fence;
            out.push((C, line.to_string()));
            continue;
        }
        if fence {
            out.push((P, line.to_string()));
            continue;
        }
        if t.starts_with('#') {
            out.push((H, line.to_string()));
            continue;
        }
        if t.starts_with('>') {
            out.push((C, line.to_string()));
            continue;
        }
        // 行内：`code` **bold** [link](url)
        let c: Vec<char> = line.chars().collect();
        let n = c.len();
        let mut i = 0;
        while i < n {
            if c[i] == '`' {
                let st = i;
                i += 1;
                while i < n && c[i] != '`' {
                    i += 1;
                }
                i = (i + 1).min(n);
                out.push((S, slice(&c, st, i)));
                continue;
            }
            if c[i] == '*' && at(&c, i, "**") {
                let st = i;
                i += 2;
                while i < n && !at(&c, i, "**") {
                    i += 1;
                }
                i = (i + 2).min(n);
                out.push((K, slice(&c, st, i)));
                continue;
            }
            if c[i] == '[' {
                let st = i;
                while i < n && c[i] != ']' {
                    i += 1;
                }
                if i + 1 < n && c[i + 1] == '(' {
                    i += 2;
                    while i < n && c[i] != ')' {
                        i += 1;
                    }
                    i = (i + 1).min(n);
                    out.push((A, slice(&c, st, i)));
                    continue;
                }
                out.push((P, slice(&c, st, i)));
                continue;
            }
            let st = i;
            while i < n && c[i] != '`' && c[i] != '*' && c[i] != '[' {
                i += 1;
            }
            if i == st {
                i += 1;
            }
            out.push((P, slice(&c, st, i)));
        }
    }
    out
}

fn lex_diff(src: &str) -> Vec<(u8, String)> {
    let mut out: Vec<(u8, String)> = Vec::new();
    for (idx, line) in src.split('\n').enumerate() {
        if idx > 0 {
            out.push((P, "\n".into()));
        }
        if line.starts_with("+++") || line.starts_with("---") {
            out.push((C, line.to_string()));
        } else if line.starts_with('+') {
            out.push((I, line.to_string()));
        } else if line.starts_with('-') {
            out.push((D, line.to_string()));
        } else if line.starts_with("@@") {
            out.push((Y, line.to_string()));
        } else if line.starts_with("diff ") || line.starts_with("index ") || line.starts_with("new file") {
            out.push((C, line.to_string()));
        } else {
            out.push((P, line.to_string()));
        }
    }
    out
}

// ---------------------------------------------------------------- 入口

pub fn tokenize(src: &str, lang: &str) -> Vec<Line> {
    let spans = match lang {
        "diff" => lex_diff(src),
        "markdown" => lex_markdown(src),
        "xml" => lex_markup(src),
        "css" | "scss" | "less" => lex_css(src),
        _ => match spec_of(lang) {
            Some(s) => lex(src, &s),
            None => vec![(P, src.to_string())],
        },
    };
    split_lines(spans)
}

pub fn font() -> FontId {
    let size = crate::theme::code_size();
    let idx = crate::theme::code_font();
    if idx > 0 {
        let list = crate::theme::alt_fonts();
        let i = (idx as usize - 1).min(list.len().saturating_sub(1));
        if let Some(name) = list.get(i) {
            return FontId::new(size, FontFamily::Name(name.clone().into()));
        }
    }
    FontId::monospace(size)
}

pub fn build_job(lines: &[Line], wrap: bool, width: f32) -> egui::text::LayoutJob {
    let f = font();
    let mut job = egui::text::LayoutJob::default();
    job.wrap = if wrap {
        TextWrapping {
            max_width: width.max(80.0),
            max_rows: usize::MAX,
            break_anywhere: true,
            overflow_character: None,
        }
    } else {
        TextWrapping::no_max_width()
    };
    let base = TextFormat {
        font_id: f.clone(),
        color: color(P),
        line_height: Some(crate::theme::line_height()),
        ..Default::default()
    };
    for (idx, line) in lines.iter().enumerate() {
        if line.spans.is_empty() {
            job.append(" ", 0.0, base.clone());
        } else {
            for (t, s) in &line.spans {
                job.append(
                    s,
                    0.0,
                    TextFormat {
                        font_id: f.clone(),
                        color: color(*t),
                        background: bg(*t),
                        line_height: Some(crate::theme::line_height()),
                        ..Default::default()
                    },
                );
            }
        }
        if idx + 1 < lines.len() {
            job.append("\n", 0.0, base.clone());
        }
    }
    job
}
