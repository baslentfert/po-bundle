//! A Slint app with po-bundle: one .po file per language for both the
//! .slint file (`@tr`) and the Rust code (`tr!`).
//!
//! `cargo run` and pick a language at the top right. `cargo run -- --cycle`
//! switches language every two seconds by itself.

use std::rc::Rc;

use po_bundle::tr;
use slint::{ModelRc, SharedString, StandardListViewItem, VecModel};

po_bundle::include_translations!();
slint::include_modules!();

/// Everything Rust hands to the UI. Texts in the .slint file follow a
/// language switch by themselves; these are set again on every switch.
fn set_texts(w: &AppWindow, languages: &[(&'static str, &'static str)]) {
    w.set_greeting(tr!("Hello, {name}!", name = "Ada").as_ref().into());

    // A list made in Rust is a model: fill it again too. "Automatic" first,
    // then each language in its own name.
    let mut names: Vec<SharedString> = vec![tr!("Automatic").into()];
    names.extend(languages.iter().map(|(_, name)| SharedString::from(*name)));
    w.set_languages(ModelRc::from(Rc::new(VecModel::from(names))));

    // Table rows from Rust, with translated cells.
    let row = |name: &str, status: &str| -> ModelRc<StandardListViewItem> {
        ModelRc::from(Rc::new(VecModel::from(vec![StandardListViewItem::from(name), StandardListViewItem::from(status)])))
    };
    let rows = vec![
        row("my-pc", tr!("Connected")),
        row("laptop", tr!("Not found")),
        row("office", &tr!("Last seen {days} days ago", days = 3)),
    ];
    w.set_table_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let languages = po_bundle::languages!();

    // On a language switch, set the Rust texts again (on the UI thread).
    let w = window.as_weak();
    let langs = languages.clone();
    po_bundle::on_change(move |code| {
        println!("language: {code}");
        let (w, langs) = (w.clone(), langs.clone());
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = w.upgrade() {
                set_texts(&w, &langs);
            }
        });
    });
    po_bundle::set_language(None); // follow the OS
    set_texts(&window, &languages);

    let langs = languages.clone();
    window.on_language_chosen(move |i| {
        let code = if i == 0 { None } else { langs.get(i as usize - 1).map(|(c, _)| *c) };
        po_bundle::set_language(code);
    });
    window.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    // `--tab N`: start on tab N (for screenshots).
    let args: Vec<String> = std::env::args().collect();
    if let Some(n) = args.iter().position(|a| a == "--tab").and_then(|i| args.get(i + 1)).and_then(|n| n.parse().ok()) {
        window.set_tab(n);
    }

    // `--cycle`: switch language every 2 s (to watch everything follow).
    let cycle = slint::Timer::default();
    if std::env::args().any(|a| a == "--cycle") {
        let (w, codes) = (window.as_weak(), languages.clone());
        let mut n = 0;
        cycle.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(2), move || {
            n = (n + 1) % (codes.len() + 1);
            if let Some(w) = w.upgrade() {
                w.set_language(n as i32);
            }
            po_bundle::set_language(if n == 0 { None } else { Some(codes[n - 1].0) });
        });
    }
    window.run()
}
