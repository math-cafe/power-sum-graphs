//! A restricted TOML reader and canonical writer, for the files posum
//! writes: `LKH.toml`, a run directory's `run.toml`, manifests and provenance
//! files (`doc/formats.md` sections 8 and 9).
//!
//! Accepted: `key = value` lines; `[name]` and `[[name]]` headers; strings
//! without escapes, non-negative integers, booleans, one-line arrays, inline
//! tables, and arrays spread over lines with one value per line followed by a
//! comma. Anything else is an error, not a guess. [`render`] writes a
//! document back in the canonical form of section 8.3, so that a file is
//! canonical exactly when it reads back to its own bytes.

use std::fmt;

/// A value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// A non-negative integer.
    Integer(u128),
    /// A string, without escapes.
    String(String),
    /// A boolean.
    Boolean(bool),
    /// An array.
    Array(Vec<Value>),
    /// An inline table, keys in order.
    Inline(Table),
}

/// Keys and values in file order.
pub type Table = Vec<(String, Value)>;

/// A `[name]` or `[[name]]` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// Its name.
    pub name: String,
    /// Whether it is an element of an array of tables, `[[name]]`.
    pub array: bool,
    /// Its keys.
    pub table: Table,
}

/// A document: its top-level keys, then its tables in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    /// Keys before the first header.
    pub top: Table,
    /// Each table, in file order.
    pub tables: Vec<Section>,
}

impl Document {
    /// The tables named `name`, in order.
    pub fn sections<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Section> + 'a {
        self.tables.iter().filter(move |s| s.name == name)
    }
}

/// A line the reader does not accept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// The 1-based line.
    pub line: usize,
    /// What is wrong.
    pub reason: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for ParseError {}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A cursor over one value's text.
struct Cursor<'a> {
    text: &'a str,
}

impl Cursor<'_> {
    fn skip_spaces(&mut self) {
        self.text = self.text.trim_start_matches(' ');
    }

    fn eat(&mut self, prefix: &str) -> bool {
        match self.text.strip_prefix(prefix) {
            Some(rest) => {
                self.text = rest;
                true
            }
            None => false,
        }
    }

    fn value(&mut self) -> Result<Value, &'static str> {
        self.skip_spaces();
        if self.eat("\"") {
            let end = self.text.find('"').ok_or("unterminated string")?;
            let (content, rest) = self.text.split_at(end);
            if content.contains('\\') {
                return Err("escapes are not supported");
            }
            self.text = &rest[1..];
            return Ok(Value::String(content.to_owned()));
        }
        if self.eat("true") {
            return Ok(Value::Boolean(true));
        }
        if self.eat("false") {
            return Ok(Value::Boolean(false));
        }
        if self.eat("[") {
            let mut items = Vec::new();
            self.skip_spaces();
            if self.eat("]") {
                return Ok(Value::Array(items));
            }
            loop {
                items.push(self.value()?);
                self.skip_spaces();
                if self.eat("]") {
                    return Ok(Value::Array(items));
                }
                if !self.eat(",") {
                    return Err("expected , or ] in an array");
                }
            }
        }
        if self.eat("{") {
            let mut table = Vec::new();
            loop {
                self.skip_spaces();
                let end = self
                    .text
                    .find(" =")
                    .ok_or("expected key = value in an inline table")?;
                let key = &self.text[..end];
                if !valid_key(key) {
                    return Err("invalid key");
                }
                self.text = &self.text[end + 2..];
                table.push((key.to_owned(), self.value()?));
                self.skip_spaces();
                if self.eat("}") {
                    return Ok(Value::Inline(table));
                }
                if !self.eat(",") {
                    return Err("expected , or } in an inline table");
                }
            }
        }
        let digits = self.text.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return Err("invalid value");
        }
        let (number, rest) = self.text.split_at(digits);
        self.text = rest;
        number
            .parse()
            .map(Value::Integer)
            .map_err(|_| "integer too large")
    }
}

/// Parses a document.
///
/// # Errors
///
/// On the first line outside the accepted subset.
pub fn parse(text: &str) -> Result<Document, ParseError> {
    let mut document = Document::default();
    let mut lines = text.lines().enumerate();
    while let Some((index, raw)) = lines.next() {
        let error = |reason| ParseError {
            line: index + 1,
            reason,
        };
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix("[[").and_then(|r| r.strip_suffix("]]")) {
            document.tables.push(Section {
                name: name.to_owned(),
                array: true,
                table: Vec::new(),
            });
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            document.tables.push(Section {
                name: name.to_owned(),
                array: false,
                table: Vec::new(),
            });
            continue;
        }
        let (key, rest) = line
            .split_once(" = ")
            .ok_or(error("expected key = value"))?;
        if !valid_key(key) {
            return Err(error("invalid key"));
        }
        let value = if rest == "[" {
            // An array over lines, one value per line followed by a comma.
            let mut items = Vec::new();
            loop {
                let (index, raw) = lines.next().ok_or(error("unterminated array"))?;
                let error = |reason| ParseError {
                    line: index + 1,
                    reason,
                };
                let item = raw.trim();
                if item == "]" {
                    break;
                }
                let item = item
                    .strip_suffix(',')
                    .ok_or(error("expected a comma after the value"))?;
                let mut cursor = Cursor { text: item };
                items.push(cursor.value().map_err(error)?);
                if !cursor.text.trim().is_empty() {
                    return Err(error("unexpected text after the value"));
                }
            }
            Value::Array(items)
        } else {
            let mut cursor = Cursor { text: rest };
            let value = cursor.value().map_err(error)?;
            if !cursor.text.trim().is_empty() {
                return Err(error("unexpected text after the value"));
            }
            value
        };
        let table = match document.tables.last_mut() {
            Some(section) => &mut section.table,
            None => &mut document.top,
        };
        table.push((key.to_owned(), value));
    }
    Ok(document)
}

