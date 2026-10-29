//! Reads questions written as plain text.
//!
//! ```text
//! 1. What is the powerhouse of the cell?
//! A) Nucleus
//! B) Mitochondria *                (* marks a correct answer)
//! C) Ribosome
//! Explanation: Mitochondria make ATP.
//!
//! 2. Pick the primary colours (multi)
//! a) Red *
//! b) Green
//! c) Blue *
//! d) Yellow
//! ```
//!
//! A question starts with a number and `.` or `)`. Options start with a letter
//! and `)` or `.`. A question with several correct answers needs `(multi)` after
//! its text. Other lines continue the question text or explanation. The quiz's
//! title and settings are set in the app, not in the text.
//!
//! A `*` at the end of an option marks it correct. For an option whose text is
//! a star, write `\*` at the end, or leave a lone `*` as the whole option.

use crate::model::{
    AnswerOption, MAX_OPTIONS, MAX_QUESTIONS, MIN_OPTIONS, Question, QuestionKind, new_id,
};

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Prompt,
    Options,
    Explanation,
}

struct Draft {
    line: usize,
    prompt: String,
    explanation: String,
    multi: bool,
    options: Vec<(String, bool)>,
    section: Section,
}

/// Parses questions from text. On failure returns every problem found, each with its line number.
pub fn parse_quiz_text(text: &str) -> Result<Vec<Question>, Vec<String>> {
    let mut questions = Vec::new();
    let mut errors = Vec::new();
    let mut current: Option<Draft> = None;

    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = question_start(line) {
            if let Some(draft) = current.take() {
                finish(draft, &mut questions, &mut errors);
            }
            let (prompt, multi) = strip_multi(rest);
            current = Some(Draft {
                line: line_no,
                prompt: prompt.to_string(),
                explanation: String::new(),
                multi,
                options: Vec::new(),
                section: Section::Prompt,
            });
            continue;
        }

        if let Some(rest) = option_start(line) {
            match current.as_mut() {
                Some(draft) => {
                    let (label, correct) = option_text(rest);
                    draft.options.push((label, correct));
                    draft.section = Section::Options;
                }
                None => errors.push(format!(
                    "Line {line_no}: an answer option must come after a question."
                )),
            }
            continue;
        }

        if let Some(rest) = strip_label(line, "explanation:") {
            match current.as_mut() {
                Some(draft) => {
                    draft.explanation = rest.trim().to_string();
                    draft.section = Section::Explanation;
                }
                None => errors.push(format!(
                    "Line {line_no}: an explanation must come after a question."
                )),
            }
            continue;
        }

        match current.as_mut() {
            Some(draft) => match draft.section {
                Section::Prompt => {
                    draft.prompt.push('\n');
                    draft.prompt.push_str(line);
                }
                Section::Explanation => {
                    draft.explanation.push('\n');
                    draft.explanation.push_str(line);
                }
                Section::Options => errors.push(format!(
                    "Line {line_no}: \"{line}\" is not an option. Options start with a letter and ) or ., like A) or a."
                )),
            },
            None => errors.push(format!(
                "Line {line_no}: \"{line}\" is not understood. Each question starts with a number, like 1."
            )),
        }
    }

    if let Some(draft) = current.take() {
        finish(draft, &mut questions, &mut errors);
    }
    if questions.is_empty() && errors.is_empty() {
        errors.push("No questions found. Start each question with a number, like 1. What is...".to_string());
    }
    if questions.len() > MAX_QUESTIONS {
        errors.push(format!("A quiz can have at most {MAX_QUESTIONS} questions."));
    }

    if errors.is_empty() { Ok(questions) } else { Err(errors) }
}

/// `1. text` or `12) text` → the text after the number.
fn question_start(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let after = line[digits..].strip_prefix(['.', ')'])?;
    if after.is_empty() || after.starts_with(char::is_whitespace) {
        Some(after.trim())
    } else {
        None
    }
}

/// `A) text` or `b. text` → the text after the letter.
fn option_start(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || !matches!(bytes[1], b')' | b'.') {
        return None;
    }
    let after = &line[2..];
    if after.is_empty() || after.starts_with(char::is_whitespace) {
        Some(after.trim())
    } else {
        None
    }
}

/// Splits an option into its text and whether it is correct.
///
/// A `*` at the end marks the option correct. Two cases keep a star as text:
/// a lone `*` (the whole option is the star), and `\*` at the end (an escaped star).
fn option_text(rest: &str) -> (String, bool) {
    let trimmed = rest.trim_end();
    if let Some(literal) = trimmed.strip_suffix("\\*") {
        return (format!("{}*", literal.trim_end()), false);
    }
    if let Some(label) = trimmed.strip_suffix('*') {
        if !label.trim().is_empty() {
            return (label.trim().to_string(), true);
        }
    }
    (trimmed.trim().to_string(), false)
}

/// Removes a trailing `(multi)` tag from a question line.
fn strip_multi(rest: &str) -> (&str, bool) {
    let trimmed = rest.trim_end();
    let tag = "(multi)";
    if trimmed.len() >= tag.len() {
        let cut = trimmed.len() - tag.len();
        if let Some(end) = trimmed.get(cut..) {
            if end.eq_ignore_ascii_case(tag) {
                return (trimmed[..cut].trim_end(), true);
            }
        }
    }
    (trimmed, false)
}

