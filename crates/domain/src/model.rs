use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::validation::ValidationIssue;

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub const MAX_QUESTIONS: usize = 200;
pub const MIN_OPTIONS: usize = 2;
pub const MAX_OPTIONS: usize = 8;
pub const MAX_TAGS: usize = 8;
pub const MAX_TITLE_CHARS: usize = 120;
pub const MAX_DESCRIPTION_CHARS: usize = 1_000;
pub const MAX_CATEGORY_CHARS: usize = 48;
pub const MAX_TAG_CHARS: usize = 24;
pub const MAX_PROMPT_CHARS: usize = 2_000;
pub const MAX_OPTION_CHARS: usize = 500;
pub const MAX_EXPLANATION_CHARS: usize = 4_000;
pub const MAX_TIME_LIMIT_SECS: u32 = 300 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuestionKind {
    #[default]
    Single,
    Multi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuizSource {
    #[default]
    User,
    Sample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttemptStatus {
    InProgress,
    Submitted,
    Expired,
}

impl AttemptStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Submitted => "submitted",
            Self::Expired => "expired",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "submitted" => Self::Submitted,
            "expired" => Self::Expired,
            _ => Self::InProgress,
        }
    }

    pub fn is_finished(self) -> bool {
        !matches!(self, Self::InProgress)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnswerOption {
    #[serde(default = "new_id")]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub is_correct: bool,
}

impl AnswerOption {
    pub fn blank(position: usize) -> Self {
        Self {
            id: new_id(),
            text: String::new(),
            is_correct: position == 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    #[serde(default = "new_id")]
    pub id: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub kind: QuestionKind,
    #[serde(default)]
    pub options: Vec<AnswerOption>,
}

impl Question {
    pub fn blank() -> Self {
        Self {
            id: new_id(),
            prompt: String::new(),
            explanation: String::new(),
            kind: QuestionKind::Single,
            options: (0..4).map(AnswerOption::blank).collect(),
        }
    }

    pub fn correct_count(&self) -> usize {
        self.options.iter().filter(|option| option.is_correct).count()
    }

    pub fn is_valid(&self) -> bool {
        !self.prompt.trim().is_empty()
            && (MIN_OPTIONS..=MAX_OPTIONS).contains(&self.options.len())
            && self.options.iter().all(|option| !option.text.trim().is_empty())
            && match self.kind {
                QuestionKind::Single => self.correct_count() == 1,
                QuestionKind::Multi => self.correct_count() >= 2,
            }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuizSettings {
    #[serde(default)]
    pub time_limit_secs: Option<u32>,
    #[serde(default)]
    pub shuffle_questions: bool,
    #[serde(default)]
    pub shuffle_options: bool,
    #[serde(default)]
    pub pass_percent: Option<u8>,
    #[serde(default = "default_true")]
    pub one_at_a_time: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for QuizSettings {
    fn default() -> Self {
        Self {
            time_limit_secs: None,
            shuffle_questions: false,
            shuffle_options: false,
            pass_percent: None,
            one_at_a_time: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Quiz {
    #[serde(default = "new_id")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub settings: QuizSettings,
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub source: QuizSource,
    #[serde(default)]
    pub sample_slug: Option<String>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl Quiz {
    pub fn blank(now: i64) -> Self {
        Self {
            id: new_id(),
            title: "Untitled quiz".to_string(),
            description: String::new(),
            category: None,
            tags: Vec::new(),
            settings: QuizSettings::default(),
            questions: Vec::new(),
            source: QuizSource::User,
            sample_slug: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.validation_issues().is_empty()
    }

    pub fn validation_issues(&self) -> Vec<ValidationIssue> {
        crate::validation::issues_for(self)
    }

    pub fn normalized(mut self) -> Self {
        self.tags = self
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .fold(Vec::new(), |mut tags, tag| {
                if !tags.iter().any(|existing| existing.eq_ignore_ascii_case(&tag)) {
                    tags.push(tag);
                }
                tags
            });
        self.tags.truncate(MAX_TAGS);
        self.category = self
            .category
            .map(|category| category.trim().to_string())
            .filter(|category| !category.is_empty());
        for question in &mut self.questions {
            question.id = if question.id.trim().is_empty() {
                new_id()
            } else {
                question.id.clone()
            };
            for option in &mut question.options {
                option.id = if option.id.trim().is_empty() {
                    new_id()
                } else {
                    option.id.clone()
                };
            }
        }
        self
    }

    /// A deep copy suitable for duplication inside the current library.
    pub fn duplicated(&self, now: i64) -> Self {
        let mut copy = self.clone();
        copy.id = new_id();
        copy.source = QuizSource::User;
        copy.sample_slug = None;
        copy.created_at = now;
        copy.updated_at = now;
        for question in &mut copy.questions {
            question.id = new_id();
            for option in &mut question.options {
                option.id = new_id();
            }
        }
        copy
    }

    /// A deep copy prepared for a pack import. Incoming IDs are never trusted.
    pub fn imported(&self, now: i64) -> Self {
        let mut copy = self.duplicated(now);
        copy.created_at = now;
        copy.updated_at = now;
        copy
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizSummary {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub question_count: u32,
    pub time_limit_secs: Option<u32>,
    pub pass_percent: Option<u8>,
    pub source: QuizSource,
    pub ready: bool,
    pub updated_at: i64,
    pub attempts_count: u32,
    pub best_percent: Option<f64>,
    pub active_attempt_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptSummary {
    pub id: String,
    pub quiz_id: Option<String>,
    pub quiz_title: String,
    pub status: AttemptStatus,
    pub started_at: i64,
    pub deadline_at: Option<i64>,
    pub submitted_at: Option<i64>,
    pub correct_count: Option<u32>,
    pub total_count: u32,
    pub score_percent: Option<f64>,
    pub passed: Option<bool>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub quizzes: Vec<QuizSummary>,
    pub attempts: Vec<AttemptSummary>,
}

#[derive(Debug, Clone)]
pub struct AttemptMeta {
    pub id: String,
    pub quiz_id: Option<String>,
    pub quiz_title: String,
    pub status: AttemptStatus,
    pub started_at: i64,
    pub deadline_at: Option<i64>,
    pub submitted_at: Option<i64>,
    pub correct_count: Option<u32>,
    pub total_count: u32,
    pub score_percent: Option<f64>,
    pub passed: Option<bool>,
    pub duration_ms: Option<i64>,
}

impl AttemptMeta {
    pub fn summary(&self) -> AttemptSummary {
        AttemptSummary {
            id: self.id.clone(),
            quiz_id: self.quiz_id.clone(),
            quiz_title: self.quiz_title.clone(),
            status: self.status,
            started_at: self.started_at,
            deadline_at: self.deadline_at,
            submitted_at: self.submitted_at,
            correct_count: self.correct_count,
            total_count: self.total_count,
            score_percent: self.score_percent,
            passed: self.passed,
            duration_ms: self.duration_ms,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAnswer {
    pub question_id: String,
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub flagged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptOptionView {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptQuestionView {
    pub id: String,
    pub prompt: String,
    pub kind: QuestionKind,
    pub options: Vec<AttemptOptionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptView {
    pub id: String,
    pub quiz_id: Option<String>,
    pub quiz_title: String,
    pub status: AttemptStatus,
    pub started_at: i64,
    pub deadline_at: Option<i64>,
    pub submitted_at: Option<i64>,
    pub one_at_a_time: bool,
    pub total_questions: u32,
    pub questions: Vec<AttemptQuestionView>,
    pub answers: Vec<SavedAnswer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOption {
    pub id: String,
    pub text: String,
    pub is_correct: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQuestion {
    pub id: String,
    pub prompt: String,
    pub kind: QuestionKind,
    pub explanation: String,
    pub options: Vec<ReviewOption>,
    pub selected: Vec<String>,
    pub flagged: bool,
    pub is_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptReview {
    pub attempt: AttemptSummary,
    pub pass_percent: Option<u8>,
    pub questions: Vec<ReviewQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAnswerResult {
    pub auto_submitted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub quizzes: u32,
    pub attempts: u32,
}
