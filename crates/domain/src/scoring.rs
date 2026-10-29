use serde::{Deserialize, Serialize};

use crate::model::{
    AttemptMeta, AttemptOptionView, AttemptQuestionView, AttemptReview, AttemptView, QuestionKind,
    Quiz, ReviewOption, ReviewQuestion, SavedAnswer,
};
use crate::shuffle::{seed_for, shuffled_order};

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub correct: u32,
    pub total: u32,
    pub percent: f64,
    pub passed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotOption {
    pub id: String,
    pub text: String,
    pub is_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotQuestion {
    pub id: String,
    pub prompt: String,
    pub kind: QuestionKind,
    pub explanation: String,
    pub options: Vec<SnapshotOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptSnapshot {
    pub quiz_id: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub settings: crate::model::QuizSettings,
    pub questions: Vec<SnapshotQuestion>,
}

impl AttemptSnapshot {
    pub fn build(quiz: &Quiz, seed: u64) -> Self {
        let question_order = if quiz.settings.shuffle_questions {
            shuffled_order(quiz.questions.len(), seed)
        } else {
            (0..quiz.questions.len()).collect()
        };

        let questions = question_order
            .into_iter()
            .map(|index| {
                let question = &quiz.questions[index];
                let option_order = if quiz.settings.shuffle_options {
                    shuffled_order(
                        question.options.len(),
                        seed_for(seed, &format!("option:{}", question.id)),
                    )
                } else {
                    (0..question.options.len()).collect()
                };
                SnapshotQuestion {
                    id: question.id.clone(),
                    prompt: question.prompt.clone(),
                    kind: question.kind,
                    explanation: question.explanation.clone(),
                    options: option_order
                        .into_iter()
                        .map(|option_index| {
                            let option = &question.options[option_index];
                            SnapshotOption {
                                id: option.id.clone(),
                                text: option.text.clone(),
                                is_correct: option.is_correct,
                            }
                        })
                        .collect(),
                }
            })
            .collect();

        Self {
            quiz_id: quiz.id.clone(),
            title: quiz.title.clone(),
            description: quiz.description.clone(),
            category: quiz.category.clone(),
            tags: quiz.tags.clone(),
            settings: quiz.settings.clone(),
            questions,
        }
    }

    pub fn score(&self, answers: &[SavedAnswer]) -> Score {
        let total = self.questions.len() as u32;
        let mut correct = 0u32;
        for question in &self.questions {
            let selected = answers
                .iter()
                .find(|answer| answer.question_id == question.id)
                .map(|answer| answer.selected.as_slice())
                .unwrap_or(&[]);
            if is_correct(question, selected) {
                correct += 1;
            }
        }
        let percent = if total == 0 {
            0.0
        } else {
            f64::from(correct) / f64::from(total) * 100.0
        };
        let passed = self
            .settings
            .pass_percent
            .map(|pass| percent >= f64::from(pass));
        Score {
            correct,
            total,
            percent,
            passed,
        }
    }

    pub fn view(&self, meta: &AttemptMeta, answers: &[SavedAnswer]) -> AttemptView {
        let questions = self
            .questions
            .iter()
            .map(|question| AttemptQuestionView {
                id: question.id.clone(),
                prompt: question.prompt.clone(),
                kind: question.kind,
                options: question
                    .options
                    .iter()
                    .map(|option| AttemptOptionView {
                        id: option.id.clone(),
                        text: option.text.clone(),
                    })
                    .collect(),
            })
            .collect();

        let answers = self
            .questions
            .iter()
            .map(|question| {
                answers
                    .iter()
                    .find(|answer| answer.question_id == question.id)
                    .cloned()
                    .unwrap_or_else(|| SavedAnswer {
                        question_id: question.id.clone(),
                        selected: Vec::new(),
                        flagged: false,
                    })
            })
            .collect();

        AttemptView {
            id: meta.id.clone(),
            quiz_id: meta.quiz_id.clone(),
            quiz_title: self.title.clone(),
            status: meta.status,
            started_at: meta.started_at,
            deadline_at: meta.deadline_at,
            submitted_at: meta.submitted_at,
            one_at_a_time: self.settings.one_at_a_time,
            total_questions: self.questions.len() as u32,
            questions,
            answers,
        }
    }

    pub fn review(&self, meta: &AttemptMeta, answers: &[SavedAnswer]) -> AttemptReview {
        let questions = self
            .questions
            .iter()
            .map(|question| {
                let selected = answers
                    .iter()
                    .find(|answer| answer.question_id == question.id)
                    .cloned()
                    .unwrap_or_else(|| SavedAnswer {
                        question_id: question.id.clone(),
                        selected: Vec::new(),
                        flagged: false,
                    });
                let correct = is_correct(question, &selected.selected);
                ReviewQuestion {
                    id: question.id.clone(),
                    prompt: question.prompt.clone(),
                    kind: question.kind,
                    explanation: question.explanation.clone(),
                    options: question
                        .options
                        .iter()
                        .map(|option| ReviewOption {
                            id: option.id.clone(),
                            text: option.text.clone(),
                            is_correct: option.is_correct,
                            selected: selected.selected.iter().any(|id| id == &option.id),
                        })
                        .collect(),
                    selected: selected.selected.clone(),
                    flagged: selected.flagged,
                    is_correct: correct,
                }
            })
            .collect();

        AttemptReview {
            attempt: meta.summary(),
            pass_percent: self.settings.pass_percent,
            questions,
        }
    }
}

pub fn is_correct(question: &SnapshotQuestion, selected: &[String]) -> bool {
    let correct_count = question.options.iter().filter(|option| option.is_correct).count();
    if selected.len() != correct_count {
        return false;
    }
    question
        .options
        .iter()
        .filter(|option| option.is_correct)
        .all(|option| selected.iter().any(|id| id == &option.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AnswerOption, Question, QuizSettings};

    fn quiz() -> Quiz {
        let mut quiz = Quiz::blank(0);
        quiz.title = "Test".into();
        quiz.questions = vec![Question {
            id: "q1".into(),
            prompt: "Pick two".into(),
            explanation: "Because".into(),
            kind: QuestionKind::Multi,
            options: vec![
                AnswerOption {
                    id: "a".into(),
                    text: "A".into(),
                    is_correct: true,
                },
                AnswerOption {
                    id: "b".into(),
                    text: "B".into(),
                    is_correct: true,
                },
                AnswerOption {
                    id: "c".into(),
                    text: "C".into(),
                    is_correct: false,
                },
            ],
        }];
        quiz
    }

    #[test]
    fn multi_select_is_all_or_nothing() {
        let snapshot = AttemptSnapshot::build(&quiz(), 1);
        let all = snapshot.score(&[SavedAnswer {
            question_id: "q1".into(),
            selected: vec!["a".into(), "b".into()],
            flagged: false,
        }]);
        assert_eq!(all.correct, 1);
        assert_eq!(all.percent, 100.0);

        let partial = snapshot.score(&[SavedAnswer {
            question_id: "q1".into(),
            selected: vec!["a".into()],
            flagged: false,
        }]);
        assert_eq!(partial.correct, 0);
    }

    #[test]
    fn shuffle_is_stored_in_snapshot() {
        let mut quiz = quiz();
        quiz.settings = QuizSettings {
            shuffle_questions: true,
            shuffle_options: true,
            ..Default::default()
        };
        let snapshot = AttemptSnapshot::build(&quiz, 7);
        assert_eq!(snapshot.questions.len(), 1);
        assert_eq!(snapshot.questions[0].options.len(), 3);
    }
}
