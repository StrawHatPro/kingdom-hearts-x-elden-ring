//! (LOGPRIV, the user: "logs should hold NO personal info") Every line is passed through [`line`] before it is
//! written, so a log can be shared as it is:
//! - the user's profile folder (`%USERPROFILE%`, e.g. `C:\Users\Name`) becomes `%USERPROFILE%`;
//! - the mod's folder becomes `<mod>`, the game's `<game>` ([`folder`]);
//! - the Windows user name and the PC's name, as whole words anywhere, become `<user>` / `<pc>`;
//! - any other `\Users\<name>\` path part becomes `\Users\<user>\`;
//! - a 64-bit Steam ID (17 digits starting 7656119, the save folder's name) and the account number after Steam's
//!   `userdata\` become `<steam id>`.
//! Also read by the loader (`kh-er-loader`, `#[path]`): one rule for both logs.

use std::borrow::Cow;
use std::sync::{Mutex, OnceLock};

const USER: &str = "<user>";
const STEAM: &str = "<steam id>";

/// Words taken out wherever they stand as whole words (case-insensitive), with what they become.
static WORDS: Mutex<Vec<(String, &'static str)>> = Mutex::new(Vec::new());
/// Folders taken out wherever they start a path part (the mod's, the game's).
static FOLDERS: Mutex<Vec<(String, &'static str)>> = Mutex::new(Vec::new());
/// Words never hidden (a user or PC named so would otherwise blank out the mod's own words).
const NOT_PERSONAL: [&str; 4] = ["sora", "donald", "goofy", "unnamed"];

/// Takes `word` out of every later line as `as_` (the user name, the PC's name).
pub fn hide(word: &str, as_: &'static str) {
    let word = word.trim();
    if word.is_empty() || NOT_PERSONAL.contains(&word.to_ascii_lowercase().as_str()) {
        return;
    }
    if let Ok(mut w) = WORDS.lock() {
        if !w.iter().any(|(k, _)| k.eq_ignore_ascii_case(word)) {
            w.push((word.to_string(), as_));
        }
    }
}

/// Takes the folder `path` out of every later line as `as_` (`<mod>`, `<game>`).
pub fn folder(path: &std::path::Path, as_: &'static str) {
    let p = path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    if p.len() > 3 {
        if let Ok(mut f) = FOLDERS.lock() {
            if !f.iter().any(|(k, _)| k.eq_ignore_ascii_case(&p)) {
                f.push((p, as_));
            }
        }
    }
}

/// The Windows user name and the PC's name, once.
fn env_words() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        for (var, as_) in [("USERNAME", USER), ("COMPUTERNAME", "<pc>")] {
            if let Ok(v) = std::env::var(var) {
                hide(&v, as_);
            }
        }
    });
}

/// The profile folder as Windows names it (without a trailing separator), if it is long enough to be one.
fn profile() -> Option<&'static str> {
    static PROFILE: OnceLock<Option<String>> = OnceLock::new();
    PROFILE
        .get_or_init(|| std::env::var("USERPROFILE").ok().map(|p| p.trim_end_matches(['\\', '/']).to_string()).filter(|p| p.len() > 3))
        .as_deref()
}

/// `text` with the user name and Steam ID taken out (borrowed when there is nothing to take out).
pub fn line(text: &str) -> Cow<'_, str> {
    env_words();
    let folders = FOLDERS.lock().map(|f| f.clone()).unwrap_or_default();
    let words = WORDS.lock().map(|w| w.clone()).unwrap_or_default();
    line_with(text, profile(), &folders, &words)
}

fn line_with<'a>(text: &'a str, profile: Option<&str>, folders: &[(String, &str)], words: &[(String, &str)]) -> Cow<'a, str> {
    let mut out = Cow::Borrowed(text);
    // (the longest folder first: the mod may sit inside the profile, the game's folder inside the mod's)
    let mut folders: Vec<&(String, &str)> = folders.iter().collect();
    folders.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    for (p, as_) in folders {
        out = replace_ci(out, p, as_);
        out = replace_ci(out, &p.replace('\\', "/"), as_);
    }
    if let Some(p) = profile {
        out = replace_ci(out, p, "%USERPROFILE%");
        out = replace_ci(out, &p.replace('\\', "/"), "%USERPROFILE%");
    }
    // (a user name may hold spaces and apostrophes: the part runs to the next separator; more taken out, never less)
    out = after_marker(out, &["\\users\\", "/users/"], USER, |c| matches!(c, '\\' | '/' | '"' | '(' | ')' | ',' | ';' | '<' | '>' | '|' | '\n'));
    out = after_marker(out, &["\\userdata\\", "/userdata/"], STEAM, |c| !c.is_ascii_digit());
    let mut words: Vec<&(String, &str)> = words.iter().collect();
    words.sort_by_key(|(w, _)| std::cmp::Reverse(w.len()));
    for (w, as_) in words {
        out = whole_words(out, w, as_);
    }
    steam_ids(out)
}

