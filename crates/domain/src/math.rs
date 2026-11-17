//! Formulas written as LaTeX between `$...$` (inline) and `$$...$$` (display).
//!
//! Rules:
//! - An inline formula closes on the same line. A display formula may run over
//!   several lines.
//! - `\$` outside a formula is a literal dollar sign. Inside a formula every
//!   backslash belongs to LaTeX, so `\$` there does not close the formula.
//! - A `$` or `$$` with no closing partner is plain text. The scanner counts
//!   these so the editor and the paste parser can warn about them.

/// A formula found in some text. `start` is the byte index of its opening
/// delimiter and `end` is the byte index just past its closing delimiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathSpan {
    pub start: usize,
    pub end: usize,
    pub display: bool,
}

impl MathSpan {
    fn delimiter_len(self) -> usize {
        if self.display { 2 } else { 1 }
    }

    /// The LaTeX between the delimiters.
    pub fn source<'a>(&self, text: &'a str) -> &'a str {
        let d = self.delimiter_len();
        &text[self.start + d..self.end - d]
    }
}

/// The formulas in a text, and how many `$` or `$$` openers never closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    pub spans: Vec<MathSpan>,
    pub unclosed: usize,
}

/// Finds every formula in `text`, scanning from the start.
pub fn math_spans(text: &str) -> Scan {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut unclosed = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'$') {
            i += 2;
            continue;
        }
        if bytes[i] != b'$' {
            i += 1;
            continue;
        }
        let display = bytes.get(i + 1) == Some(&b'$');
        let body = i + if display { 2 } else { 1 };
        match closer(bytes, body, display) {
            Some(close) => {
                let end = close + if display { 2 } else { 1 };
                spans.push(MathSpan { start: i, end, display });
                i = end;
            }
            None => {
                unclosed += 1;
                i = body;
            }
        }
    }
    Scan { spans, unclosed }
}

/// Index of the delimiter that closes a formula whose body starts at `from`.
fn closer(bytes: &[u8], from: usize, display: bool) -> Option<usize> {
    let mut j = from;
    while j < bytes.len() {
        match bytes[j] {
            // Skip the escaped character, so `\$` never closes a formula.
            b'\\' => j += 2,
            b'\n' if !display => return None,
            b'$' if !display => return Some(j),
            b'$' if bytes.get(j + 1) == Some(&b'$') => return Some(j),
            _ => j += 1,
        }
    }
    None
}

/// True when byte `index` falls inside one of the spans.
pub fn contains(spans: &[MathSpan], index: usize) -> bool {
    spans.iter().any(|span| span.start <= index && index < span.end)
}

/// One part of a text: plain words, or a formula to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Plain text, with `\$` already turned into `$`.
    Text(String),
    Math { display: bool, source: String },
}

/// Splits a text into plain and formula pieces, in order.
pub fn pieces(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut at = 0;
    for span in math_spans(text).spans {
        if span.start > at {
            out.push(Piece::Text(unescape(&text[at..span.start])));
        }
        out.push(Piece::Math {
            display: span.display,
            source: span.source(text).to_string(),
        });
        at = span.end;
    }
    if at < text.len() {
        out.push(Piece::Text(unescape(&text[at..])));
    }
    out
}

/// The text with its `$` delimiters removed and `\$` resolved. Used for labels
/// that screen readers announce.
pub fn plain_text(text: &str) -> String {
    pieces(text)
        .into_iter()
        .map(|piece| match piece {
            Piece::Text(text) => text,
            Piece::Math { source, .. } => source,
        })
        .collect()
}

fn unescape(text: &str) -> String {
    text.replace("\\$", "$")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_inline_and_display_formulas() {
        let scan = math_spans("Solve $x^2$ and $$\\int_0^1 f$$ now.");
        assert_eq!(scan.unclosed, 0);
        assert_eq!(scan.spans.len(), 2);
        assert!(!scan.spans[0].display);
        assert_eq!(scan.spans[0].source("Solve $x^2$ and $$\\int_0^1 f$$ now."), "x^2");
        assert!(scan.spans[1].display);
    }

    #[test]
    fn escaped_dollar_outside_a_formula_is_text() {
        let text = "It costs \\$5, not $x$.";
        let scan = math_spans(text);
        assert_eq!(scan.unclosed, 0);
        assert_eq!(scan.spans.len(), 1);
        assert_eq!(
            pieces(text),
            vec![
                Piece::Text("It costs $5, not ".into()),
                Piece::Math { display: false, source: "x".into() },
                Piece::Text(".".into()),
            ]
        );
    }

    #[test]
    fn escaped_dollar_inside_a_formula_does_not_close_it() {
        let text = "$a\\$b$";
        let scan = math_spans(text);
        assert_eq!(scan.spans.len(), 1);
        assert_eq!(scan.spans[0].source(text), "a\\$b");
    }

    #[test]
    fn unmatched_dollar_is_counted_and_kept_as_text() {
        let scan = math_spans("It costs $5");
        assert_eq!(scan.unclosed, 1);
        assert!(scan.spans.is_empty());
        assert_eq!(pieces("It costs $5"), vec![Piece::Text("It costs $5".into())]);
    }

    #[test]
    fn inline_formula_does_not_cross_a_line() {
        let scan = math_spans("first $a\nsecond$");
        assert_eq!(scan.unclosed, 2);
        assert!(scan.spans.is_empty());
    }

    #[test]
    fn display_formula_can_span_lines() {
        let text = "$$\na\n$$";
        let scan = math_spans(text);
        assert_eq!(scan.unclosed, 0);
        assert_eq!(scan.spans.len(), 1);
        assert!(scan.spans[0].display);
    }

    #[test]
    fn backslash_before_closing_delimiter_is_latex() {
        // `\\` is a LaTeX line break, so the `$` after it still closes the formula.
        let text = "$a\\\\$";
        let scan = math_spans(text);
        assert_eq!(scan.spans.len(), 1);
        assert_eq!(scan.spans[0].source(text), "a\\\\");
    }

    #[test]
    fn plain_text_drops_delimiters() {
        assert_eq!(plain_text("Area $\\pi r^2$ and \\$1"), "Area \\pi r^2 and $1");
    }

    #[test]
    fn contains_reports_positions_inside_spans() {
        let spans = math_spans("a $b*c$ d").spans;
        assert!(contains(&spans, 3));
        assert!(!contains(&spans, 0));
        assert!(!contains(&spans, 8));
    }
}
