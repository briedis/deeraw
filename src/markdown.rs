//! Basic markdown in text objects: `**bold**`, `*italic*` (or `_italic_`), `~~strikethrough~~`
//! and `` `code` `` within a line; `#`, `##` and `###` headings and `-`, `*` and `1.` list items
//! at the start of one. The text is stored as typed; this only decides how it's laid out.

use std::ops::Range;

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, text::{LayoutJob, TextFormat}};

/// Name of the bold font family (`install_fonts`).
const BOLD: &str = "bold";
/// Ubuntu Bold, the bold weight of egui's default font (Ubuntu Light), under the same licence
/// (`assets/fonts/UFL.txt`).
const BOLD_TTF: &[u8] = include_bytes!("../assets/fonts/Ubuntu-Bold.ttf");
/// Font size of `#`, `##` and `###` headings, per pixel of the text's font size.
const HEADING_SCALE: [f32; 3] = [2.0, 1.5, 1.25];
/// Indent of list items, per pixel of font size.
const LIST_INDENT: f32 = 0.6;
/// Opacity of the markup characters while editing.
const MARKER_OPACITY: f32 = 0.4;
/// Opacity of the text color behind `code`.
const CODE_BACKGROUND_OPACITY: f32 = 0.12;

/// Adds the bold font family to egui's default fonts.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(BOLD.to_owned(), egui::FontData::from_static(BOLD_TTF).into());
    // Characters Ubuntu Bold lacks (e.g. emoji) fall back to the regular fonts.
    let mut bold = vec![BOLD.to_owned()];
    bold.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
    fonts.families.insert(FontFamily::Name(BOLD.into()), bold);
    ctx.set_fonts(fonts);
}

/// The inline formatting of a piece of a line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
}

/// A piece of a line: text in `style`, or the markup characters that change it.
#[derive(Debug, PartialEq)]
struct Span {
    range: Range<usize>,
    style: Style,
    marker: bool,
}

/// How a line starts.
#[derive(Debug, PartialEq)]
enum Block {
    Paragraph,
    /// Level 1–3; the `#`s and a space.
    Heading(usize),
    /// `- ` or `* `.
    Bullet,
    /// `1. ` and the like.
    Numbered,
}

/// What a line starts with and how long that prefix is (in bytes).
fn block(line: &str) -> (Block, usize) {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=3).contains(&hashes) && line[hashes..].starts_with(' ') {
        return (Block::Heading(hashes), hashes + 1);
    }
    if line.starts_with("- ") || line.starts_with("* ") {
        return (Block::Bullet, 2);
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return (Block::Numbered, digits + 2);
    }
    (Block::Paragraph, 0)
}

/// Emphasis markers, longest first.
const MARKERS: [&str; 4] = ["**", "~~", "*", "_"];

fn flag<'a>(style: &'a mut Style, marker: &str) -> &'a mut bool {
    match marker {
        "**" => &mut style.bold,
        "~~" => &mut style.strike,
        _ => &mut style.italic,
    }
}

