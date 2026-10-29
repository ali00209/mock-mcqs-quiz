use std::collections::HashSet;
use std::time::Duration;

use mocktest_domain::{
    AttemptMeta, AttemptReview, AttemptSnapshot, AttemptStatus, AttemptSummary, AttemptView,
    BootstrapData, Command, QuestionKind, Quiz, QuizSettings, QuizSource,
    QuizSummary, SavedAnswer, SaveAnswerResult, export_quiz, import_quiz, new_id, read_pack,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value;

use crate::schema;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("The database could not be read: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("The stored data is damaged: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Pack(#[from] mocktest_domain::PackError),
    #[error("{0}")]
    Migration(String),
}

pub fn open(path: &str) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    schema::migrate(&conn).map_err(DbError::Migration)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection, DbError> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    schema::migrate(&conn).map_err(DbError::Migration)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.busy_timeout(Duration::from_millis(2_500))?;
    Ok(())
}

pub fn dispatch(conn: &Connection, command: Command, now: i64) -> Result<Value, DbError> {
    finalize_expired(conn, now)?;
    match command {
        Command::Bootstrap => Ok(serde_json::to_value(BootstrapData {
            quizzes: list_quizzes(conn)?,
            attempts: list_attempts(conn)?,
        })?),
        Command::GetQuiz { id } => Ok(serde_json::to_value(load_quiz(conn, &id)?)?),
        Command::CreateQuiz => Ok(serde_json::to_value(create_quiz(conn, now)?)?),
        Command::SaveQuiz { quiz } => Ok(serde_json::to_value(save_quiz(conn, quiz, now)?)?),
        Command::DeleteQuiz { id } => {
            delete_quiz(conn, &id)?;
            Ok(Value::Null)
        }
        Command::DuplicateQuiz { id } => Ok(serde_json::to_value(duplicate_quiz(conn, &id, now)?)?),
        Command::ImportQuiz { json } => {
            Ok(serde_json::to_value(import_into_library(conn, &json, now)?)?)
        }
        Command::ExportQuiz { id } => {
            let quiz = load_quiz(conn, &id)?;
            Ok(Value::String(export_quiz(&quiz, now)?))
        }
        Command::StartAttempt { quiz_id, seed } => {
            Ok(serde_json::to_value(start_attempt(conn, &quiz_id, seed, now)?)?)
        }
        Command::GetAttempt { id } => Ok(serde_json::to_value(get_attempt(conn, &id, now)?)?),
        Command::SaveAnswer {
            attempt_id,
            question_id,
            selected,
            flagged,
        } => Ok(serde_json::to_value(save_answer(
            conn,
            &attempt_id,
            &question_id,
            selected,
            flagged,
            now,
        )?)?),
        Command::SubmitAttempt { id } => {
            Ok(serde_json::to_value(submit_attempt(conn, &id, now)?)?)
        }
        Command::GetReview { id } => Ok(serde_json::to_value(get_review(conn, &id, now)?)?),
        Command::DeleteAttempt { id } => {
            delete_attempt(conn, &id)?;
            Ok(Value::Null)
        }
        Command::ClearHistory => {
            clear_history(conn)?;
            Ok(Value::Null)
        }
        Command::BackupDatabase | Command::RestoreDatabase { .. } | Command::ResetData => {
            Err(DbError::Invalid(
                "This operation is handled by the database worker.".to_string(),
            ))
        }
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn unique_title(conn: &Connection, base: &str) -> Result<String, DbError> {
    let base = base.trim();
    let base = if base.is_empty() { "Untitled quiz" } else { base };
    for n in 1..10_000usize {
        let suffix = if n == 1 {
            " (copy)".to_string()
        } else {
            format!(" (copy {n})")
        };
        let room = mocktest_domain::MAX_TITLE_CHARS.saturating_sub(suffix.chars().count());
        let candidate = format!("{}{}", truncate_chars(base, room).trim_end(), suffix);
        let exists: i64 = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM quizzes WHERE title = ?1 COLLATE NOCASE)",
            [&candidate],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(candidate);
        }
    }
    Ok(truncate_chars(base, mocktest_domain::MAX_TITLE_CHARS))
}

fn tags_from_json(value: &str) -> Vec<String> {
    serde_json::from_str(value).unwrap_or_default()
}

fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

fn quiz_from_row(row: &Row<'_>) -> rusqlite::Result<Quiz> {
    let time_limit: Option<i64> = row.get(5)?;
    let pass_percent: Option<i64> = row.get(8)?;
    let source: String = row.get(10)?;
    Ok(Quiz {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        category: row.get(3)?,
        tags: tags_from_json(&row.get::<_, String>(4)?),
        settings: QuizSettings {
            time_limit_secs: time_limit.map(|value| value.max(0) as u32),
            shuffle_questions: row.get::<_, i64>(6)? != 0,
            shuffle_options: row.get::<_, i64>(7)? != 0,
            pass_percent: pass_percent.map(|value| value.clamp(0, 100) as u8),
            one_at_a_time: row.get::<_, i64>(9)? != 0,
        },
        questions: Vec::new(),
        source: if source == "sample" {
            QuizSource::Sample
        } else {
            QuizSource::User
        },
        sample_slug: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn load_quiz(conn: &Connection, id: &str) -> Result<Quiz, DbError> {
    let mut quiz = conn
        .query_row(
            "SELECT id, title, description, category, tags_json, time_limit_secs,
                    shuffle_questions, shuffle_options, pass_percent, one_at_a_time,
                    source, sample_slug, created_at, updated_at
             FROM quizzes WHERE id = ?1",
            [id],
            quiz_from_row,
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound("That quiz no longer exists.".to_string()))?;

    let mut question_stmt = conn.prepare(
        "SELECT id, prompt, explanation, kind FROM questions
         WHERE quiz_id = ?1 ORDER BY position, id",
    )?;
    let question_rows = question_stmt
        .query_map([id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (question_id, prompt, explanation, kind) in question_rows {
        let mut option_stmt = conn.prepare(
            "SELECT id, text, is_correct FROM answer_options
             WHERE question_id = ?1 ORDER BY position, id",
        )?;
        let options = option_stmt
            .query_map([&question_id], |row| {
                Ok(mocktest_domain::AnswerOption {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    is_correct: row.get::<_, i64>(2)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        quiz.questions.push(mocktest_domain::Question {
            id: question_id,
            prompt,
            explanation,
            kind: if kind == "multi" {
                QuestionKind::Multi
            } else {
                QuestionKind::Single
            },
            options,
        });
    }
    Ok(quiz)
}

fn insert_questions(conn: &Connection, quiz: &Quiz) -> Result<(), DbError> {
    let mut question_stmt = conn.prepare(
        "INSERT INTO questions (id, quiz_id, position, prompt, explanation, kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut option_stmt = conn.prepare(
        "INSERT INTO answer_options (id, question_id, position, text, is_correct)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (question_position, question) in quiz.questions.iter().enumerate() {
        question_stmt.execute(params![
            question.id,
            quiz.id,
            question_position as i64,
            question.prompt,
            question.explanation,
            match question.kind {
                QuestionKind::Single => "single",
                QuestionKind::Multi => "multi",
            }
        ])?;
        for (option_position, option) in question.options.iter().enumerate() {
            option_stmt.execute(params![
                option.id,
                question.id,
                option_position as i64,
                option.text,
                i64::from(option.is_correct as u8)
            ])?;
        }
    }
    Ok(())
}

fn insert_quiz(conn: &Connection, quiz: &Quiz) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO quizzes (
            id, title, description, category, tags_json, time_limit_secs,
            shuffle_questions, shuffle_options, pass_percent, one_at_a_time,
            source, sample_slug, ready, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            quiz.id,
            quiz.title,
            quiz.description,
            quiz.category,
            tags_to_json(&quiz.tags),
            quiz.settings.time_limit_secs.map(i64::from),
            i64::from(quiz.settings.shuffle_questions as u8),
            i64::from(quiz.settings.shuffle_options as u8),
            quiz.settings.pass_percent.map(i64::from),
            i64::from(quiz.settings.one_at_a_time as u8),
            match quiz.source {
                QuizSource::User => "user",
                QuizSource::Sample => "sample",
            },
            quiz.sample_slug,
            i64::from(quiz.is_ready()),
            quiz.created_at,
            quiz.updated_at,
        ],
    )?;
    insert_questions(conn, quiz)?;
    Ok(())
}

fn update_quiz(conn: &Connection, quiz: &Quiz) -> Result<(), DbError> {
    let changed = conn.execute(
        "UPDATE quizzes SET
            title = ?2, description = ?3, category = ?4, tags_json = ?5,
            time_limit_secs = ?6, shuffle_questions = ?7, shuffle_options = ?8,
            pass_percent = ?9, one_at_a_time = ?10, ready = ?11, updated_at = ?12
         WHERE id = ?1",
        params![
            quiz.id,
            quiz.title,
            quiz.description,
            quiz.category,
            tags_to_json(&quiz.tags),
            quiz.settings.time_limit_secs.map(i64::from),
            i64::from(quiz.settings.shuffle_questions as u8),
            i64::from(quiz.settings.shuffle_options as u8),
            quiz.settings.pass_percent.map(i64::from),
            i64::from(quiz.settings.one_at_a_time as u8),
            i64::from(quiz.is_ready()),
            quiz.updated_at,
        ],
    )?;
    if changed == 0 {
        return Err(DbError::NotFound(
            "That quiz no longer exists.".to_string(),
        ));
    }
    conn.execute("DELETE FROM questions WHERE quiz_id = ?1", [&quiz.id])?;
    insert_questions(conn, quiz)?;
    Ok(())
}

fn create_quiz(conn: &Connection, now: i64) -> Result<Quiz, DbError> {
    let mut quiz = Quiz::blank(now).normalized();
    quiz.title = unique_title(conn, &quiz.title)?;
    let tx = conn.unchecked_transaction()?;
    insert_quiz(&tx, &quiz)?;
    tx.commit()?;
    Ok(quiz)
}

fn save_quiz(conn: &Connection, quiz: Quiz, now: i64) -> Result<Quiz, DbError> {
    let existing = load_quiz(conn, &quiz.id)?;
    if existing.source == QuizSource::Sample {
        return Err(DbError::Invalid(
            "Sample quizzes are read-only. Duplicate one to edit it.".to_string(),
        ));
    }
    let mut quiz = quiz.normalized();
    quiz.created_at = existing.created_at;
    quiz.updated_at = now;
    quiz.source = existing.source;
    quiz.sample_slug = existing.sample_slug;
    let tx = conn.unchecked_transaction()?;
    update_quiz(&tx, &quiz)?;
    tx.commit()?;
    Ok(quiz)
}

fn delete_quiz(conn: &Connection, id: &str) -> Result<(), DbError> {
    let quiz = load_quiz(conn, id)?;
    if quiz.source == QuizSource::Sample {
        return Err(DbError::Invalid(
            "Sample quizzes are built in and cannot be deleted.".to_string(),
        ));
    }
    conn.execute("DELETE FROM quizzes WHERE id = ?1", [id])?;
    Ok(())
}

fn duplicate_quiz(conn: &Connection, id: &str, now: i64) -> Result<Quiz, DbError> {
    let source = load_quiz(conn, id)?;
    let mut copy = source.duplicated(now);
    copy.title = unique_title(conn, &copy.title)?;
    let tx = conn.unchecked_transaction()?;
    insert_quiz(&tx, &copy)?;
    tx.commit()?;
    Ok(copy)
}

fn import_into_library(conn: &Connection, json: &str, now: i64) -> Result<Quiz, DbError> {
    let mut quiz = import_quiz(json, now)?;
    quiz.title = unique_title(conn, &quiz.title)?;
    let tx = conn.unchecked_transaction()?;
    insert_quiz(&tx, &quiz)?;
    tx.commit()?;
    Ok(quiz)
}

fn list_quizzes(conn: &Connection) -> Result<Vec<QuizSummary>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT q.id, q.title, q.description, q.category, q.tags_json,
                (SELECT COUNT(*) FROM questions WHERE quiz_id = q.id),
                q.time_limit_secs, q.pass_percent, q.source, q.ready, q.updated_at,
                (SELECT COUNT(*) FROM attempts a
                    WHERE a.quiz_id = q.id AND a.status != 'in_progress'),
                (SELECT MAX(a.score_percent) FROM attempts a
                    WHERE a.quiz_id = q.id AND a.status != 'in_progress'),
                (SELECT a.id FROM attempts a
                    WHERE a.quiz_id = q.id AND a.status = 'in_progress'
                    ORDER BY a.started_at DESC LIMIT 1)
         FROM quizzes q
         ORDER BY CASE q.source WHEN 'user' THEN 0 ELSE 1 END, q.updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let time_limit: Option<i64> = row.get(6)?;
            let pass: Option<i64> = row.get(7)?;
            let source: String = row.get(8)?;
            Ok(QuizSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                category: row.get(3)?,
                tags: tags_from_json(&row.get::<_, String>(4)?),
                question_count: row.get::<_, i64>(5)?.max(0) as u32,
                time_limit_secs: time_limit.map(|value| value.max(0) as u32),
                pass_percent: pass.map(|value| value.clamp(0, 100) as u8),
                source: if source == "sample" {
                    QuizSource::Sample
                } else {
                    QuizSource::User
                },
                ready: row.get::<_, i64>(9)? != 0,
                updated_at: row.get(10)?,
                attempts_count: row.get::<_, i64>(11)?.max(0) as u32,
                best_percent: row.get(12)?,
                active_attempt_id: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn attempt_meta_from_row(row: &Row<'_>) -> rusqlite::Result<AttemptMeta> {
    let status: String = row.get(3)?;
    Ok(AttemptMeta {
        id: row.get(0)?,
        quiz_id: row.get(1)?,
        quiz_title: row.get(2)?,
        status: AttemptStatus::from_db(&status),
        started_at: row.get(4)?,
        deadline_at: row.get(5)?,
        submitted_at: row.get(6)?,
        correct_count: row.get::<_, Option<i64>>(7)?.map(|value| value.max(0) as u32),
        total_count: row.get::<_, i64>(8)?.max(0) as u32,
        score_percent: row.get(9)?,
        passed: row.get::<_, Option<i64>>(10)?.map(|value| value != 0),
        duration_ms: row.get(11)?,
    })
}

const ATTEMPT_COLUMNS: &str = "id, quiz_id, quiz_title, status, started_at, deadline_at,
    submitted_at, correct_count, total_count, score_percent, passed, duration_ms";

fn load_attempt_meta(conn: &Connection, id: &str) -> Result<AttemptMeta, DbError> {
    conn.query_row(
        &format!("SELECT {ATTEMPT_COLUMNS} FROM attempts WHERE id = ?1"),
        [id],
        attempt_meta_from_row,
    )
    .optional()?
    .ok_or_else(|| DbError::NotFound("That attempt no longer exists.".to_string()))
}

fn list_attempts(conn: &Connection) -> Result<Vec<AttemptSummary>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ATTEMPT_COLUMNS} FROM attempts ORDER BY started_at DESC"
    ))?;
    let rows = stmt
        .query_map([], |row| Ok(attempt_meta_from_row(row)?.summary()))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_snapshot(conn: &Connection, attempt_id: &str) -> Result<AttemptSnapshot, DbError> {
    let json: String = conn
        .query_row(
            "SELECT snapshot_json FROM attempts WHERE id = ?1",
            [attempt_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound("That attempt no longer exists.".to_string()))?;
    Ok(serde_json::from_str(&json)?)
}

fn load_answers(conn: &Connection, attempt_id: &str) -> Result<Vec<SavedAnswer>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT question_id, selected_json, flagged FROM attempt_answers
         WHERE attempt_id = ?1",
    )?;
    let rows = stmt
        .query_map([attempt_id], |row| {
            Ok(SavedAnswer {
                question_id: row.get(0)?,
                selected: serde_json::from_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                flagged: row.get::<_, i64>(2)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_attempt_bundle(
    conn: &Connection,
    id: &str,
) -> Result<(AttemptMeta, AttemptSnapshot, Vec<SavedAnswer>), DbError> {
    let meta = load_attempt_meta(conn, id)?;
    let snapshot = load_snapshot(conn, id)?;
    let answers = load_answers(conn, id)?;
    Ok((meta, snapshot, answers))
}

fn active_attempt_for_quiz(conn: &Connection, quiz_id: &str) -> Result<Option<String>, DbError> {
    Ok(conn
        .query_row(
            "SELECT id FROM attempts
             WHERE quiz_id = ?1 AND status = 'in_progress'
             ORDER BY started_at DESC LIMIT 1",
            [quiz_id],
            |row| row.get(0),
        )
        .optional()?)
}

fn start_attempt(
    conn: &Connection,
    quiz_id: &str,
    seed: u64,
    now: i64,
) -> Result<AttemptView, DbError> {
    if let Some(existing) = active_attempt_for_quiz(conn, quiz_id)? {
        let (meta, snapshot, answers) = load_attempt_bundle(conn, &existing)?;
        return Ok(snapshot.view(&meta, &answers));
    }
    let quiz = load_quiz(conn, quiz_id)?;
    if let Some(issue) = quiz.validation_issues().first() {
        return Err(DbError::Invalid(format!(
            "This quiz is not ready: {}",
            issue.message
        )));
    }
    let snapshot = AttemptSnapshot::build(&quiz, seed);
    let id = new_id();
    let deadline = quiz
        .settings
        .time_limit_secs
        .map(|seconds| now + i64::from(seconds) * 1_000);
    conn.execute(
        "INSERT INTO attempts (
            id, quiz_id, quiz_title, snapshot_json, started_at, deadline_at,
            status, total_count
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_progress', ?7)",
        params![
            id,
            quiz.id,
            snapshot.title,
            serde_json::to_string(&snapshot)?,
            now,
            deadline,
            snapshot.questions.len() as i64,
        ],
    )?;
    let meta = load_attempt_meta(conn, &id)?;
    Ok(snapshot.view(&meta, &[]))
}

fn get_attempt(conn: &Connection, id: &str, now: i64) -> Result<AttemptView, DbError> {
    finalize_expired(conn, now)?;
    let (meta, snapshot, answers) = load_attempt_bundle(conn, id)?;
    Ok(snapshot.view(&meta, &answers))
}

fn save_answer(
    conn: &Connection,
    attempt_id: &str,
    question_id: &str,
    selected: Vec<String>,
    flagged: bool,
    now: i64,
) -> Result<SaveAnswerResult, DbError> {
    let meta = load_attempt_meta(conn, attempt_id)?;
    if meta.status != AttemptStatus::InProgress {
        return Ok(SaveAnswerResult {
            auto_submitted: true,
        });
    }
    let snapshot = load_snapshot(conn, attempt_id)?;
    let question = snapshot
        .questions
        .iter()
        .find(|question| question.id == question_id)
        .ok_or_else(|| DbError::Invalid("That question is not part of this attempt.".to_string()))?;
    let valid: HashSet<&str> = question.options.iter().map(|option| option.id.as_str()).collect();
    let mut selected = selected;
    selected.retain(|id| valid.contains(id.as_str()));
    selected.sort();
    selected.dedup();
    let selected_json = serde_json::to_string(&selected)?;
    conn.execute(
        "INSERT INTO attempt_answers (attempt_id, question_id, selected_json, flagged)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(attempt_id, question_id) DO UPDATE SET
            selected_json = excluded.selected_json,
            flagged = excluded.flagged",
        params![attempt_id, question_id, selected_json, i64::from(flagged as u8)],
    )?;

    if let Some(deadline) = meta.deadline_at
        && deadline <= now
    {
        finish_attempt(conn, attempt_id, now)?;
        return Ok(SaveAnswerResult {
            auto_submitted: true,
        });
    }
    Ok(SaveAnswerResult {
        auto_submitted: false,
    })
}

fn finish_attempt(conn: &Connection, attempt_id: &str, now: i64) -> Result<(), DbError> {
    let meta = load_attempt_meta(conn, attempt_id)?;
    if meta.status != AttemptStatus::InProgress {
        return Ok(());
    }
    let snapshot = load_snapshot(conn, attempt_id)?;
    let answers = load_answers(conn, attempt_id)?;
    let score = snapshot.score(&answers);
    let expired = meta.deadline_at.is_some_and(|deadline| deadline <= now);
    let submitted_at = if expired {
        meta.deadline_at.unwrap_or(now)
    } else {
        now
    };
    let duration = (submitted_at - meta.started_at).max(0);
    conn.execute(
        "UPDATE attempts SET
            status = ?2, submitted_at = ?3, correct_count = ?4, total_count = ?5,
            score_percent = ?6, passed = ?7, duration_ms = ?8
         WHERE id = ?1",
        params![
            attempt_id,
            if expired { "expired" } else { "submitted" },
            submitted_at,
            i64::from(score.correct),
            i64::from(score.total),
            score.percent,
            score.passed.map(|value| value as i64),
            duration,
        ],
    )?;
    Ok(())
}

fn finalize_expired(conn: &Connection, now: i64) -> Result<(), DbError> {
    let ids = {
        let mut stmt = conn.prepare(
            "SELECT id FROM attempts
             WHERE status = 'in_progress' AND deadline_at IS NOT NULL AND deadline_at <= ?1",
        )?;
        stmt.query_map([now], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for id in ids {
        finish_attempt(conn, &id, now)?;
    }
    Ok(())
}

fn submit_attempt(conn: &Connection, id: &str, now: i64) -> Result<AttemptReview, DbError> {
    finalize_expired(conn, now)?;
    finish_attempt(conn, id, now)?;
    let (meta, snapshot, answers) = load_attempt_bundle(conn, id)?;
    if meta.status == AttemptStatus::InProgress {
        return Err(DbError::Invalid(
            "The attempt could not be submitted.".to_string(),
        ));
    }
    Ok(snapshot.review(&meta, &answers))
}

fn get_review(conn: &Connection, id: &str, now: i64) -> Result<AttemptReview, DbError> {
    finalize_expired(conn, now)?;
    let (meta, snapshot, answers) = load_attempt_bundle(conn, id)?;
    if meta.status == AttemptStatus::InProgress {
        return Err(DbError::Invalid(
            "Submit the attempt before reviewing it.".to_string(),
        ));
    }
    Ok(snapshot.review(&meta, &answers))
}

/// Removes every attempt and its answers. Quizzes and their questions stay.
fn clear_history(conn: &Connection) -> Result<(), DbError> {
    let tx = conn.unchecked_transaction()?;
    // Answers first, so no row is left pointing at a deleted attempt.
    tx.execute("DELETE FROM attempt_answers", [])?;
    tx.execute("DELETE FROM attempts", [])?;
    tx.commit()?;
    Ok(())
}

fn delete_attempt(conn: &Connection, id: &str) -> Result<(), DbError> {
    let changed = conn.execute("DELETE FROM attempts WHERE id = ?1", [id])?;
    if changed == 0 {
        return Err(DbError::NotFound(
            "That attempt no longer exists.".to_string(),
        ));
    }
    Ok(())
}

pub fn seed_samples(conn: &Connection, samples: &[(&str, &str)], now: i64) -> Result<(), DbError> {
    for (slug, json) in samples {
        seed_sample(conn, slug, json, now)?;
    }
    Ok(())
}

pub fn seed_sample(conn: &Connection, slug: &str, json: &str, now: i64) -> Result<(), DbError> {
    let mut quiz = read_pack(json)?;
    quiz.id = format!("sample-{slug}");
    quiz.source = QuizSource::Sample;
    quiz.sample_slug = Some(slug.to_string());
    quiz.created_at = now;
    quiz.updated_at = now;

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO quizzes (
            id, title, description, category, tags_json, time_limit_secs,
            shuffle_questions, shuffle_options, pass_percent, one_at_a_time,
            source, sample_slug, ready, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'sample', ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            description = excluded.description,
            category = excluded.category,
            tags_json = excluded.tags_json,
            time_limit_secs = excluded.time_limit_secs,
            shuffle_questions = excluded.shuffle_questions,
            shuffle_options = excluded.shuffle_options,
            pass_percent = excluded.pass_percent,
            one_at_a_time = excluded.one_at_a_time,
            source = 'sample',
            sample_slug = excluded.sample_slug,
            ready = excluded.ready,
            updated_at = excluded.updated_at",
        params![
            quiz.id,
            quiz.title,
            quiz.description,
            quiz.category,
            tags_to_json(&quiz.tags),
            quiz.settings.time_limit_secs.map(i64::from),
            i64::from(quiz.settings.shuffle_questions as u8),
            i64::from(quiz.settings.shuffle_options as u8),
            quiz.settings.pass_percent.map(i64::from),
            i64::from(quiz.settings.one_at_a_time as u8),
            slug,
            i64::from(quiz.is_ready()),
            quiz.created_at,
            quiz.updated_at,
        ],
    )?;
    tx.execute("DELETE FROM questions WHERE quiz_id = ?1", [&quiz.id])?;
    insert_questions(&tx, &quiz)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mocktest_domain::{AnswerOption, Question};

    const PACK: &str = r#"{
      "format": "mocktest.quizpack",
      "version": 1,
      "exportedAt": 0,
      "quiz": {
        "id": "sample-demo",
        "title": "Demo",
        "description": "A demo",
        "category": "Test",
        "tags": ["demo"],
        "settings": {
          "timeLimitSecs": null,
          "shuffleQuestions": false,
          "shuffleOptions": false,
          "passPercent": 50,
          "oneAtATime": true
        },
        "questions": [
          {
            "id": "sample-demo-q1",
            "prompt": "2 + 2?",
            "explanation": "Four.",
            "kind": "single",
            "options": [
              {"id": "sample-demo-q1-a", "text": "4", "isCorrect": true},
              {"id": "sample-demo-q1-b", "text": "5", "isCorrect": false}
            ]
          }
        ],
        "source": "sample",
        "sampleSlug": "demo",
        "createdAt": 0,
        "updatedAt": 0
      }
    }"#;

    fn seeded() -> Connection {
        let conn = open_in_memory().unwrap();
        seed_sample(&conn, "demo", PACK, 1_000).unwrap();
        conn
    }

    #[test]
    fn seed_is_idempotent() {
        let conn = seeded();
        seed_sample(&conn, "demo", PACK, 2_000).unwrap();
        let data: BootstrapData = serde_json::from_value(
            dispatch(&conn, Command::Bootstrap, 2_000).unwrap(),
        )
        .unwrap();
        assert_eq!(data.quizzes.len(), 1);
        assert_eq!(data.quizzes[0].source, QuizSource::Sample);
        assert_eq!(data.quizzes[0].question_count, 1);
    }

    #[test]
    fn create_take_and_review_flow() {
        let conn = seeded();
        let _quiz = mocktest_domain::Quiz {
            id: "live".into(),
            title: "Live".into(),
            description: String::new(),
            category: None,
            tags: vec![],
            settings: QuizSettings {
                pass_percent: Some(50),
                ..Default::default()
            },
            questions: vec![Question {
                id: "q1".into(),
                prompt: "Pick A".into(),
                explanation: "A is right".into(),
                kind: QuestionKind::Single,
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
            }],
            source: QuizSource::User,
            sample_slug: None,
            created_at: 10,
            updated_at: 10,
        };
        let mut quiz = create_quiz(&conn, 20).unwrap();
        quiz.title = "Live".into();
        quiz.questions = vec![Question {
            id: "q1".into(),
            prompt: "Pick A".into(),
            explanation: "A is right".into(),
            kind: QuestionKind::Single,
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
        quiz.source = QuizSource::User;
        quiz.sample_slug = None;
        quiz.settings.pass_percent = Some(50);
        let saved = save_quiz(&conn, quiz, 20).unwrap();
        assert!(saved.is_ready());
        let saved_id = saved.id.clone();
        let view: AttemptView = serde_json::from_value(
            dispatch(
                &conn,
                Command::StartAttempt {
                    quiz_id: saved_id.clone(),
                    seed: 3,
                },
                30,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(view.questions.len(), 1);
        let result: SaveAnswerResult = serde_json::from_value(
            dispatch(
                &conn,
                Command::SaveAnswer {
                    attempt_id: view.id.clone(),
                    question_id: "q1".into(),
                    selected: vec!["a".into()],
                    flagged: false,
                },
                40,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!result.auto_submitted);
        let review: AttemptReview = serde_json::from_value(
            dispatch(
                &conn,
                Command::SubmitAttempt {
                    id: view.id.clone(),
                },
                50,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(review.attempt.correct_count, Some(1));
        assert_eq!(review.attempt.score_percent, Some(100.0));
        assert_eq!(review.attempt.passed, Some(true));
        assert_eq!(review.questions.len(), 1);
    }

    #[test]
    fn deleting_quiz_keeps_attempt_history() {
        let conn = seeded();
        let data: BootstrapData =
            serde_json::from_value(dispatch(&conn, Command::Bootstrap, 1_100).unwrap()).unwrap();
        let quiz_id = data.quizzes[0].id.clone();
        let view: AttemptView = serde_json::from_value(
            dispatch(
                &conn,
                Command::StartAttempt {
                    quiz_id: quiz_id.clone(),
                    seed: 1,
                },
                1_200,
            )
            .unwrap(),
        )
        .unwrap();
        dispatch(
            &conn,
            Command::SaveAnswer {
                attempt_id: view.id.clone(),
                question_id: "sample-demo-q1".into(),
                selected: vec!["sample-demo-q1-a".into()],
                flagged: false,
            },
            1_300,
        )
        .unwrap();
        dispatch(&conn, Command::SubmitAttempt { id: view.id.clone() }, 1_400).unwrap();
        // Samples cannot be deleted, so make a user copy first.
        let copy: Quiz =
            serde_json::from_value(dispatch(&conn, Command::DuplicateQuiz { id: quiz_id }, 1_500).unwrap())
                .unwrap();
        let copy_view: AttemptView = serde_json::from_value(
            dispatch(
                &conn,
                Command::StartAttempt {
                    quiz_id: copy.id.clone(),
                    seed: 2,
                },
                1_600,
            )
            .unwrap(),
        )
        .unwrap();
        dispatch(&conn, Command::DeleteQuiz { id: copy.id }, 1_700).unwrap();
        let data: BootstrapData =
            serde_json::from_value(dispatch(&conn, Command::Bootstrap, 1_800).unwrap()).unwrap();
        assert!(data.quizzes.iter().all(|quiz| quiz.source == QuizSource::Sample));
        assert!(data.attempts.iter().any(|attempt| attempt.id == copy_view.id));
        assert_eq!(
            data.attempts
                .iter()
                .find(|attempt| attempt.id == copy_view.id)
                .unwrap()
                .quiz_id,
            None
        );
    }

    #[test]
    fn expired_attempt_auto_submits() {
        let conn = seeded();
        let mut quiz = create_quiz(&conn, 2_000).unwrap();
        quiz.title = "Timed".into();
        quiz.questions = vec![Question {
            id: "q1".into(),
            prompt: "Pick A".into(),
            explanation: "A is right".into(),
            kind: QuestionKind::Single,
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
        quiz.source = QuizSource::User;
        quiz.sample_slug = None;
        quiz.settings.time_limit_secs = Some(60);
        quiz.settings.pass_percent = None;
        let saved = save_quiz(&conn, quiz, 2_000).unwrap();
        let saved_id = saved.id.clone();
        let view: AttemptView = serde_json::from_value(
            dispatch(
                &conn,
                Command::StartAttempt {
                    quiz_id: saved_id.clone(),
                    seed: 4,
                },
                3_000,
            )
            .unwrap(),
        )
        .unwrap();
        let data: BootstrapData = serde_json::from_value(
            dispatch(&conn, Command::Bootstrap, 3_000 + 61_000).unwrap(),
        )
        .unwrap();
        let attempt = data
            .attempts
            .iter()
            .find(|attempt| attempt.id == view.id)
            .unwrap();
        assert_eq!(attempt.status, AttemptStatus::Expired);
        assert_eq!(attempt.correct_count, Some(0));
    }
}
