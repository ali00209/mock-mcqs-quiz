use rusqlite::Connection;

pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS quizzes (
    id                TEXT PRIMARY KEY NOT NULL,
    title             TEXT NOT NULL,
    description       TEXT NOT NULL DEFAULT '',
    category          TEXT,
    tags_json         TEXT NOT NULL DEFAULT '[]',
    time_limit_secs   INTEGER,
    shuffle_questions INTEGER NOT NULL DEFAULT 0,
    shuffle_options   INTEGER NOT NULL DEFAULT 0,
    pass_percent      INTEGER,
    one_at_a_time     INTEGER NOT NULL DEFAULT 1,
    source            TEXT NOT NULL DEFAULT 'user',
    sample_slug       TEXT UNIQUE,
    ready             INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS questions (
    id          TEXT PRIMARY KEY NOT NULL,
    quiz_id     TEXT NOT NULL REFERENCES quizzes(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    prompt      TEXT NOT NULL DEFAULT '',
    explanation TEXT NOT NULL DEFAULT '',
    kind        TEXT NOT NULL DEFAULT 'single'
);

CREATE INDEX IF NOT EXISTS questions_quiz_idx ON questions(quiz_id, position);

CREATE TABLE IF NOT EXISTS answer_options (
    id          TEXT PRIMARY KEY NOT NULL,
    question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    text        TEXT NOT NULL DEFAULT '',
    is_correct  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS answer_options_question_idx ON answer_options(question_id, position);

CREATE TABLE IF NOT EXISTS attempts (
    id             TEXT PRIMARY KEY NOT NULL,
    quiz_id        TEXT REFERENCES quizzes(id) ON DELETE SET NULL,
    quiz_title     TEXT NOT NULL,
    snapshot_json  TEXT NOT NULL,
    started_at     INTEGER NOT NULL,
    deadline_at    INTEGER,
    submitted_at   INTEGER,
    status         TEXT NOT NULL DEFAULT 'in_progress',
    correct_count  INTEGER,
    total_count    INTEGER NOT NULL DEFAULT 0,
    score_percent  REAL,
    passed         INTEGER,
    duration_ms    INTEGER
);

CREATE INDEX IF NOT EXISTS attempts_started_idx ON attempts(started_at DESC);
CREATE INDEX IF NOT EXISTS attempts_quiz_idx ON attempts(quiz_id, status);

CREATE TABLE IF NOT EXISTS attempt_answers (
    attempt_id    TEXT NOT NULL REFERENCES attempts(id) ON DELETE CASCADE,
    question_id   TEXT NOT NULL,
    selected_json TEXT NOT NULL DEFAULT '[]',
    flagged       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (attempt_id, question_id)
);
"#;

/// Version 2: answer feedback per quiz, and pause state per attempt.
const SCHEMA_V2: &str = r#"
ALTER TABLE quizzes ADD COLUMN feedback TEXT NOT NULL DEFAULT 'after_submit';
ALTER TABLE attempts ADD COLUMN paused INTEGER NOT NULL DEFAULT 0;
ALTER TABLE attempts ADD COLUMN remaining_ms INTEGER;
"#;

pub fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if version > SCHEMA_VERSION {
        return Err(
            "This database was created by a newer version of MockTest and cannot be opened here."
                .to_string(),
        );
    }
    if version < 1 {
        conn.execute_batch("BEGIN;")
            .map_err(|error| error.to_string())?;
        conn.execute_batch(SCHEMA_V1)
            .map_err(|error| error.to_string())?;
        conn.pragma_update(None, "user_version", 1)
            .map_err(|error| error.to_string())?;
        conn.execute_batch("COMMIT;")
            .map_err(|error| error.to_string())?;
    }
    if version < 2 {
        conn.execute_batch("BEGIN;")
            .map_err(|error| error.to_string())?;
        conn.execute_batch(SCHEMA_V2)
            .map_err(|error| error.to_string())?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|error| error.to_string())?;
        conn.execute_batch("COMMIT;")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