fn render_value(value: &Value, out: &mut String) {
    match value {
        Value::Integer(i) => out.push_str(&i.to_string()),
        Value::String(s) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
        Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                render_value(item, out);
            }
            out.push(']');
        }
        Value::Inline(table) => {
            out.push_str("{ ");
            for (i, (key, value)) in table.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(key);
                out.push_str(" = ");
                render_value(value, out);
            }
            out.push_str(" }");
        }
    }
}

fn render_table(table: &Table, out: &mut String) {
    for (key, value) in table {
        out.push_str(key);
        out.push_str(" = ");
        // An array of inline tables goes one per line (section 8.3, rule 7).
        if let Value::Array(items) = value
            && !items.is_empty()
            && items.iter().all(|i| matches!(i, Value::Inline(_)))
        {
            out.push_str("[\n");
            for item in items {
                out.push_str("  ");
                render_value(item, out);
                out.push_str(",\n");
            }
            out.push_str("]\n");
            continue;
        }
        render_value(value, out);
        out.push('\n');
    }
}

/// The canonical text of a document, `doc/formats.md` section 8.3.
#[must_use]
pub fn render(document: &Document) -> String {
    let mut out = String::new();
    render_table(&document.top, &mut out);
    for section in &document.tables {
        out.push('\n');
        if section.array {
            out.push_str(&format!("[[{}]]\n", section.name));
        } else {
            out.push_str(&format!("[{}]\n", section.name));
        }
        render_table(&section.table, &mut out);
    }
    out
}

/// The value of `key` in `table`.
#[must_use]
pub fn get<'a>(table: &'a Table, key: &str) -> Option<&'a Value> {
    table.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// The string value of `key`.
#[must_use]
pub fn get_str<'a>(table: &'a Table, key: &str) -> Option<&'a str> {
    match get(table, key)? {
        Value::String(s) => Some(s),
        _ => None,
    }
}

/// The integer value of `key`, if it fits in `u64`.
#[must_use]
pub fn get_int(table: &Table, key: &str) -> Option<u64> {
    match get(table, key)? {
        Value::Integer(i) => u64::try_from(*i).ok(),
        _ => None,
    }
}

/// The boolean value of `key`.
#[must_use]
pub fn get_bool(table: &Table, key: &str) -> Option<bool> {
    match get(table, key)? {
        Value::Boolean(b) => Some(*b),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_subset() {
        let doc = parse(
            "k = 5\nname = \"x y\"\nflag = true\n\n[[segment]]\nseed = 2\n[[segment]]\nseed = 3\n",
        )
        .unwrap();
        assert_eq!(get_int(&doc.top, "k"), Some(5));
        assert_eq!(get_str(&doc.top, "name"), Some("x y"));
        assert_eq!(get_bool(&doc.top, "flag"), Some(true));
        assert_eq!(doc.sections("segment").count(), 2);
        assert_eq!(get_int(&doc.tables[1].table, "seed"), Some(3));
    }

    #[test]
    fn refuses_the_rest() {
        assert!(parse("k=5\n").is_err());
        assert!(parse("k = -5\n").is_err());
        assert!(parse("s = \"a\\\"b\"\n").is_err());
        assert!(parse("a b = 1\n").is_err());
        assert!(parse("a = 1 2\n").is_err());
        assert!(parse("a = [\n  1\n]\n").is_err());
    }

    #[test]
    fn manifests_round_trip() {
        let canonical = include_str!("../../../tests/vectors/canon.toml");
        assert_eq!(render(&parse(canonical).unwrap()), canonical);
        let island = "format = \"posum-bundle 1\"\nk = 5\nisland = [246877, 248831]\nclaims = [\"hamiltonian\"]\nwitnesses = [\n  { n = 246877, id = \"ab\", reference = true },\n  { n = 246878, id = \"cd\", reference = true },\n]\n\n[archive]\nplain = { file = \"x.tar\", url = \"https://e\", sha256 = \"ff\" }\n";
        assert_eq!(render(&parse(island).unwrap()), island);
        // Not canonical: reads, but renders differently.
        let loose = "k  = 5\n";
        assert!(parse(loose).is_err());
        let spaced = "k = 5\n\n\nn = 6\n";
        assert_ne!(render(&parse(spaced).unwrap()), spaced);
        let noncanon = include_str!("../../../tests/vectors/noncanon.toml");
        assert_eq!(
            render(&parse(noncanon).unwrap()),
            noncanon,
            "key order is checked by the schema"
        );
    }
}
