use serde::{Deserialize, Serialize};

use crate::model::{
    MAX_CATEGORY_CHARS, MAX_DESCRIPTION_CHARS, MAX_EXPLANATION_CHARS, MAX_OPTIONS, MAX_OPTION_CHARS,
    MAX_PROMPT_CHARS, MAX_QUESTIONS, MAX_TAGS, MAX_TAG_CHARS, MAX_TIME_LIMIT_SECS, MAX_TITLE_CHARS,
    Quiz,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub field: String,
    pub message: String,
    pub severity: ValidationSeverity,
    pub question_id: Option<String>,
    pub question_index: Option<usize>,
}

fn issue(
    field: &str,
    message: impl Into<String>,
    severity: ValidationSeverity,
    question: Option<(usize, &str)>,
) -> ValidationIssue {
    ValidationIssue {
        field: field.to_string(),
        message: message.into(),
        severity,
        question_id: question.map(|(_, id)| id.to_string()),
        question_index: question.map(|(index, _)| index),
    }
}

fn too_long(value: &str, max: usize) -> bool {
    value.chars().count() > max
}

pub(crate) fn issues_for(quiz: &Quiz) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let error = ValidationSeverity::Error;

    let title = quiz.title.trim();
    if title.is_empty() {
        issues.push(issue("title", "Give the quiz a title.", error, None));
    } else if too_long(title, MAX_TITLE_CHARS) {
        issues.push(issue(
            "title",
            format!("The title must be {MAX_TITLE_CHARS} characters or fewer."),
            error,
            None,
        ));
    }

    if too_long(&quiz.description, MAX_DESCRIPTION_CHARS) {
        issues.push(issue(
            "description",
            format!("The description must be {MAX_DESCRIPTION_CHARS} characters or fewer."),
            error,
            None,
        ));
    }

    if let Some(category) = &quiz.category
        && too_long(category, MAX_CATEGORY_CHARS)
    {
        issues.push(issue(
            "category",
            format!("The category must be {MAX_CATEGORY_CHARS} characters or fewer."),
            error,
            None,
        ));
    }

    if quiz.tags.len() > MAX_TAGS {
        issues.push(issue(
            "tags",
            format!("Use at most {MAX_TAGS} tags."),
            error,
            None,
        ));
    }
    for tag in &quiz.tags {
        if too_long(tag, MAX_TAG_CHARS) {
            issues.push(issue(
                "tags",
                format!("Each tag must be {MAX_TAG_CHARS} characters or fewer."),
                error,
                None,
            ));
            break;
        }
    }

    if let Some(seconds) = quiz.settings.time_limit_secs
        && (seconds == 0 || seconds > MAX_TIME_LIMIT_SECS)
    {
        issues.push(issue(
            "timeLimit",
            "The time limit must be between 1 and 300 minutes.",
            error,
            None,
        ));
    }
    if let Some(pass) = quiz.settings.pass_percent
        && !(1..=100).contains(&pass)
    {
        issues.push(issue(
            "passPercent",
            "The pass mark must be between 1 and 100 percent.",
            error,
            None,
        ));
    }

    if quiz.questions.is_empty() {
        issues.push(issue(
            "questions",
            "Add at least one question.",
            error,
            None,
        ));
    }
    if quiz.questions.len() > MAX_QUESTIONS {
        issues.push(issue(
            "questions",
            format!("A quiz can have at most {MAX_QUESTIONS} questions."),
            error,
            None,
        ));
    }

    for (index, question) in quiz.questions.iter().enumerate() {
        let q = Some((index, question.id.as_str()));
        let label = format!("Question {}", index + 1);
        if question.prompt.trim().is_empty() {
            issues.push(issue(
                "prompt",
                format!("{label}: add a question prompt."),
                error,
                q,
            ));
        } else if too_long(&question.prompt, MAX_PROMPT_CHARS) {
            issues.push(issue(
                "prompt",
                format!("{label}: prompt must be {MAX_PROMPT_CHARS} characters or fewer."),
                error,
                q,
            ));
        }

        if question.options.len() < 2 {
            issues.push(issue(
                "options",
                format!("{label}: add at least two answer options."),
                error,
                q,
            ));
        } else if question.options.len() > MAX_OPTIONS {
            issues.push(issue(
                "options",
                format!("{label}: use at most {MAX_OPTIONS} answer options."),
                error,
                q,
            ));
        }

        for (option_index, option) in question.options.iter().enumerate() {
            if option.text.trim().is_empty() {
                issues.push(issue(
                    "option",
                    format!("{label}: option {} needs text.", option_index + 1),
                    error,
                    q,
                ));
            } else if too_long(&option.text, MAX_OPTION_CHARS) {
                issues.push(issue(
                    "option",
                    format!(
                        "{label}: option {} must be {MAX_OPTION_CHARS} characters or fewer.",
                        option_index + 1
                    ),
                    error,
                    q,
                ));
            }
        }

        let correct = question.correct_count();
        match question.kind {
            crate::model::QuestionKind::Single if correct != 1 => {
                issues.push(issue(
                    "correct",
                    format!("{label}: mark exactly one correct option."),
                    error,
                    q,
                ));
            }
            crate::model::QuestionKind::Multi if correct < 2 => {
                issues.push(issue(
                    "correct",
                    format!("{label}: mark at least two correct options."),
                    error,
                    q,
                ));
            }
            _ => {}
        }

        if too_long(&question.explanation, MAX_EXPLANATION_CHARS) {
            issues.push(issue(
                "explanation",
                format!("{label}: explanation must be {MAX_EXPLANATION_CHARS} characters or fewer."),
                error,
                q,
            ));
        }
    }

    issues
}
