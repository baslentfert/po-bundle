//! The texts inside Slint's own widgets (`StandardButton`, the context menu
//! of `LineEdit`/`TextEdit`, the date and time pickers). Slint translates
//! them with `@tr` too, with the widget's name as context, from the same
//! bundled `.po` files. `po-bundle update --slint …` adds them, with the
//! translations below already filled in.
//!
//! Collected from the widgets of Slint 1.18 (`i-slint-compiler/widgets`).

/// (context, source text, Dutch, German).
pub const WIDGETS: &[(&str, &str, &str, &str)] = &[
    ("StandardButton", "OK", "OK", "OK"),
    ("StandardButton", "Cancel", "Annuleren", "Abbrechen"),
    ("StandardButton", "Apply", "Toepassen", "Übernehmen"),
    ("StandardButton", "Close", "Sluiten", "Schließen"),
    ("StandardButton", "Reset", "Herstellen", "Zurücksetzen"),
    ("StandardButton", "Help", "Help", "Hilfe"),
    ("StandardButton", "Yes", "Ja", "Ja"),
    ("StandardButton", "No", "Nee", "Nein"),
    ("StandardButton", "Abort", "Afbreken", "Abbrechen"),
    ("StandardButton", "Retry", "Opnieuw", "Wiederholen"),
    ("StandardButton", "Ignore", "Negeren", "Ignorieren"),
    ("LineEditBase", "Undo", "Ongedaan maken", "Rückgängig"),
    ("LineEditBase", "Redo", "Opnieuw uitvoeren", "Wiederholen"),
    ("LineEditBase", "Cut", "Knippen", "Ausschneiden"),
    ("LineEditBase", "Copy", "Kopiëren", "Kopieren"),
    ("LineEditBase", "Paste", "Plakken", "Einfügen"),
    ("LineEditBase", "Select All", "Alles selecteren", "Alles auswählen"),
    ("TextEditBase", "Cut", "Knippen", "Ausschneiden"),
    ("TextEditBase", "Copy", "Kopiëren", "Kopieren"),
    ("TextEditBase", "Paste", "Plakken", "Einfügen"),
    ("TextEditBase", "Select All", "Alles selecteren", "Alles auswählen"),
    ("DatePickerBase", "Enter date", "Datum invoeren", "Datum eingeben"),
    ("DatePickerBase", "Toggle selection mode", "Andere manier van kiezen", "Auswahlmodus wechseln"),
    ("DatePickerBase", "Previous month", "Vorige maand", "Vorheriger Monat"),
    ("DatePickerBase", "Next month", "Volgende maand", "Nächster Monat"),
    ("One-letter abbrev for Sunday", "S", "Z", "S"),
    ("One-letter abbrev for Monday", "M", "M", "M"),
    ("One-letter abbrev for Tuesday", "T", "D", "D"),
    ("One-letter abbrev for Wednesday", "W", "W", "M"),
    ("One-letter abbrev for Thursday", "T", "D", "D"),
    ("One-letter abbrev for Friday", "F", "V", "F"),
    ("One-letter abbrev for Saturday", "S", "Z", "S"),
    ("TimePickerBase", "hour", "uur", "Stunde"),
    ("TimePickerBase", "minute", "minuut", "Minute"),
    ("PeriodSelector", "AM or PM", "AM of PM", "AM oder PM"),
    ("Clock", "{} Hours or minutes of {}", "{} uren of minuten van {}", "{} Stunden oder Minuten von {}"),
];

/// Our translation of a widget text in `lang`, if we ship one.
pub fn translation(lang: &str, context: &str, id: &str) -> Option<&'static str> {
    let base = lang.split(['_', '-']).next().unwrap_or(lang);
    WIDGETS.iter().find(|(c, i, _, _)| *c == context && *i == id).and_then(|(_, _, nl, de)| match base {
        "nl" => Some(*nl),
        "de" => Some(*de),
        _ => None,
    })
}