/// Every case-insensitive whole-word `word` (not inside a longer run of letters and digits) replaced by `to`.
fn whole_words<'a>(text: Cow<'a, str>, word: &str, to: &str) -> Cow<'a, str> {
    if word.is_empty() {
        return text;
    }
    let (lower, needle) = (text.to_ascii_lowercase(), word.to_ascii_lowercase());
    if !lower.contains(&needle) {
        return text;
    }
    let alnum = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    let mut out = String::with_capacity(text.len());
    let (mut at, mut from) = (0, 0);
    while let Some(k) = lower[from..].find(&needle) {
        let (start, end) = (from + k, from + k + needle.len());
        if !alnum(text[..start].chars().next_back()) && !alnum(text[end..].chars().next()) {
            out.push_str(&text[at..start]);
            out.push_str(to);
            at = end;
        }
        from = end;
    }
    if at == 0 {
        return text;
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

/// Every case-insensitive `from` replaced by `to` (ASCII case only, as paths are compared).
fn replace_ci<'a>(text: Cow<'a, str>, from: &str, to: &str) -> Cow<'a, str> {
    if from.is_empty() || !text.to_ascii_lowercase().contains(&from.to_ascii_lowercase()) {
        return text;
    }
    let (lower, needle) = (text.to_ascii_lowercase(), from.to_ascii_lowercase());
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(k) = lower[at..].find(&needle) {
        out.push_str(&text[at..at + k]);
        out.push_str(to);
        at += k + needle.len();
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

/// The path part after each marker (up to `end`) replaced by `to`; `<...>` placeholders already there are left.
fn after_marker<'a>(text: Cow<'a, str>, markers: &[&str], to: &str, end: impl Fn(char) -> bool) -> Cow<'a, str> {
    let lower = text.to_ascii_lowercase();
    if !markers.iter().any(|m| lower.contains(m)) {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some((k, m)) = markers.iter().filter_map(|m| lower[at..].find(m).map(|k| (k, m.len()))).min() {
        let start = at + k + m;
        out.push_str(&text[at..start]);
        let len = text[start..].find(|c: char| end(c)).unwrap_or(text.len() - start);
        let part = &text[start..start + len];
        out.push_str(if part.is_empty() || part.starts_with('<') { part } else { to });
        at = start + len;
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

/// 17-digit runs starting 7656119 (64-bit Steam IDs) replaced.
fn steam_ids(text: Cow<'_, str>) -> Cow<'_, str> {
    if !text.contains("7656119") {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let mut changed = false;
    let mut flush = |run: &mut String, out: &mut String| {
        let id = run.len() == 17 && run.starts_with("7656119");
        changed |= id;
        out.push_str(if id { STEAM } else { run });
        run.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit() {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    if changed { Cow::Owned(out) } else { text }
}

#[cfg(test)]
mod tests {
    #[test]
    fn steams_account_number_never_reaches_a_log() {
        // (the Steam screenshots folder: userdata\<account number>\760; the number here is made up)
        assert_eq!(
            super::line_with(r"C:\Program Files (x86)\Steam\userdata\12345678\760\remote\1245620\screenshots", None, &[], &[]),
            r"C:\Program Files (x86)\Steam\userdata\<steam id>\760\remote\1245620\screenshots"
        );
    }

    #[test]
    fn the_mods_own_names_are_never_hidden() {
        super::hide("Sora", "<user>");
        super::hide("  ", "<user>");
        assert!(super::WORDS.lock().unwrap().iter().all(|(w, _)| !w.eq_ignore_ascii_case("sora") && !w.trim().is_empty()));
    }
}