/// Splits a line (without its block prefix) into styled spans. A marker opens only before a
/// non-space with a matching closer later in the line, and closes only after a non-space; `_`
/// also not inside a word (snake_case stays as is). Anything else is literal text.
fn spans(line: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut style = Style::default();
    let mut text_start = 0;
    let mut i = 0;
    let push_text = |spans: &mut Vec<Span>, range: Range<usize>, style: Style| {
        if !range.is_empty() {
            spans.push(Span { range, style, marker: false });
        }
    };
    while i < line.len() {
        let rest = &line[i..];
        let before = line[..i].chars().next_back();
        if rest.starts_with('`')
            && let Some(len) = rest[1..].find('`').filter(|&len| len > 0)
        {
            push_text(&mut spans, text_start..i, style);
            let code = Style { code: true, ..style };
            spans.push(Span { range: i..i + 1, style, marker: true });
            spans.push(Span { range: i + 1..i + 1 + len, style: code, marker: false });
            spans.push(Span { range: i + 1 + len..i + 2 + len, style, marker: true });
            i += len + 2;
            text_start = i;
            continue;
        }
        if let Some(marker) = MARKERS.into_iter().find(|m| rest.starts_with(m)) {
            let after = rest[marker.len()..].chars().next();
            let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
            let space = |c: Option<char>| c.is_none_or(char::is_whitespace);
            let applies = if *flag(&mut style, marker) {
                !space(before) && !(marker == "_" && word(after))
            } else {
                !space(after) && !(marker == "_" && word(before)) && closer(&rest[marker.len()..], marker)
            };
            if applies {
                push_text(&mut spans, text_start..i, style);
                spans.push(Span { range: i..i + marker.len(), style, marker: true });
                let on = flag(&mut style, marker);
                *on = !*on;
                text_start = i + marker.len();
            }
            // A double marker that doesn't apply is literal as a whole, not two single ones.
            i += marker.len();
            continue;
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    push_text(&mut spans, text_start..line.len(), style);
    spans
}

/// Whether `rest` has a `marker` that could close one opened just before it (after a non-space).
fn closer(rest: &str, marker: &str) -> bool {
    rest.match_indices(marker).any(|(i, _)| rest[..i].chars().next_back().is_some_and(|c| !c.is_whitespace()))
}

/// Lays out `text` with its markdown formatting at `font_px`. While `editing`, the markup stays
/// in, faded (the layout's text must be exactly the edited text); otherwise it's hidden and
/// bullets show as `•`. The caller sets wrapping and alignment.
pub fn layout_job(text: &str, font_px: f32, color: Color32, editing: bool) -> LayoutJob {
    let mut job = LayoutJob { break_on_newline: true, ..Default::default() };
    let faded = color.gamma_multiply(MARKER_OPACITY);
    let format = |style: Style, px: f32, marker: bool| {
        let family = match (style.code && !marker, style.bold) {
            (true, _) => FontFamily::Monospace,
            (false, true) => FontFamily::Name(BOLD.into()),
            (false, false) => FontFamily::Proportional,
        };
        let content = !marker;
        TextFormat {
            font_id: FontId::new(px, family),
            color: if marker { faded } else { color },
            italics: style.italic && content,
            strikethrough: if style.strike && content { Stroke::new((px / 14.0).max(1.0), color) } else { Stroke::NONE },
            background: if style.code && content { color.gamma_multiply(CODE_BACKGROUND_OPACITY) } else { Color32::TRANSPARENT },
            ..Default::default()
        }
    };
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            job.append("\n", 0.0, format(Style::default(), font_px, false));
        }
        let (block, prefix) = block(line);
        let (px, bold) = match block {
            Block::Heading(level) => (font_px * HEADING_SCALE[level - 1], true),
            _ => (font_px, false),
        };
        let base = Style { bold, ..Style::default() };
        let mut indent = match block {
            Block::Bullet | Block::Numbered => font_px * LIST_INDENT,
            _ => 0.0,
        };
        // The prefix as shown, and whether it's markup.
        let shown = match block {
            Block::Paragraph => None,
            Block::Numbered => Some((&line[..prefix], false)),
            Block::Bullet if !editing => Some(("• ", false)),
            _ if editing => Some((&line[..prefix], true)),
            Block::Bullet | Block::Heading(_) => None,
        };
        if let Some((shown, marker)) = shown {
            job.append(shown, indent, format(base, px, marker));
            indent = 0.0;
        }
        let content = &line[prefix..];
        for span in spans(content) {
            if span.marker && !editing {
                continue;
            }
            let style = Style { bold: span.style.bold || bold, ..span.style };
            job.append(&content[span.range], indent, format(style, px, span.marker));
            indent = 0.0;
        }
    }
    // Empty text still gets a row (for the caret and the object's height).
    if job.sections.is_empty() {
        job.append("", 0.0, format(Style::default(), font_px, false));
    }
    job
}

