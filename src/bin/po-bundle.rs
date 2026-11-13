//! `po-bundle update`: collects the strings to translate and updates the
//! `.pot` and every `.po` file. `po-bundle check`: fails when something is
//! not translated yet (for CI).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use po_bundle::po::{self, Entry, File};

const HELP: &str = "po-bundle — keep gettext .po files in step with your code

USAGE:
    po-bundle update [OPTIONS]    collect strings, update the .pot and all .po files
    po-bundle check  [OPTIONS]    exit with an error if anything is untranslated

OPTIONS (paths relative to the crate):
    --crate DIR        the crate (default: current directory)
    --dir DIR          translations folder (default: translations)
    --src DIR          Rust sources to scan for tr!(…) (default: src; repeatable)
    --slint DIR        .slint files to scan for @tr(…) (repeatable); also adds
                       the texts of Slint's own widgets (OK, Cancel, Copy, …),
                       with Dutch and German already translated
    --no-slint-widgets leave Slint's own widget texts out
    --add LANG         start a new language, e.g. --add fr (repeatable)
    --domain NAME      gettext domain (default: the package name in Cargo.toml)

Layout: <dir>/<lang>/LC_MESSAGES/<domain>.po, as Slint's bundled
translations expect, plus <dir>/<domain>.pot.
";

struct Options {
    krat: PathBuf,
    dir: PathBuf,
    src: Vec<PathBuf>,
    slint: Vec<PathBuf>,
    add: Vec<String>,
    domain: Option<String>,
    slint_widgets: bool,
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        eprint!("{HELP}");
        return ExitCode::FAILURE;
    };
    let mut o = Options { krat: ".".into(), dir: "translations".into(), src: Vec::new(), slint: Vec::new(), add: Vec::new(), domain: None, slint_widgets: true };
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        let r: Result<(), String> = (|| {
            match a.as_str() {
                "--crate" => o.krat = value()?.into(),
                "--dir" => o.dir = value()?.into(),
                "--src" => o.src.push(value()?.into()),
                "--slint" => o.slint.push(value()?.into()),
                "--add" => o.add.push(value()?),
                "--domain" => o.domain = Some(value()?),
                "--no-slint-widgets" => o.slint_widgets = false,
                "-h" | "--help" => {
                    print!("{HELP}");
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown option {a}")),
            }
            Ok(())
        })();
        if let Err(e) = r {
            eprintln!("po-bundle: {e}\n\n{HELP}");
            return ExitCode::FAILURE;
        }
    }
    if o.src.is_empty() {
        o.src.push("src".into());
    }
    let result = match command.as_str() {
        "update" => update(&o),
        "check" => check(&o),
        "-h" | "--help" | "help" => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        c => Err(format!("unknown command {c}")),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("po-bundle: {e}");
            ExitCode::FAILURE
        }
    }
}

fn domain(o: &Options) -> Result<String, String> {
    if let Some(d) = &o.domain {
        return Ok(d.clone());
    }
    let toml = o.krat.join("Cargo.toml");
    let text = std::fs::read_to_string(&toml).map_err(|e| format!("{}: {e} (use --domain)", toml.display()))?;
    let mut in_package = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_package = l == "[package]";
        } else if in_package && let Some(v) = l.strip_prefix("name") {
            let v = v.trim_start().strip_prefix('=').map(str::trim).unwrap_or("");
            return Ok(v.trim_matches('"').to_string());
        }
    }
    Err(format!("no package name in {} (use --domain)", toml.display()))
}

