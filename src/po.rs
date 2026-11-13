//! Reading and writing gettext `.po` / `.pot` files.
//!
//! Only what po-bundle needs: `msgctxt`, `msgid`, `msgid_plural` (the
//! first form is used), `msgstr`, multi-line strings, comments, the
//! `fuzzy` flag and obsolete (`#~`) entries.

use std::fmt::Write as _;

/// One message in a `.po` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// `msgctxt`; empty when there is none.
    pub context: String,
    pub id: String,
    /// `msgstr` (or `msgstr[0]` for a plural); empty when not translated
    /// yet.
    pub text: String,
    /// `msgid_plural`; empty when the message has no plural.
    pub plural: String,
    /// `msgstr[1]`, `msgstr[2]`, … of a plural.
    pub plural_texts: Vec<String>,
    /// `#:` lines: where the message is used (`src/main.rs:12`).
    pub references: Vec<String>,
    /// `#.` lines: notes for translators.
    pub notes: Vec<String>,
    /// `#` lines written by translators; kept as they are.
    pub translator_comments: Vec<String>,
    /// Marked `#, fuzzy`: a guess that a translator still has to check.
    /// Not used at run time.
    pub fuzzy: bool,
    /// An obsolete entry (`#~`): no longer in the source, kept so the
    /// translation can come back.
    pub obsolete: bool,
}

impl Entry {
    pub fn key(&self) -> (&str, &str) {
        (&self.context, &self.id)
    }
}

/// A whole `.po` file: the header (the entry with an empty id) and the
/// messages.
#[derive(Debug, Clone, Default)]
pub struct File {
    pub header: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    None,
    Context,
    Id,
    Plural,
    Text,
    OtherForm(usize),
}

/// Reads a `.po` or `.pot` file.
pub fn parse(source: &str) -> Result<File, ParseError> {
    let mut file = File::default();
    let mut cur = Entry::default();
    let mut field = Field::None;
    let mut started = false;

    let finish = |cur: &mut Entry, started: &mut bool, file: &mut File| {
        if *started {
            let e = std::mem::take(cur);
            if e.id.is_empty() && e.context.is_empty() && !e.obsolete {
                file.header = e.text;
            } else {
                file.entries.push(e);
            }
        }
        *started = false;
    };

    for (n, raw) in source.lines().enumerate() {
        let line_no = n + 1;
        let mut line = raw.trim();
        if line.is_empty() {
            finish(&mut cur, &mut started, &mut file);
            field = Field::None;
            continue;
        }
        let mut obsolete = false;
        if let Some(rest) = line.strip_prefix("#~") {
            obsolete = true;
            line = rest.trim_start();
            if line.starts_with('|') {
                continue; // previous msgid of an obsolete entry
            }
        } else if let Some(rest) = line.strip_prefix('#') {
            // A comment starts a new entry if the previous one had a msgid
            // already (files without blank lines between entries).
            if field != Field::None && started {
                finish(&mut cur, &mut started, &mut file);
                field = Field::None;
            }
            if let Some(r) = rest.strip_prefix(':') {
                cur.references.extend(r.split_whitespace().map(String::from));
            } else if let Some(r) = rest.strip_prefix('.') {
                cur.notes.push(r.trim().to_string());
            } else if let Some(r) = rest.strip_prefix(',') {
                cur.fuzzy |= r.split(',').any(|f| f.trim() == "fuzzy");
            } else if rest.starts_with('|') {
                // previous msgid: not needed
            } else {
                cur.translator_comments.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
            }
            continue;
        }
        let err = |message: &str| ParseError { line: line_no, message: message.to_string() };
        let (keyword, rest) = match line.find(|c: char| c.is_whitespace()) {
            Some(i) if !line.starts_with('"') => (&line[..i], line[i..].trim_start()),
            _ => ("", line),
        };
        let target = match keyword {
            "" => field,
            "msgctxt" => Field::Context,
            "msgid" => Field::Id,
            "msgid_plural" => Field::Plural,
            "msgstr" | "msgstr[0]" => Field::Text,
            k if k.starts_with("msgstr[") => {
                let n: usize = k[7..].trim_end_matches(']').parse().map_err(|_| err("bad plural index"))?;
                Field::OtherForm(n)
            }
            k => return Err(err(&format!("unknown keyword `{k}`"))),
        };
        if keyword == "msgctxt" || (keyword == "msgid" && field != Field::Context) {
            // A new entry begins.
            if field != Field::None {
                finish(&mut cur, &mut started, &mut file);
            }
        }
        started = true;
        cur.obsolete |= obsolete;
        let text = unquote(rest).ok_or_else(|| err("expected a quoted string"))?;
        match target {
            Field::Context => cur.context.push_str(&text),
            Field::Id => cur.id.push_str(&text),
            Field::Text => cur.text.push_str(&text),
            Field::Plural => cur.plural.push_str(&text),
            Field::OtherForm(n) => {
                if cur.plural_texts.len() < n {
                    cur.plural_texts.resize(n, String::new());
                }
                cur.plural_texts[n - 1].push_str(&text);
            }
            Field::None => return Err(err("text outside an entry")),
        }
        field = target;
    }
    finish(&mut cur, &mut started, &mut file);
    Ok(file)
}