/// Case-insensitive prefix match that returns the rest of the line.
fn strip_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let head = line.get(..label.len())?;
    if head.eq_ignore_ascii_case(label) {
        Some(&line[label.len()..])
    } else {
        None
    }
}

fn finish(draft: Draft, questions: &mut Vec<Question>, errors: &mut Vec<String>) {
    let label = format!("Question {} (line {})", questions.len() + 1, draft.line);
    if draft.prompt.trim().is_empty() {
        errors.push(format!("{label}: the question has no text."));
    }
    if draft.options.len() < MIN_OPTIONS {
        errors.push(format!("{label}: add at least two options, like A) and B)."));
    }
    if draft.options.len() > MAX_OPTIONS {
        errors.push(format!("{label}: use at most {MAX_OPTIONS} options."));
    }
    if draft.options.iter().any(|(text, _)| text.is_empty()) {
        errors.push(format!("{label}: an option has no text."));
    }
    let correct = draft.options.iter().filter(|(_, c)| *c).count();
    if draft.multi {
        if correct < 2 {
            errors.push(format!("{label}: a (multi) question needs at least two options marked *."));
        }
    } else if correct == 0 {
        errors.push(format!("{label}: no correct answer. Add * after the right option."));
    } else if correct > 1 {
        errors.push(format!(
            "{label}: more than one correct answer. Add (multi) after the question if several answers are right."
        ));
    }

    questions.push(Question {
        id: new_id(),
        prompt: draft.prompt.trim().to_string(),
        explanation: draft.explanation,
        kind: if draft.multi { QuestionKind::Multi } else { QuestionKind::Single },
        options: draft
            .options
            .into_iter()
            .map(|(text, is_correct)| AnswerOption { id: new_id(), text, is_correct })
            .collect(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Quiz;

    const SAMPLE: &str = "\
1. What is the powerhouse of the cell?
A) Nucleus
B) Mitochondria *
C) Ribosome
Explanation: Mitochondria make ATP.

2. Pick the primary colours (multi)
a) Red *
b) Green
c) Blue *
d) Yellow
";

    #[test]
    fn parses_questions_options_and_answers() {
        let questions = parse_quiz_text(SAMPLE).unwrap();
        assert_eq!(questions.len(), 2);

        let first = &questions[0];
        assert_eq!(first.prompt, "What is the powerhouse of the cell?");
        assert_eq!(first.kind, QuestionKind::Single);
        assert_eq!(first.options.len(), 3);
        assert!(first.options[1].is_correct);
        assert_eq!(first.options[1].text, "Mitochondria");
        assert_eq!(first.explanation, "Mitochondria make ATP.");

        let second = &questions[1];
        assert_eq!(second.kind, QuestionKind::Multi);
        assert_eq!(second.options.iter().filter(|o| o.is_correct).count(), 2);
    }

    #[test]
    fn parsed_questions_make_a_ready_quiz() {
        let mut quiz = Quiz::blank(0);
        quiz.title = "Biology basics".into();
        quiz.questions = parse_quiz_text(SAMPLE).unwrap();
        assert!(quiz.is_ready(), "{:?}", quiz.validation_issues());
    }

    #[test]
    fn reports_problems_with_line_numbers() {
        let errors = parse_quiz_text("A) Orphan option\n1. Q?\nA) x\nB) y\n").unwrap_err();
        assert!(errors.iter().any(|e| e.starts_with("Line 1:")));
        assert!(errors.iter().any(|e| e.contains("no correct answer")));
    }

    #[test]
    fn single_answer_with_two_correct_is_rejected() {
        let errors = parse_quiz_text("1. Q?\nA) x *\nB) y *\n").unwrap_err();
        assert!(errors.iter().any(|e| e.contains("more than one correct")));
    }

    #[test]
    fn a_lone_star_is_option_text_not_a_marker() {
        let questions = parse_quiz_text("1. Which is multiplication?\nA) +\nB) *\nC) /\nD) - *\n").unwrap();
        let options = &questions[0].options;
        assert_eq!(options[1].text, "*");
        assert!(!options[1].is_correct);
        assert!(options[3].is_correct);
    }

    #[test]
    fn escaped_star_is_literal_text() {
        let questions = parse_quiz_text("1. Q?\nA) \\*\nB) x *\n").unwrap();
        assert_eq!(questions[0].options[0].text, "*");
        assert!(!questions[0].options[0].is_correct);
    }

    #[test]
    fn right_associativity_question_parses() {
        let text = "69. Which is right-associative?\nA) +\nB) *\nC) = (assignment) *\nD) /\nExplanation: Assignment is right-associative.\n";
        let questions = parse_quiz_text(text).unwrap();
        assert_eq!(questions[0].options[2].text, "= (assignment)");
        assert_eq!(questions[0].options.iter().filter(|o| o.is_correct).count(), 1);
    }

    #[test]
    fn header_style_lines_are_rejected() {
        let errors = parse_quiz_text("Title: Biology\n1. Q?\nA) x *\nB) y\n").unwrap_err();
        assert!(errors[0].starts_with("Line 1:"));
    }

    #[test]
    fn empty_text_has_no_questions() {
        let errors = parse_quiz_text("   \n").unwrap_err();
        assert!(errors[0].contains("No questions"));
    }

    #[test]
    fn prompt_spans_lines() {
        let questions = parse_quiz_text("1. First line\nsecond line\nA) x *\nB) y\n").unwrap();
        assert_eq!(questions[0].prompt, "First line\nsecond line");
    }
}
