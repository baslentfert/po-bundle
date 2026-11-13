//! For `build.rs`: turns the `.po` files into static tables in the binary.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::po;

/// Reads `<dir>/<lang>/LC_MESSAGES/<crate>.po` for every language and
/// writes the tables that [`crate::include_translations!`] includes. A
/// relative `dir` is relative to the crate (`CARGO_MANIFEST_DIR`), like
/// Slint's `with_bundled_translations`, so both can share one folder.
///
/// Untranslated and fuzzy messages are left out (the source text is shown
/// instead). If `<dir>/<crate>.pot` exists, prints one cargo warning per
/// language with the number of messages still to translate.
///
/// Panics with a clear message on a broken `.po` file, so the build stops.
pub fn compile(dir: impl AsRef<Path>) {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("run from build.rs"));
    let domain = std::env::var("CARGO_PKG_NAME").expect("run from build.rs");
    let dir = manifest.join(dir);
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("run from build.rs")).join("po_bundle.rs");
    println!("cargo:rerun-if-changed={}", dir.display());
    let code = generate(&dir, &domain, |p| println!("cargo:rerun-if-changed={}", p.display()), |w| {
        println!("cargo:warning={w}");
    })
    .unwrap_or_else(|e| panic!("po-bundle: {e}"));
    std::fs::write(&out, code).unwrap_or_else(|e| panic!("po-bundle: cannot write {}: {e}", out.display()));
}

/// The `.po` files under `dir` for `domain`: (language code, path), sorted.
pub fn po_files(dir: &Path, domain: &str) -> Vec<(String, PathBuf)> {
    let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let p = e.path().join("LC_MESSAGES").join(format!("{domain}.po"));
            p.is_file().then(|| (e.file_name().to_string_lossy().into_owned(), p))
        })
        .collect();
    files.sort();
    files
}

/// The Rust source of a [`crate::Catalog`] for `domain`.
pub fn generate(dir: &Path, domain: &str, mut seen: impl FnMut(&Path), mut warn: impl FnMut(&str)) -> Result<String, String> {
    let template = dir.join(format!("{domain}.pot"));
    let wanted: Option<Vec<(String, String)>> = if template.is_file() {
        seen(&template);
        let t = std::fs::read_to_string(&template).map_err(|e| format!("{}: {e}", template.display()))?;
        let t = po::parse(&t).map_err(|e| format!("{}: {e}", template.display()))?;
        Some(t.entries.into_iter().filter(|e| !e.obsolete).map(|e| (e.context, e.id)).collect())
    } else {
        None
    };
    let mut code = String::new();
    let _ = write!(code, "::po_bundle::Catalog {{ domain: {domain:?}, languages: &[");
    for (lang, path) in po_files(dir, domain) {
        seen(&path);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = po::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut messages: Vec<&po::Entry> = file.entries.iter().filter(|e| !e.obsolete && !e.fuzzy && !e.text.is_empty()).collect();
        messages.sort_by(|a, b| a.key().cmp(&b.key()));
        messages.dedup_by(|a, b| a.key() == b.key());
        if let Some(w) = &wanted {
            let missing = w.iter().filter(|(c, i)| messages.binary_search_by(|m| m.key().cmp(&(c.as_str(), i.as_str()))).is_err()).count();
            if missing > 0 {
                warn(&format!("po-bundle: {lang}: {missing} of {} messages not translated yet ({})", w.len(), path.display()));
            }
        }
        let _ = write!(code, "({lang:?}, &[");
        for m in messages {
            let _ = write!(code, "::po_bundle::Message {{ context: {:?}, id: {:?}, text: {:?} }},", m.context, m.id, m.text);
        }
        code.push_str("]),");
    }
    code.push_str("] }");
    Ok(code)
}
