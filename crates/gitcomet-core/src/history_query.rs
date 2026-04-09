use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HistoryQueryField {
    Message,
    Author,
    Ref,
    Sha,
    File,
    Content,
}

impl HistoryQueryField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Author => "author",
            Self::Ref => "ref",
            Self::Sha => "sha",
            Self::File => "file",
            Self::Content => "content",
        }
    }

    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "message" | "msg" | "m" => Some(Self::Message),
            "author" | "a" => Some(Self::Author),
            "ref" | "refs" | "r" => Some(Self::Ref),
            "sha" | "commit" | "id" => Some(Self::Sha),
            "file" | "path" | "f" => Some(Self::File),
            "content" | "contents" | "diff" | "patch" | "c" => Some(Self::Content),
            _ => None,
        }
    }
}

impl fmt::Display for HistoryQueryField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HistoryQueryTerm {
    Plain(Arc<str>),
    Field {
        field: HistoryQueryField,
        value: Arc<str>,
    },
}

impl HistoryQueryTerm {
    pub fn plain(value: impl Into<Arc<str>>) -> Self {
        Self::Plain(value.into())
    }

    pub fn field(field: HistoryQueryField, value: impl Into<Arc<str>>) -> Self {
        Self::Field {
            field,
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Self::Plain(value) => value.as_ref(),
            Self::Field { value, .. } => value.as_ref(),
        }
    }

    pub fn field_kind(&self) -> Option<HistoryQueryField> {
        match self {
            Self::Plain(_) => None,
            Self::Field { field, .. } => Some(*field),
        }
    }
}

impl fmt::Display for HistoryQueryTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain(value) => write_query_value(f, value),
            Self::Field { field, value } => {
                write!(f, "{field}:")?;
                write_query_value(f, value)
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct HistoryQuery {
    terms: Vec<HistoryQueryTerm>,
}

impl HistoryQuery {
    pub fn new(terms: Vec<HistoryQueryTerm>) -> Self {
        Self { terms }
    }

    pub fn parse(raw: &str) -> Self {
        let tokens = tokenize_history_query(raw);
        if tokens.is_empty() {
            return Self::default();
        }

        let mut terms = Vec::with_capacity(tokens.len());
        let mut ix = 0usize;
        while let Some(token) = tokens.get(ix) {
            if token.starts_with('?') {
                ix += 1;
                continue;
            }

            if let Some((field, value)) = parse_field_token(token) {
                if !value.is_empty() {
                    terms.push(HistoryQueryTerm::field(field, Arc::<str>::from(value)));
                    ix += 1;
                    continue;
                }

                if let Some(next) = tokens.get(ix + 1)
                    && !next.starts_with('?')
                    && !next.is_empty()
                {
                    terms.push(HistoryQueryTerm::field(
                        field,
                        Arc::<str>::from(next.as_str()),
                    ));
                    ix += 2;
                    continue;
                }

                ix += 1;
                continue;
            }

            if !token.is_empty() {
                terms.push(HistoryQueryTerm::plain(Arc::<str>::from(token.as_str())));
            }
            ix += 1;
        }

        Self { terms }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn terms(&self) -> &[HistoryQueryTerm] {
        &self.terms
    }

    pub fn into_terms(self) -> Vec<HistoryQueryTerm> {
        self.terms
    }

    pub fn has_plain_terms(&self) -> bool {
        self.terms
            .iter()
            .any(|term| matches!(term, HistoryQueryTerm::Plain(_)))
    }

    pub fn uses_field(&self, field: HistoryQueryField) -> bool {
        self.terms
            .iter()
            .any(|term| term.field_kind() == Some(field))
    }
}

impl fmt::Display for HistoryQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (ix, term) in self.terms.iter().enumerate() {
            if ix > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{term}")?;
        }
        Ok(())
    }
}

fn parse_field_token(token: &str) -> Option<(HistoryQueryField, &str)> {
    let (field, value) = token.split_once(':')?;
    let field = HistoryQueryField::parse(field.trim().to_ascii_lowercase().as_str())?;
    Some((field, value.trim()))
}

fn tokenize_history_query(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escape = false;

    for ch in raw.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        if ch == '\\' {
            escape = true;
            continue;
        }

        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
            }
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if escape {
        current.push('\\');
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn write_query_value(f: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    let needs_quotes = value.is_empty()
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\'' | ':' | '?'));
    if !needs_quotes {
        return f.write_str(value);
    }

    f.write_str("\"")?;
    for ch in value.chars() {
        if matches!(ch, '"' | '\\') {
            f.write_str("\\")?;
        }
        f.write_str(ch.encode_utf8(&mut [0; 4]))?;
    }
    f.write_str("\"")
}

#[cfg(test)]
mod tests {
    use super::{HistoryQuery, HistoryQueryField, HistoryQueryTerm};

    #[test]
    fn parse_empty_query() {
        assert!(HistoryQuery::parse("").is_empty());
        assert!(HistoryQuery::parse("   ").is_empty());
    }

    #[test]
    fn parse_plain_and_field_terms() {
        let query = HistoryQuery::parse("alice message:\"ship it\" sha:deadbeef");
        assert_eq!(
            query.terms(),
            &[
                HistoryQueryTerm::Plain("alice".into()),
                HistoryQueryTerm::Field {
                    field: HistoryQueryField::Message,
                    value: "ship it".into(),
                },
                HistoryQueryTerm::Field {
                    field: HistoryQueryField::Sha,
                    value: "deadbeef".into(),
                },
            ]
        );
    }

    #[test]
    fn parse_supports_split_field_value_tokens() {
        let query = HistoryQuery::parse("author: Alice file: src/main.rs");
        assert_eq!(
            query.terms(),
            &[
                HistoryQueryTerm::Field {
                    field: HistoryQueryField::Author,
                    value: "Alice".into(),
                },
                HistoryQueryTerm::Field {
                    field: HistoryQueryField::File,
                    value: "src/main.rs".into(),
                },
            ]
        );
    }

    #[test]
    fn parse_ignores_helper_tokens() {
        let query = HistoryQuery::parse("? file:src/lib.rs ?");
        assert_eq!(
            query.terms(),
            &[HistoryQueryTerm::Field {
                field: HistoryQueryField::File,
                value: "src/lib.rs".into(),
            }]
        );
    }

    #[test]
    fn parse_handles_quoted_values_and_escapes() {
        let query = HistoryQuery::parse("content:\"foo bar\\\"baz\" path:'dir/new name.txt'");
        assert_eq!(
            query.terms(),
            &[
                HistoryQueryTerm::Field {
                    field: HistoryQueryField::Content,
                    value: "foo bar\"baz".into(),
                },
                HistoryQueryTerm::Field {
                    field: HistoryQueryField::File,
                    value: "dir/new name.txt".into(),
                },
            ]
        );
    }

    #[test]
    fn display_round_trips_normalized_query() {
        let query = HistoryQuery::parse("author:\"Alice Example\" fix file:src/lib.rs");
        assert_eq!(
            query.to_string(),
            "author:\"Alice Example\" fix file:src/lib.rs"
        );
    }
}