/// `"a\"b"` → `a"b`.
fn unquote(s: &str) -> Option<String> {
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'a' => out.push('\u{7}'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    Some(out)
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Writes `keyword "value"`, split after each `\n` like gettext does.
fn write_field(out: &mut String, prefix: &str, keyword: &str, value: &str) {
    let parts: Vec<&str> = value.split_inclusive('\n').collect();
    if parts.len() > 1 {
        let _ = writeln!(out, "{prefix}{keyword} \"\"");
        for p in parts {
            let _ = writeln!(out, "{prefix}{}", quote(p));
        }
    } else {
        let _ = writeln!(out, "{prefix}{keyword} {}", quote(value));
    }
}

/// Writes a `.po` file.
pub fn write(file: &File) -> String {
    let mut out = String::new();
    write_field(&mut out, "", "msgid", "");
    write_field(&mut out, "", "msgstr", &file.header);
    for e in &file.entries {
        out.push('\n');
        for c in &e.translator_comments {
            let _ = writeln!(out, "# {c}");
        }
        for n in &e.notes {
            let _ = writeln!(out, "#. {n}");
        }
        if !e.obsolete && !e.references.is_empty() {
            let _ = writeln!(out, "#: {}", e.references.join(" "));
        }
        if e.fuzzy {
            out.push_str("#, fuzzy\n");
        }
        let prefix = if e.obsolete { "#~ " } else { "" };
        if !e.context.is_empty() {
            write_field(&mut out, prefix, "msgctxt", &e.context);
        }
        write_field(&mut out, prefix, "msgid", &e.id);
        if e.plural.is_empty() {
            write_field(&mut out, prefix, "msgstr", &e.text);
        } else {
            write_field(&mut out, prefix, "msgid_plural", &e.plural);
            write_field(&mut out, prefix, "msgstr[0]", &e.text);
            let forms = plural_count(&file.header).max(e.plural_texts.len() + 1);
            for i in 1..forms {
                write_field(&mut out, prefix, &format!("msgstr[{i}]"), e.plural_texts.get(i - 1).map_or("", String::as_str));
            }
        }
    }
    out
}

/// `nplurals` from the `Plural-Forms` header; 2 when absent.
pub fn plural_count(header: &str) -> usize {
    header
        .lines()
        .find_map(|l| l.strip_prefix("Plural-Forms:"))
        .and_then(|v| v.split(';').find_map(|p| p.trim().strip_prefix("nplurals=")))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(2)
}

/// The gettext `Plural-Forms` header for common languages (from the GNU
/// gettext manual), for new `.po` files.
pub fn plural_forms(lang: &str) -> Option<&'static str> {
    let base = lang.split(['_', '-']).next().unwrap_or(lang);
    Some(match base {
        "ja" | "ko" | "zh" | "vi" | "th" | "id" | "tr" => "nplurals=1; plural=0;",
        "fr" | "pt" if lang != "pt_PT" => "nplurals=2; plural=(n > 1);",
        "en" | "nl" | "de" | "fy" | "da" | "sv" | "nb" | "no" | "fi" | "it" | "es" | "el" | "hu" | "et" | "pt" => "nplurals=2; plural=(n != 1);",
        "ru" | "uk" | "be" | "sr" | "hr" | "bs" => {
            "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);"
        }
        "pl" => "nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        "cs" | "sk" => "nplurals=3; plural=(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2;",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOORBEELD: &str = r#"# Dutch translation
msgid ""
msgstr ""
"Language: nl\n"
"Content-Type: text/plain; charset=UTF-8\n"

#. Shown on the main screen
#: src/main.rs:10
msgid "Share this screen"
msgstr "Dit scherm delen"

#, fuzzy
msgctxt "menu"
msgid "Open"
msgstr "Openen"

msgid ""
"Two\n"
"lines"
msgstr "Twee\nregels"

msgid "{n} file"
msgid_plural "{n} files"
msgstr[0] "{n} bestand"
msgstr[1] "{n} bestanden"

#~ msgid "Old"
#~ msgstr "Oud"
"#;

    #[test]
    fn reads_and_writes_back() {
        let f = parse(VOORBEELD).unwrap();
        assert!(f.header.contains("Language: nl"));
        assert_eq!(f.entries.len(), 5);
        let e = &f.entries[0];
        assert_eq!((e.id.as_str(), e.text.as_str()), ("Share this screen", "Dit scherm delen"));
        assert_eq!(e.references, ["src/main.rs:10"]);
        assert_eq!(e.notes, ["Shown on the main screen"]);
        assert!(f.entries[1].fuzzy);
        assert_eq!(f.entries[1].context, "menu");
        assert_eq!(f.entries[2].id, "Two\nlines");
        assert_eq!(f.entries[2].text, "Twee\nregels");
        assert_eq!(f.entries[3].plural, "{n} files");
        assert_eq!(f.entries[3].text, "{n} bestand");
        assert_eq!(f.entries[3].plural_texts, ["{n} bestanden"]);
        assert!(f.entries[4].obsolete);
        assert_eq!(f.entries[4].id, "Old");
        // Writing and reading again gives the same.
        let again = parse(&write(&f)).unwrap();
        assert_eq!(again.entries, f.entries);
        assert_eq!(again.header, f.header);
    }

    #[test]
    fn quotes_and_escapes() {
        assert_eq!(unquote(r#""a \"b\" \\ c\n""#).unwrap(), "a \"b\" \\ c\n");
        assert_eq!(quote("a \"b\"\n"), r#""a \"b\"\n""#);
    }
}