/// All strings to translate, in order of appearance.
fn collect(o: &Options) -> Result<Vec<Entry>, String> {
    let mut found: BTreeMap<(String, String), Entry> = BTreeMap::new();
    let mut order: Vec<(String, String)> = Vec::new();
    let mut add = |context: String, id: String, plural: String, reference: String, notes: Vec<String>| {
        let key = (context.clone(), id.clone());
        let e = found.entry(key.clone()).or_insert_with(|| {
            order.push(key);
            Entry { context, id, ..Default::default() }
        });
        if !plural.is_empty() {
            e.plural = plural;
        }
        e.references.push(reference);
        for n in notes {
            if !e.notes.contains(&n) {
                e.notes.push(n);
            }
        }
    };
    for src in &o.src {
        let root = o.krat.join(src);
        let mut files = Vec::new();
        walk(&root, "rs", &mut files);
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let rel = f.strip_prefix(&o.krat).unwrap_or(&f).display().to_string();
            for (context, id, line) in scan_rust(&text) {
                add(context, id, String::new(), format!("{rel}:{line}"), Vec::new());
            }
        }
    }
    for dir in &o.slint {
        let root = o.krat.join(dir);
        let mut files = Vec::new();
        walk(&root, "slint", &mut files);
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let rel = f.strip_prefix(&o.krat).unwrap_or(&f).display().to_string();
            for (context, id, plural, line) in scan_slint(&text) {
                add(context, id, plural, format!("{rel}:{line}"), Vec::new());
            }
        }
    }
    if o.slint_widgets && !o.slint.is_empty() {
        for (context, id, _, _) in po_bundle::slint_widgets::WIDGETS {
            add(context.to_string(), id.to_string(), String::new(), "slint:std-widgets".into(), vec!["A text in Slint's own widgets".into()]);
        }
    }
    Ok(order.into_iter().map(|k| found.remove(&k).unwrap()).collect())
}

fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

/// Finds `tr!("…")` and `tr!("ctx" => "…")` in Rust source: (context, id,
/// line).
fn scan_rust(text: &str) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(pos) = text[i..].find("tr!") {
        let start = i + pos;
        i = start + 3;
        // Not part of a longer name (e.g. `attr!`).
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        let mut j = skip_ws(text, i);
        if !matches!(bytes.get(j), Some(b'(' | b'[' | b'{')) {
            continue;
        }
        j = skip_ws(text, j + 1);
        let Some((first, after)) = string_literal(text, j) else { continue };
        let k = skip_ws(text, after);
        let (context, id) = if text[k..].starts_with("=>") {
            let m = skip_ws(text, k + 2);
            let Some((second, _)) = string_literal(text, m) else { continue };
            (first, second)
        } else {
            (String::new(), first)
        };
        let line = text[..start].matches('\n').count() + 1;
        out.push((context, id, line));
    }
    out
}

