use serde::{Deserialize, Serialize};

use crate::model::{MAX_OPTIONS, MAX_QUESTIONS, Quiz};

pub const PACK_FORMAT: &str = "mocktest.quizpack";
pub const PACK_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("This file is not a MockTest quiz pack.")]
    InvalidFormat,
    #[error("This quiz pack uses an unsupported version ({0}).")]
    UnsupportedVersion(u32),
    #[error("The quiz pack is damaged: {0}")]
    Json(String),
    #[error("The quiz pack is invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizPack {
    pub format: String,
    pub version: u32,
    #[serde(rename = "exportedAt")]
    pub exported_at: i64,
    pub quiz: Quiz,
}

pub fn export_quiz(quiz: &Quiz, now: i64) -> Result<String, PackError> {
    let mut quiz = quiz.clone();
    quiz.source = crate::model::QuizSource::User;
    quiz.sample_slug = None;
    let pack = QuizPack {
        format: PACK_FORMAT.to_string(),
        version: PACK_VERSION,
        exported_at: now,
        quiz,
    };
    serde_json::to_string_pretty(&pack).map_err(|error| PackError::Json(error.to_string()))
}

pub fn import_quiz(json: &str, now: i64) -> Result<Quiz, PackError> {
    Ok(read_pack(json)?.imported(now))
}

pub fn read_pack(json: &str) -> Result<Quiz, PackError> {
    let pack: QuizPack =
        serde_json::from_str(json).map_err(|error| PackError::Json(error.to_string()))?;
    if pack.format != PACK_FORMAT {
        return Err(PackError::InvalidFormat);
    }
    if pack.version != PACK_VERSION {
        return Err(PackError::UnsupportedVersion(pack.version));
    }
    if pack.quiz.questions.len() > MAX_QUESTIONS {
        return Err(PackError::Invalid(format!(
            "a quiz can have at most {MAX_QUESTIONS} questions"
        )));
    }
    for (index, question) in pack.quiz.questions.iter().enumerate() {
        if question.options.len() > MAX_OPTIONS {
            return Err(PackError::Invalid(format!(
                "question {} has more than {MAX_OPTIONS} options",
                index + 1
            )));
        }
    }
    Ok(pack.quiz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AnswerOption, Question, QuizSettings};

    fn sample() -> Quiz {
        let mut quiz = Quiz::blank(0);
        quiz.id = "incoming-id".into();
        quiz.title = "Pack test".into();
        quiz.settings = QuizSettings {
            pass_percent: Some(60),
            ..Default::default()
        };
        quiz.questions = vec![Question {
            id: "q".into(),
            prompt: "Pick one".into(),
            explanation: String::new(),
            kind: crate::model::QuestionKind::Single,
            options: vec![
                AnswerOption {
                    id: "a".into(),
                    text: "A".into(),
                    is_correct: true,
                },
                AnswerOption {
                    id: "b".into(),
                    text: "B".into(),
                    is_correct: false,
                },
            ],
        }];
        quiz
    }

    #[test]
    fn round_trip_regenerates_ids() {
        let quiz = sample();
        let json = export_quiz(&quiz, 10).unwrap();
        let imported = import_quiz(&json, 20).unwrap();
        assert_ne!(imported.id, quiz.id);
        assert_ne!(imported.questions[0].id, quiz.questions[0].id);
        assert_eq!(imported.settings.pass_percent, Some(60));
        assert_eq!(imported.created_at, 20);
        assert!(imported.is_ready());
    }

    #[test]
    fn rejects_wrong_format() {
        let err = import_quiz(r#"{"format":"nope","version":1,"exportedAt":0,"quiz":{}}"#, 0)
            .unwrap_err();
        assert!(matches!(err, PackError::InvalidFormat));
    }
}
