use serde::{Deserialize, Serialize};

use crate::model::Quiz;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Command {
    Bootstrap,
    GetQuiz {
        id: String,
    },
    CreateQuiz,
    SaveQuiz {
        quiz: Quiz,
    },
    DeleteQuiz {
        id: String,
    },
    DuplicateQuiz {
        id: String,
    },
    ImportQuiz {
        json: String,
    },
    ExportQuiz {
        id: String,
    },
    StartAttempt {
        quiz_id: String,
        seed: u64,
    },
    GetAttempt {
        id: String,
    },
    SaveAnswer {
        attempt_id: String,
        question_id: String,
        selected: Vec<String>,
        flagged: bool,
    },
    SubmitAttempt {
        id: String,
    },
    GetReview {
        id: String,
    },
    DeleteAttempt {
        id: String,
    },
    PauseAttempt {
        id: String,
    },
    ResumeAttempt {
        id: String,
    },
    ClearHistory,
    BackupDatabase,
    RestoreDatabase {
        data_base64: String,
    },
    ResetData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub id: u64,
    pub command: Command,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}
