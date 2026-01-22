use super::super::*;
use std::cell::RefCell;
use std::sync::OnceLock;
use tree_sitter::StreamingIterator;

thread_local! {
    static TS_PARSER: RefCell<tree_sitter::Parser> = RefCell::new(tree_sitter::Parser::new());
    static TS_CURSOR: RefCell<tree_sitter::QueryCursor> = RefCell::new(tree_sitter::QueryCursor::new());
    static TS_INPUT: RefCell<String> = RefCell::new(String::new());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum DiffSyntaxLanguage {
    Plain,
    Html,
    Css,
    Hcl,
    Bicep,
    Lua,
    Makefile,
    Kotlin,
    Zig,
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    C,
    Cpp,
    CSharp,
    FSharp,
    VisualBasic,
    Java,
    Php,
    Ruby,
    Json,
    Toml,
    Yaml,
    Bash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum DiffSyntaxMode {
    Auto,
    HeuristicOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SyntaxToken {
    pub(super) range: Range<usize>,
    pub(super) kind: SyntaxTokenKind,
}

pub(in super::super) fn diff_syntax_language_for_path(path: &str) -> Option<DiffSyntaxLanguage> {
    let p = std::path::Path::new(path);
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let file_name = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    Some(match ext.as_str() {
        "html" | "htm" => DiffSyntaxLanguage::Html,
        // Use HTML highlighting for XML-ish formats as a pragmatic baseline.
        "xml" | "svg" | "xsl" | "xslt" | "xsd" => DiffSyntaxLanguage::Html,
        "css" | "less" | "sass" | "scss" => DiffSyntaxLanguage::Css,
        "hcl" | "tf" | "tfvars" => DiffSyntaxLanguage::Hcl,
        "bicep" => DiffSyntaxLanguage::Bicep,
        "lua" => DiffSyntaxLanguage::Lua,
        "mk" => DiffSyntaxLanguage::Makefile,
        "kt" | "kts" => DiffSyntaxLanguage::Kotlin,
        "zig" => DiffSyntaxLanguage::Zig,
        "rs" => DiffSyntaxLanguage::Rust,
        "py" => DiffSyntaxLanguage::Python,
        "js" | "jsx" | "mjs" | "cjs" => DiffSyntaxLanguage::JavaScript,
        "ts" | "cts" | "mts" => DiffSyntaxLanguage::TypeScript,
        "tsx" => DiffSyntaxLanguage::Tsx,
        "go" => DiffSyntaxLanguage::Go,
        "c" | "h" => DiffSyntaxLanguage::C,
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => DiffSyntaxLanguage::Cpp,
        "cs" => DiffSyntaxLanguage::CSharp,
        "fs" | "fsx" | "fsi" => DiffSyntaxLanguage::FSharp,
        "vb" | "vbs" => DiffSyntaxLanguage::VisualBasic,
        "java" => DiffSyntaxLanguage::Java,
        "php" | "phtml" => DiffSyntaxLanguage::Php,
        "rb" => DiffSyntaxLanguage::Ruby,
        "json" => DiffSyntaxLanguage::Json,
        "toml" => DiffSyntaxLanguage::Toml,
        "yaml" | "yml" => DiffSyntaxLanguage::Yaml,
        "sh" | "bash" | "zsh" => DiffSyntaxLanguage::Bash,
        _ => {
            if file_name == "makefile" || file_name == "gnumakefile" {
                DiffSyntaxLanguage::Makefile
            } else {
                return None;
            }
        }
    })
}

pub(super) fn syntax_tokens_for_line(
    text: &str,
    language: DiffSyntaxLanguage,
    mode: DiffSyntaxMode,
) -> Vec<SyntaxToken> {
    match mode {
        DiffSyntaxMode::HeuristicOnly => syntax_tokens_for_line_heuristic(text, language),
        DiffSyntaxMode::Auto => {
            if !should_use_treesitter_for_line(text) {
                return syntax_tokens_for_line_heuristic(text, language);
            }
            if let Some(tokens) = syntax_tokens_for_line_treesitter(text, language) {
                return tokens;
            }
            syntax_tokens_for_line_heuristic(text, language)
        }
    }
}

fn should_use_treesitter_for_line(text: &str) -> bool {
    text.len() <= MAX_TREESITTER_LINE_BYTES
}

struct TreesitterHighlightSpec {
    query: tree_sitter::Query,
    capture_kinds: Vec<Option<SyntaxTokenKind>>,
}

fn syntax_tokens_for_line_treesitter(
    text: &str,
    language: DiffSyntaxLanguage,
) -> Option<Vec<SyntaxToken>> {
    let ts_language = tree_sitter_language(language)?;
    let highlight = tree_sitter_highlight_spec(language)?;

    let input_len = text.len();
    let tree = TS_INPUT.with(|input| {
        let mut input = input.borrow_mut();
        input.clear();
        input.push_str(text);
        input.push('\n');

        TS_PARSER.with(|parser| {
            let mut parser = parser.borrow_mut();
            parser.set_language(&ts_language).ok()?;
            parser.parse(&*input, None)
        })
    })?;

    let mut tokens: Vec<SyntaxToken> = Vec::new();
    TS_INPUT.with(|input| {
        let input = input.borrow();
        TS_CURSOR.with(|cursor| {
            let mut cursor = cursor.borrow_mut();
            let mut captures =
                cursor.captures(&highlight.query, tree.root_node(), input.as_bytes());
            tree_sitter::StreamingIterator::advance(&mut captures);
            while let Some((m, capture_ix)) = captures.get() {
                let Some(capture) = m.captures.get(*capture_ix) else {
                    tree_sitter::StreamingIterator::advance(&mut captures);
                    continue;
                };

                let Some(kind) = highlight
                    .capture_kinds
                    .get(capture.index as usize)
                    .copied()
                    .flatten()
                else {
                    tree_sitter::StreamingIterator::advance(&mut captures);
                    continue;
                };

                let mut range = capture.node.byte_range();
                range.start = range.start.min(input_len);
                range.end = range.end.min(input_len);
                if range.start < range.end {
                    tokens.push(SyntaxToken { range, kind });
                }

                tree_sitter::StreamingIterator::advance(&mut captures);
            }
        });
    });

    if tokens.is_empty() {
        return Some(tokens);
    }

    tokens.sort_by(|a, b| {
        a.range
            .start
            .cmp(&b.range.start)
            .then(a.range.end.cmp(&b.range.end))
    });

    // Ensure non-overlapping tokens so the segment splitter can pick a single style per range.
    let mut out: Vec<SyntaxToken> = Vec::with_capacity(tokens.len());
    for mut token in tokens {
        if let Some(prev) = out.last()
            && token.range.start < prev.range.end
        {
            if token.range.end <= prev.range.end {
                continue;
            }
            token.range.start = prev.range.end;
            if token.range.start >= token.range.end {
                continue;
            }
        }
        out.push(token);
    }

    Some(out)
}

fn tree_sitter_language(language: DiffSyntaxLanguage) -> Option<tree_sitter::Language> {
    Some(match language {
        DiffSyntaxLanguage::Plain => return None,
        DiffSyntaxLanguage::Html => tree_sitter_html::LANGUAGE.into(),
        DiffSyntaxLanguage::Css => tree_sitter_css::LANGUAGE.into(),
        DiffSyntaxLanguage::Hcl => return None,
        DiffSyntaxLanguage::Bicep => return None,
        DiffSyntaxLanguage::Lua => return None,
        DiffSyntaxLanguage::Makefile => return None,
        DiffSyntaxLanguage::Kotlin => return None,
        DiffSyntaxLanguage::Zig => return None,
        DiffSyntaxLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        DiffSyntaxLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        DiffSyntaxLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        DiffSyntaxLanguage::C => return None,
        DiffSyntaxLanguage::Cpp => return None,
        DiffSyntaxLanguage::CSharp => return None,
        DiffSyntaxLanguage::FSharp => return None,
        DiffSyntaxLanguage::VisualBasic => return None,
        DiffSyntaxLanguage::Java => return None,
        DiffSyntaxLanguage::Php => return None,
        DiffSyntaxLanguage::Ruby => return None,
        DiffSyntaxLanguage::Json => tree_sitter_json::LANGUAGE.into(),
        DiffSyntaxLanguage::Yaml => tree_sitter_yaml::language(),
        DiffSyntaxLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        DiffSyntaxLanguage::Tsx | DiffSyntaxLanguage::JavaScript => {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        }
        DiffSyntaxLanguage::Bash => tree_sitter_bash::LANGUAGE.into(),
        DiffSyntaxLanguage::Toml => return None,
    })
}

fn tree_sitter_highlight_spec(
    language: DiffSyntaxLanguage,
) -> Option<&'static TreesitterHighlightSpec> {
    static HTML: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static CSS: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static RUST: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static PY: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static GO: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static JSON: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static YAML: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static TS: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static TSX: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static JS: OnceLock<TreesitterHighlightSpec> = OnceLock::new();
    static BASH: OnceLock<TreesitterHighlightSpec> = OnceLock::new();

    let init = |language: tree_sitter::Language, source: &'static str| -> TreesitterHighlightSpec {
        let query =
            tree_sitter::Query::new(&language, source).expect("highlights.scm should compile");
        let capture_kinds = query
            .capture_names()
            .iter()
            .map(|name| syntax_kind_from_capture_name(name))
            .collect::<Vec<_>>();
        TreesitterHighlightSpec {
            query,
            capture_kinds,
        }
    };

    Some(match language {
        DiffSyntaxLanguage::Plain => return None,
        DiffSyntaxLanguage::Html => HTML.get_or_init(|| {
            init(
                tree_sitter_html::LANGUAGE.into(),
                include_str!("../../../../../../zed/extensions/html/languages/html/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::Css => CSS.get_or_init(|| {
            init(
                tree_sitter_css::LANGUAGE.into(),
                include_str!("../../../../../../zed/crates/languages/src/css/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::Hcl => return None,
        DiffSyntaxLanguage::Bicep => return None,
        DiffSyntaxLanguage::Lua => return None,
        DiffSyntaxLanguage::Makefile => return None,
        DiffSyntaxLanguage::Kotlin => return None,
        DiffSyntaxLanguage::Zig => return None,
        DiffSyntaxLanguage::Rust => RUST.get_or_init(|| {
            init(
                tree_sitter_rust::LANGUAGE.into(),
                include_str!("../../../../../../zed/crates/languages/src/rust/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::Python => PY.get_or_init(|| {
            init(
                tree_sitter_python::LANGUAGE.into(),
                include_str!("../../../../../../zed/crates/languages/src/python/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::Go => GO.get_or_init(|| {
            init(
                tree_sitter_go::LANGUAGE.into(),
                include_str!("../../../../../../zed/crates/languages/src/go/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::C => return None,
        DiffSyntaxLanguage::Cpp => return None,
        DiffSyntaxLanguage::CSharp => return None,
        DiffSyntaxLanguage::FSharp => return None,
        DiffSyntaxLanguage::VisualBasic => return None,
        DiffSyntaxLanguage::Java => return None,
        DiffSyntaxLanguage::Php => return None,
        DiffSyntaxLanguage::Ruby => return None,
        DiffSyntaxLanguage::Json => JSON.get_or_init(|| {
            init(
                tree_sitter_json::LANGUAGE.into(),
                include_str!("../../../../../../zed/crates/languages/src/json/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::Yaml => YAML.get_or_init(|| {
            init(
                tree_sitter_yaml::language(),
                include_str!("../../../../../../zed/crates/languages/src/yaml/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::TypeScript => TS.get_or_init(|| {
            init(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                include_str!(
                    "../../../../../../zed/crates/languages/src/typescript/highlights.scm"
                ),
            )
        }),
        DiffSyntaxLanguage::Tsx => TSX.get_or_init(|| {
            init(
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                include_str!("../../../../../../zed/crates/languages/src/tsx/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::JavaScript => JS.get_or_init(|| {
            init(
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                include_str!(
                    "../../../../../../zed/crates/languages/src/javascript/highlights.scm"
                ),
            )
        }),
        DiffSyntaxLanguage::Bash => BASH.get_or_init(|| {
            init(
                tree_sitter_bash::LANGUAGE.into(),
                include_str!("../../../../../../zed/crates/languages/src/bash/highlights.scm"),
            )
        }),
        DiffSyntaxLanguage::Toml => return None,
    })
}

fn syntax_kind_from_capture_name(name: &str) -> Option<SyntaxTokenKind> {
    let base = name.split('.').next().unwrap_or(name);
    Some(match base {
        "comment" => SyntaxTokenKind::Comment,
        "string" | "character" => SyntaxTokenKind::String,
        "keyword" => SyntaxTokenKind::Keyword,
        "include" | "preproc" => SyntaxTokenKind::Keyword,
        "number" => SyntaxTokenKind::Number,
        "boolean" => SyntaxTokenKind::Constant,
        "function" | "constructor" | "method" => SyntaxTokenKind::Function,
        "type" => SyntaxTokenKind::Type,
        // Tree-sitter highlight queries often capture most identifiers as `variable.*`.
        // Coloring these makes Rust diffs look like "everything is blue", so we skip them.
        "variable" => return None,
        "property" | "field" | "attribute" => SyntaxTokenKind::Property,
        "tag" | "namespace" | "selector" => SyntaxTokenKind::Type,
        "constant" => SyntaxTokenKind::Constant,
        "punctuation" | "operator" => SyntaxTokenKind::Punctuation,
        _ => return None,
    })
}

fn syntax_tokens_for_line_heuristic(text: &str, language: DiffSyntaxLanguage) -> Vec<SyntaxToken> {
    let mut tokens: Vec<SyntaxToken> = Vec::new();
    let len = text.len();
    let mut i = 0usize;

    let is_ident_start = |ch: char| ch == '_' || ch.is_ascii_alphabetic();
    let is_ident_continue = |ch: char| ch == '_' || ch.is_ascii_alphanumeric();
    let is_digit = |ch: char| ch.is_ascii_digit();

    while i < len {
        let rest = &text[i..];

        if matches!(language, DiffSyntaxLanguage::Html) && rest.starts_with("<!--") {
            let end = rest.find("-->").map(|ix| i + ix + 3).unwrap_or(len);
            tokens.push(SyntaxToken {
                range: i..end,
                kind: SyntaxTokenKind::Comment,
            });
            i = end;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::FSharp) && rest.starts_with("(*") {
            let end = rest.find("*)").map(|ix| i + ix + 2).unwrap_or(len);
            tokens.push(SyntaxToken {
                range: i..end,
                kind: SyntaxTokenKind::Comment,
            });
            i = end;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::Lua) && rest.starts_with("--") {
            if rest.starts_with("--[[") {
                let end = rest.find("]]").map(|ix| i + ix + 2).unwrap_or(len);
                tokens.push(SyntaxToken {
                    range: i..end,
                    kind: SyntaxTokenKind::Comment,
                });
                i = end;
                continue;
            }
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        let (line_comment, hash_comment, block_comment) = match language {
            DiffSyntaxLanguage::Python | DiffSyntaxLanguage::Toml | DiffSyntaxLanguage::Yaml => {
                (None, Some('#'), false)
            }
            DiffSyntaxLanguage::Bash => (None, Some('#'), false),
            DiffSyntaxLanguage::Makefile => (None, Some('#'), false),
            DiffSyntaxLanguage::Rust
            | DiffSyntaxLanguage::JavaScript
            | DiffSyntaxLanguage::TypeScript
            | DiffSyntaxLanguage::Tsx
            | DiffSyntaxLanguage::Go
            | DiffSyntaxLanguage::C
            | DiffSyntaxLanguage::Cpp
            | DiffSyntaxLanguage::CSharp
            | DiffSyntaxLanguage::Java
            | DiffSyntaxLanguage::Kotlin
            | DiffSyntaxLanguage::Zig
            | DiffSyntaxLanguage::Bicep => (Some("//"), None, true),
            DiffSyntaxLanguage::Hcl => (Some("//"), Some('#'), true),
            DiffSyntaxLanguage::Php => (Some("//"), Some('#'), true),
            DiffSyntaxLanguage::Ruby
            | DiffSyntaxLanguage::FSharp
            | DiffSyntaxLanguage::VisualBasic
            | DiffSyntaxLanguage::Html
            | DiffSyntaxLanguage::Css => (None, None, false),
            DiffSyntaxLanguage::Json => (None, None, false),
            DiffSyntaxLanguage::Plain => (Some("//"), Some('#'), true),
            DiffSyntaxLanguage::Lua => (None, None, false),
        };

        if let Some(prefix) = line_comment
            && rest.starts_with(prefix)
        {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        if block_comment && rest.starts_with("/*") {
            let end = rest.find("*/").map(|ix| i + ix + 2).unwrap_or(len);
            tokens.push(SyntaxToken {
                range: i..end,
                kind: SyntaxTokenKind::Comment,
            });
            i = end;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::Ruby) && rest.starts_with('#') {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        if matches!(language, DiffSyntaxLanguage::VisualBasic)
            && (rest.starts_with('\'') || rest.to_ascii_lowercase().starts_with("rem "))
        {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        if let Some('#') = hash_comment
            && rest.starts_with('#')
        {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };

        if ch == '"'
            || ch == '\''
            || (ch == '`'
                && matches!(
                    language,
                    DiffSyntaxLanguage::JavaScript
                        | DiffSyntaxLanguage::TypeScript
                        | DiffSyntaxLanguage::Tsx
                        | DiffSyntaxLanguage::Go
                        | DiffSyntaxLanguage::Bash
                        | DiffSyntaxLanguage::Plain
                ))
        {
            let quote = ch;
            let mut j = i + quote.len_utf8();
            let mut escaped = false;
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                let next_len = next.len_utf8();
                if escaped {
                    escaped = false;
                    j += next_len;
                    continue;
                }
                if next == '\\' {
                    escaped = true;
                    j += next_len;
                    continue;
                }
                if next == quote {
                    j += next_len;
                    break;
                }
                j += next_len;
            }

            tokens.push(SyntaxToken {
                range: i..j.min(len),
                kind: SyntaxTokenKind::String,
            });
            i = j.min(len);
            continue;
        }

        if ch.is_ascii_digit() {
            let mut j = i;
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                if is_digit(next) || next == '_' || next == '.' || next == 'x' || next == 'b' {
                    j += next.len_utf8();
                } else {
                    break;
                }
            }
            if j > i {
                tokens.push(SyntaxToken {
                    range: i..j,
                    kind: SyntaxTokenKind::Number,
                });
                i = j;
                continue;
            }
        }

        if is_ident_start(ch) {
            let mut j = i + ch.len_utf8();
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                if is_ident_continue(next) {
                    j += next.len_utf8();
                } else {
                    break;
                }
            }
            let ident = &text[i..j];
            if is_keyword(language, ident) {
                tokens.push(SyntaxToken {
                    range: i..j,
                    kind: SyntaxTokenKind::Keyword,
                });
            }
            i = j;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::Css) && (ch == '.' || ch == '#') {
            let mut j = i + 1;
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                if is_ident_continue(next) || next == '-' {
                    j += next.len_utf8();
                } else {
                    break;
                }
            }
            if j > i + 1 {
                tokens.push(SyntaxToken {
                    range: i..j,
                    kind: SyntaxTokenKind::Type,
                });
                i = j;
                continue;
            }
        }

        i += ch.len_utf8();
    }

    tokens
}

fn is_keyword(language: DiffSyntaxLanguage, ident: &str) -> bool {
    // NOTE: This is a heuristic fallback when we don't want to use tree-sitter for a line.
    match language {
        DiffSyntaxLanguage::Plain => matches!(ident, "true" | "false" | "null"),
        DiffSyntaxLanguage::Html => matches!(ident, "true" | "false"),
        DiffSyntaxLanguage::Css => matches!(ident, "true" | "false"),
        DiffSyntaxLanguage::Hcl => matches!(
            ident,
            "true" | "false" | "null" | "for" | "in" | "if" | "else" | "endif" | "endfor"
        ),
        DiffSyntaxLanguage::Bicep => matches!(
            ident,
            "param" | "var" | "resource" | "module" | "output" | "existing" | "true" | "false"
        ),
        DiffSyntaxLanguage::Lua => matches!(
            ident,
            "and"
                | "break"
                | "do"
                | "else"
                | "elseif"
                | "end"
                | "false"
                | "for"
                | "function"
                | "goto"
                | "if"
                | "in"
                | "local"
                | "nil"
                | "not"
                | "or"
                | "repeat"
                | "return"
                | "then"
                | "true"
                | "until"
                | "while"
        ),
        DiffSyntaxLanguage::Makefile => matches!(ident, "if" | "else" | "endif"),
        DiffSyntaxLanguage::Kotlin => matches!(
            ident,
            "as" | "break"
                | "class"
                | "continue"
                | "do"
                | "else"
                | "false"
                | "for"
                | "fun"
                | "if"
                | "in"
                | "interface"
                | "is"
                | "null"
                | "object"
                | "package"
                | "return"
                | "super"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typealias"
                | "val"
                | "var"
                | "when"
                | "while"
        ),
        DiffSyntaxLanguage::Zig => matches!(
            ident,
            "const"
                | "var"
                | "fn"
                | "pub"
                | "usingnamespace"
                | "test"
                | "if"
                | "else"
                | "while"
                | "for"
                | "switch"
                | "and"
                | "or"
                | "orelse"
                | "break"
                | "continue"
                | "return"
                | "try"
                | "catch"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::Rust => matches!(
            ident,
            "as" | "async"
                | "await"
                | "break"
                | "const"
                | "continue"
                | "crate"
                | "dyn"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "pub"
                | "ref"
                | "return"
                | "Self"
                | "self"
                | "static"
                | "struct"
                | "super"
                | "trait"
                | "true"
                | "type"
                | "unsafe"
                | "use"
                | "where"
                | "while"
        ),
        DiffSyntaxLanguage::Python => matches!(
            ident,
            "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "False"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "None"
                | "nonlocal"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "True"
                | "try"
                | "while"
                | "with"
                | "yield"
        ),
        DiffSyntaxLanguage::JavaScript
        | DiffSyntaxLanguage::TypeScript
        | DiffSyntaxLanguage::Tsx => {
            matches!(
                ident,
                "break"
                    | "case"
                    | "catch"
                    | "class"
                    | "const"
                    | "continue"
                    | "debugger"
                    | "default"
                    | "delete"
                    | "do"
                    | "else"
                    | "export"
                    | "extends"
                    | "false"
                    | "finally"
                    | "for"
                    | "function"
                    | "if"
                    | "import"
                    | "in"
                    | "instanceof"
                    | "new"
                    | "null"
                    | "return"
                    | "super"
                    | "switch"
                    | "this"
                    | "throw"
                    | "true"
                    | "try"
                    | "typeof"
                    | "var"
                    | "void"
                    | "while"
                    | "with"
                    | "yield"
            )
        }
        DiffSyntaxLanguage::Go => matches!(
            ident,
            "break"
                | "case"
                | "chan"
                | "const"
                | "continue"
                | "default"
                | "defer"
                | "else"
                | "fallthrough"
                | "for"
                | "func"
                | "go"
                | "goto"
                | "if"
                | "import"
                | "interface"
                | "map"
                | "package"
                | "range"
                | "return"
                | "select"
                | "struct"
                | "switch"
                | "type"
                | "var"
        ),
        DiffSyntaxLanguage::C | DiffSyntaxLanguage::Cpp | DiffSyntaxLanguage::CSharp => matches!(
            ident,
            "auto"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "for"
                | "goto"
                | "if"
                | "inline"
                | "new"
                | "nullptr"
                | "private"
                | "protected"
                | "public"
                | "return"
                | "sizeof"
                | "static"
                | "struct"
                | "switch"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typedef"
                | "typename"
                | "union"
                | "using"
                | "virtual"
                | "void"
                | "volatile"
                | "while"
        ),
        DiffSyntaxLanguage::FSharp => matches!(
            ident,
            "let"
                | "in"
                | "match"
                | "with"
                | "type"
                | "member"
                | "interface"
                | "abstract"
                | "override"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::VisualBasic => matches!(
            ident,
            "Dim"
                | "As"
                | "If"
                | "Then"
                | "Else"
                | "End"
                | "For"
                | "Each"
                | "In"
                | "Next"
                | "While"
                | "Do"
                | "Loop"
                | "True"
                | "False"
                | "Nothing"
        ),
        DiffSyntaxLanguage::Java => matches!(
            ident,
            "abstract"
                | "assert"
                | "boolean"
                | "break"
                | "byte"
                | "case"
                | "catch"
                | "char"
                | "class"
                | "const"
                | "continue"
                | "default"
                | "do"
                | "double"
                | "else"
                | "enum"
                | "extends"
                | "final"
                | "finally"
                | "float"
                | "for"
                | "goto"
                | "if"
                | "implements"
                | "import"
                | "instanceof"
                | "int"
                | "interface"
                | "long"
                | "native"
                | "new"
                | "null"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "return"
                | "short"
                | "static"
                | "strictfp"
                | "super"
                | "switch"
                | "synchronized"
                | "this"
                | "throw"
                | "throws"
                | "transient"
                | "true"
                | "false"
                | "try"
                | "void"
                | "volatile"
                | "while"
        ),
        DiffSyntaxLanguage::Php => matches!(
            ident.to_ascii_lowercase().as_str(),
            "function"
                | "class"
                | "public"
                | "private"
                | "protected"
                | "static"
                | "final"
                | "abstract"
                | "extends"
                | "implements"
                | "use"
                | "namespace"
                | "return"
                | "if"
                | "else"
                | "elseif"
                | "for"
                | "foreach"
                | "while"
                | "do"
                | "switch"
                | "case"
                | "default"
                | "try"
                | "catch"
                | "finally"
                | "throw"
                | "new"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::Ruby => matches!(
            ident,
            "def"
                | "class"
                | "module"
                | "end"
                | "if"
                | "elsif"
                | "else"
                | "unless"
                | "case"
                | "when"
                | "while"
                | "until"
                | "for"
                | "in"
                | "do"
                | "break"
                | "next"
                | "redo"
                | "retry"
                | "return"
                | "yield"
                | "super"
                | "self"
                | "true"
                | "false"
                | "nil"
        ),
        DiffSyntaxLanguage::Json => matches!(ident, "true" | "false" | "null"),
        DiffSyntaxLanguage::Toml => matches!(ident, "true" | "false"),
        DiffSyntaxLanguage::Yaml => matches!(ident, "true" | "false" | "null"),
        DiffSyntaxLanguage::Bash => matches!(
            ident,
            "if" | "then"
                | "else"
                | "elif"
                | "fi"
                | "for"
                | "in"
                | "do"
                | "done"
                | "case"
                | "esac"
                | "while"
                | "function"
                | "return"
                | "break"
                | "continue"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn treesitter_line_length_guard() {
        assert!(super::should_use_treesitter_for_line("fn main() {}"));
        assert!(!super::should_use_treesitter_for_line(
            &"a".repeat(MAX_TREESITTER_LINE_BYTES + 1)
        ));
    }

    #[test]
    fn xml_uses_html_highlighting() {
        assert_eq!(
            diff_syntax_language_for_path("foo.xml"),
            Some(DiffSyntaxLanguage::Html)
        );
    }

    #[test]
    fn treesitter_variable_capture_is_not_colored() {
        assert_eq!(super::syntax_kind_from_capture_name("variable"), None);
        assert_eq!(
            super::syntax_kind_from_capture_name("variable.parameter"),
            None
        );
    }

    #[test]
    fn treesitter_tokenization_is_safe_across_languages() {
        let rust_line = "fn main() { let x = 1; }";
        let json_line = "{\"x\": 1}";

        let rust =
            syntax_tokens_for_line(rust_line, DiffSyntaxLanguage::Rust, DiffSyntaxMode::Auto);
        let json =
            syntax_tokens_for_line(json_line, DiffSyntaxLanguage::Json, DiffSyntaxMode::Auto);

        for t in rust {
            assert!(t.range.start <= t.range.end);
            assert!(t.range.end <= rust_line.len());
        }
        for t in json {
            assert!(t.range.start <= t.range.end);
            assert!(t.range.end <= json_line.len());
        }
    }

    #[test]
    #[ignore]
    fn perf_treesitter_tokenization_smoke() {
        let text = "fn main() { let x = Some(123); println!(\"{x:?}\"); }";
        let start = Instant::now();
        for _ in 0..200_000 {
            let _ = syntax_tokens_for_line(text, DiffSyntaxLanguage::Rust, DiffSyntaxMode::Auto);
        }
        eprintln!("syntax_tokens_for_line (rust): {:?}", start.elapsed());
    }
}