/// Wraps the characters `start..end` (char indices) of `text` in `marker` (`**` or `*`), or
/// unwraps them if the markers are already just outside or just inside. Returns the new
/// selection, over the same characters; an empty one gets a pair of markers with the caret in it.
pub fn toggle_marker(text: &mut String, start: usize, end: usize, marker: &str) -> (usize, usize) {
    let byte = |c: usize| text.char_indices().nth(c).map_or(text.len(), |(b, _)| b);
    let (a, b) = (byte(start), byte(end));
    let len = marker.len();
    // `*` and `**` share a character: a run of 3 is both, a run of 2 is only bold.
    let has = |run: usize| if len == 1 { run % 2 == 1 } else { run >= 2 };
    let run_before = |s: &str| s.len() - s.trim_end_matches('*').len();
    let run_after = |s: &str| s.len() - s.trim_start_matches('*').len();
    let (before, selected, after) = (&text[..a], &text[a..b], &text[b..]);
    if has(run_before(before)) && has(run_after(after)) {
        text.replace_range(b..b + len, "");
        text.replace_range(a - len..a, "");
        (start - len, end - len)
    } else if selected.len() >= 2 * len && has(run_after(selected)) && has(run_before(selected)) {
        text.replace_range(b - len..b, "");
        text.replace_range(a..a + len, "");
        (start, end - 2 * len)
    } else {
        text.insert_str(b, marker);
        text.insert_str(a, marker);
        (start + len, end + len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spans of `line` as (text, marker) pairs, with the style of the text ones.
    fn parts(line: &str) -> Vec<(&str, bool, Style)> {
        spans(line).into_iter().map(|s| (&line[s.range], s.marker, s.style)).collect()
    }

    const PLAIN: Style = Style { bold: false, italic: false, strike: false, code: false };
    const BOLD_STYLE: Style = Style { bold: true, ..PLAIN };
    const ITALIC: Style = Style { italic: true, ..PLAIN };

    #[test]
    fn emphasis() {
        assert_eq!(parts("a **b** c"), vec![("a ", false, PLAIN), ("**", true, PLAIN), ("b", false, BOLD_STYLE), ("**", true, BOLD_STYLE), (" c", false, PLAIN)]);
        assert_eq!(parts("*i*"), vec![("*", true, PLAIN), ("i", false, ITALIC), ("*", true, ITALIC)]);
        assert_eq!(parts("_i_")[1], ("i", false, ITALIC));
        assert_eq!(parts("~~s~~")[1].2, Style { strike: true, ..PLAIN });
        let all = parts("***x***");
        assert_eq!(all[2], ("x", false, Style { bold: true, italic: true, ..PLAIN }));
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn literal_markers() {
        // Unclosed, surrounded by spaces, or inside a word.
        assert_eq!(parts("2 * 3 * 4"), vec![("2 * 3 * 4", false, PLAIN)]);
        assert_eq!(parts("**open"), vec![("**open", false, PLAIN)]);
        assert_eq!(parts("snake_case_name"), vec![("snake_case_name", false, PLAIN)]);
        assert_eq!(parts("a ** b ** c"), vec![("a ** b ** c", false, PLAIN)]);
        assert_eq!(parts("``"), vec![("``", false, PLAIN)]);
    }

    #[test]
    fn code_is_literal() {
        let code = Style { code: true, ..PLAIN };
        assert_eq!(parts("`**x**`"), vec![("`", true, PLAIN), ("**x**", false, code), ("`", true, PLAIN)]);
    }

    #[test]
    fn blocks() {
        assert_eq!(block("# Title"), (Block::Heading(1), 2));
        assert_eq!(block("### Title"), (Block::Heading(3), 4));
        assert_eq!(block("#### Title"), (Block::Paragraph, 0));
        assert_eq!(block("#hashtag"), (Block::Paragraph, 0));
        assert_eq!(block("- item"), (Block::Bullet, 2));
        assert_eq!(block("* item"), (Block::Bullet, 2));
        assert_eq!(block("12. item"), (Block::Numbered, 4));
        assert_eq!(block("12.5 kg"), (Block::Paragraph, 0));
    }

    #[test]
    fn editing_keeps_every_character() {
        let text = "# Head\n- **a** *b*\n1. `c` ~~d~~\nsnake_case";
        assert_eq!(layout_job(text, 16.0, Color32::BLACK, true).text, text);
        assert_eq!(layout_job(text, 16.0, Color32::BLACK, false).text, "Head\n• a b\n1. c d\nsnake_case");
    }

    fn toggled(text: &str, start: usize, end: usize, marker: &str) -> (String, (usize, usize)) {
        let mut text = text.to_owned();
        let range = toggle_marker(&mut text, start, end, marker);
        (text, range)
    }

    #[test]
    fn toggling_markers() {
        assert_eq!(toggled("a b c", 2, 3, "**"), ("a **b** c".to_owned(), (4, 5)));
        assert_eq!(toggled("a **b** c", 4, 5, "**"), ("a b c".to_owned(), (2, 3)));
        assert_eq!(toggled("a **b** c", 2, 7, "**"), ("a b c".to_owned(), (2, 3)));
        assert_eq!(toggled("ab", 1, 1, "*"), ("a**b".to_owned(), (2, 2)));
        // Italic inside bold and back; bold doesn't count as italic.
        assert_eq!(toggled("**b**", 2, 3, "*"), ("***b***".to_owned(), (3, 4)));
        assert_eq!(toggled("***b***", 3, 4, "*"), ("**b**".to_owned(), (2, 3)));
        assert_eq!(toggled("***b***", 3, 4, "**"), ("*b*".to_owned(), (1, 2)));
        assert_eq!(toggled("ā ē", 2, 3, "*"), ("ā *ē*".to_owned(), (3, 4)));
    }
}