/// Finds `@tr("…")` and `@tr("ctx" => "…")` in a `.slint` file, the way
/// Slint does: without a context, the context is the name of the enclosing
/// component. For plurals (`"one" | "many" % n`) the first form is the id.
fn scan_slint(text: &str) -> Vec<(String, String, String, usize)> {
    // Where each component starts, with its name.
    let mut components: Vec<(usize, String)> = Vec::new();
    let mut i = 0;
    while let Some(pos) = text[i..].find("component") {
        let at = i + pos;
        i = at + 9;
        let before_ok = at == 0 || !text.as_bytes()[at - 1].is_ascii_alphanumeric();
        let rest = &text[i..];
        if before_ok && rest.starts_with(char::is_whitespace) {
            let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
            if !name.is_empty() {
                components.push((at, name));
            }
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = text[i..].find("@tr") {
        let start = i + pos;
        i = start + 3;
        let mut j = skip_ws(text, i);
        if text.as_bytes().get(j) != Some(&b'(') {
            continue;
        }
        j = skip_ws(text, j + 1);
        let Some((first, after)) = string_literal(text, j) else { continue };
        let k = skip_ws(text, after);
        let (context, id, after_id) = if text[k..].starts_with("=>") {
            let m = skip_ws(text, k + 2);
            let Some((second, after)) = string_literal(text, m) else { continue };
            (first, second, after)
        } else {
            let component = components.iter().rev().find(|(at, _)| *at < start).map(|(_, n)| n.clone()).unwrap_or_default();
            (component, first, after)
        };
        // Plural: "one" | "many" % n
        let p = skip_ws(text, after_id);
        let plural = if text[p..].starts_with('|') {
            string_literal(text, skip_ws(text, p + 1)).map(|(s, _)| s).unwrap_or_default()
        } else {
            String::new()
        };
        let line = text[..start].matches('\n').count() + 1;
        out.push((context, id, plural, line));
    }
    out
}

fn skip_ws(text: &str, mut i: usize) -> usize {
    let b = text.as_bytes();
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// A Rust string literal at `i` ("…" or r"…" / r#"…"#): its value and the
/// index after it.
fn string_literal(text: &str, i: usize) -> Option<(String, usize)> {
    let b = text.as_bytes();
    if b.get(i) == Some(&b'r') {
        let mut j = i + 1;
        let mut hashes = 0;
        while b.get(j) == Some(&b'#') {
            hashes += 1;
            j += 1;
        }
        if b.get(j) != Some(&b'"') {
            return None;
        }
        let end_marker = format!("\"{}", "#".repeat(hashes));
        let end = text[j + 1..].find(&end_marker)? + j + 1;
        return Some((text[j + 1..end].to_string(), end + end_marker.len()));
    }
    if b.get(i) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = text[i + 1..].char_indices();
    while let Some((n, c)) = chars.next() {
        match c {
            '"' => return Some((out, i + 1 + n + 1)),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '0' => out.push('\0'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                '\'' => out.push('\''),
                '\n' => {
                    // Line continuation: skip the following whitespace.
                    while let Some((_, w)) = chars.clone().next() {
                        if !w.is_whitespace() {
                            break;
                        }
                        chars.next();
                    }
                }
                'u' => {
                    let rest: String = chars.by_ref().map(|(_, c)| c).take_while(|c| *c != '}').collect();
                    let hex = rest.trim_start_matches('{');
                    out.push(char::from_u32(u32::from_str_radix(hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn header(lang: Option<&str>) -> String {
    let mut h = String::from("Content-Type: text/plain; charset=UTF-8\nContent-Transfer-Encoding: 8bit\n");
    if let Some(l) = lang {
        h.push_str(&format!("Language: {l}\n"));
        if let Some(p) = po::plural_forms(l) {
            h.push_str(&format!("Plural-Forms: {p}\n"));
        }
    }
    h.push_str("X-Generator: po-bundle\n");
    h
}

/// Merges the found strings into an existing `.po` file. Returns (added,
/// made obsolete, untranslated).
fn merge(lang: &str, old: &File, found: &[Entry]) -> (File, usize, usize, usize) {
    let mut out = File { header: old.header.clone(), entries: Vec::new() };
    let (mut added, mut untranslated) = (0, 0);
    for f in found {
        let previous = old.entries.iter().find(|e| e.key() == f.key());
        let mut e = f.clone();
        if let Some(p) = previous {
            e.text = p.text.clone();
            e.plural_texts = p.plural_texts.clone();
            e.fuzzy = p.fuzzy;
            e.translator_comments = p.translator_comments.clone();
        } else {
            added += 1;
        }
        // Slint's own widgets: we ship some translations.
        if e.text.is_empty()
            && let Some(t) = po_bundle::slint_widgets::translation(lang, &e.context, &e.id)
        {
            e.text = t.to_string();
        }
        if e.text.is_empty() || e.fuzzy {
            untranslated += 1;
        }
        out.entries.push(e);
    }
    let mut obsolete = 0;
    for e in &old.entries {
        if found.iter().any(|f| f.key() == e.key()) {
            continue;
        }
        if !e.text.is_empty() {
            // Keep the translation for when the string comes back.
            let mut o = e.clone();
            if !o.obsolete {
                obsolete += 1;
            }
            o.obsolete = true;
            o.references.clear();
            out.entries.push(o);
        } else if !e.obsolete {
            obsolete += 1;
        }
    }
    (out, added, obsolete, untranslated)
}

fn update(o: &Options) -> Result<bool, String> {
    let domain = domain(o)?;
    let dir = o.krat.join(&o.dir);
    let found = collect(o)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let pot = File { header: header(None), entries: found.clone() };
    let pot_path = dir.join(format!("{domain}.pot"));
    std::fs::write(&pot_path, po::write(&pot)).map_err(|e| format!("{}: {e}", pot_path.display()))?;
    println!("{}: {} messages", pot_path.display(), found.len());
    for lang in &o.add {
        let p = dir.join(lang).join("LC_MESSAGES").join(format!("{domain}.po"));
        if !p.exists() {
            std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&p, po::write(&File { header: header(Some(lang)), entries: Vec::new() })).map_err(|e| e.to_string())?;
        }
    }
    for (lang, path) in po_bundle::build::po_files(&dir, &domain) {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let old = po::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let (new, added, obsolete, untranslated) = merge(&lang, &old, &found);
        std::fs::write(&path, po::write(&new)).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{lang}: {added} new, {obsolete} no longer used, {untranslated} to translate");
    }
    Ok(true)
}

fn check(o: &Options) -> Result<bool, String> {
    let domain = domain(o)?;
    let dir = o.krat.join(&o.dir);
    let found = collect(o)?;
    let mut ok = true;
    for (lang, path) in po_bundle::build::po_files(&dir, &domain) {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = po::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let missing: Vec<&Entry> = found
            .iter()
            .filter(|f| !file.entries.iter().any(|e| e.key() == f.key() && !e.obsolete && !e.fuzzy && !e.text.is_empty()))
            .collect();
        if !missing.is_empty() {
            ok = false;
            println!("{lang}: {} to translate", missing.len());
            for m in missing.iter().take(10) {
                println!("    {:?}", m.id);
            }
        }
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tr_calls() {
        let src = "let a = tr!(\"Open\");\nlet b = po_bundle::tr!(\"menu\" => \"Close\");\nlet c = attr!(\"no\");\nlet d = tr!(\"Hi, {name}\", name = x);\nlet e = tr!(r#\"say \"hi\"\"#);";
        let found = scan_rust(src);
        assert_eq!(
            found,
            vec![
                (String::new(), "Open".to_string(), 1),
                ("menu".to_string(), "Close".to_string(), 2),
                (String::new(), "Hi, {name}".to_string(), 4),
                (String::new(), "say \"hi\"".to_string(), 5),
            ]
        );
    }

    #[test]
    fn finds_slint_tr_with_component_context() {
        let src = "export component AppWindow inherits Window {\n    Text { text: @tr(\"Hello\"); }\n    Text { text: @tr(\"menu\" => \"Open\"); }\n}\ncomponent Other { Text { text: @tr(\"{n} file\" | \"{n} files\" % 2); } }";
        assert_eq!(
            scan_slint(src),
            vec![
                ("AppWindow".to_string(), "Hello".to_string(), String::new(), 2),
                ("menu".to_string(), "Open".to_string(), String::new(), 3),
                ("Other".to_string(), "{n} file".to_string(), "{n} files".to_string(), 5),
            ]
        );
    }

    #[test]
    fn merge_keeps_adds_and_obsoletes() {
        let old = po::parse("msgid \"\"\nmsgstr \"Language: nl\\n\"\n\nmsgid \"Open\"\nmsgstr \"Openen\"\n\nmsgid \"Gone\"\nmsgstr \"Weg\"\n").unwrap();
        let found = vec![
            Entry { id: "Open".into(), ..Default::default() },
            Entry { id: "New".into(), ..Default::default() },
        ];
        let (new, added, obsolete, untranslated) = merge("nl", &old, &found);
        assert_eq!((added, obsolete, untranslated), (1, 1, 1));
        assert_eq!(new.entries[0].text, "Openen");
        assert!(new.entries[2].obsolete && new.entries[2].id == "Gone");
    }
}
