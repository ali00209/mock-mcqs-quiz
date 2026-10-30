use std::cell::RefCell;
use std::rc::Rc;

use base64::Engine;
use console_error_panic_hook::set_once;
use mocktest_db::{dispatch, open, seed_samples};
use mocktest_domain::{Command, Request, Response};
use rusqlite::Connection;
use serde_json::Value;
use sqlite_wasm_rs::WasmOsCallback;
use rsqlite_vfs::transfer::DbTransfer;
use sqlite_wasm_vfs::sahpool::{OpfsSAHPoolCfg, OpfsSAHPoolUtil, install};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use web_sys::DedicatedWorkerGlobalScope;

const DB_NAME: &str = "mocktest.db";
const RESTORE_PREFIX: &str = "mocktest_restore_";

// Built-in sample quizzes. Empty for now: to restore them, add the JSON packs
// under <repo>/samples/ and list them here with include_str!, e.g.
//   ("general-knowledge", include_str!("../../../samples/general_knowledge.json")),
const SAMPLES: &[(&str, &str)] = &[];

struct WorkerState {
    connection: Option<Connection>,
    pool: Option<OpfsSAHPoolUtil>,
    scope: Rc<DedicatedWorkerGlobalScope>,
}

fn worker_scope() -> Rc<DedicatedWorkerGlobalScope> {
    Rc::new(js_sys::global().unchecked_into::<DedicatedWorkerGlobalScope>())
}

thread_local! {
    static STATE: Rc<RefCell<WorkerState>> = Rc::new(RefCell::new(WorkerState {
        connection: None,
        pool: None,
        scope: worker_scope(),
    }));
}

fn state() -> Rc<RefCell<WorkerState>> {
    STATE.with(|value| value.clone())
}

fn unavailable() -> String {
    "Database storage is unavailable.".to_string()
}

fn post_message(state: &Rc<RefCell<WorkerState>>, message: &str) {
    let scope = state.borrow().scope.clone();
    let value = JsValue::from_str(message);
    let _ = scope.post_message(&value);
}

fn respond(state: &Rc<RefCell<WorkerState>>, request_id: u64, result: Result<Value, String>) {
    let response = match result {
        Ok(data) => Response {
            id: request_id,
            ok: true,
            data: Some(data),
            error: None,
        },
        Err(error) => Response {
            id: request_id,
            ok: false,
            data: None,
            error: Some(error),
        },
    };
    let message = serde_json::json!({
        "id": response.id,
        "ok": response.ok,
        "data": response.data,
        "error": response.error,
    });
    post_message(state, &message.to_string());
}

/// Drops the live connection so the database file is not in use.
fn close_connection(state: &Rc<RefCell<WorkerState>>) {
    let old = state.borrow_mut().connection.take();
    drop(old);
}

fn reopen_connection(state: &Rc<RefCell<WorkerState>>) -> Result<(), String> {
    let connection = open(DB_NAME).map_err(|error| error.to_string())?;
    state.borrow_mut().connection = Some(connection);
    Ok(())
}

fn backup_database(state: &Rc<RefCell<WorkerState>>) -> Result<Value, String> {
    close_connection(state);
    let exported: Result<Vec<u8>, String> = {
        let state_ref = state.borrow();
        let pool = state_ref.pool.as_ref().ok_or_else(unavailable)?;
        pool.export_db(DB_NAME).map_err(|error| error.to_string())
    };
    // Reopen even if the export failed, so the app keeps working.
    reopen_connection(state)?;
    let bytes = exported?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    serde_json::to_value(encoded).map_err(|error| error.to_string())
}

fn restore_database(state: &Rc<RefCell<WorkerState>>, data_base64: &str) -> Result<Value, String> {
    close_connection(state);
    let outcome = restore_bytes(state, data_base64);
    reopen_connection(state)?;
    outcome
}

fn restore_bytes(state: &Rc<RefCell<WorkerState>>, data_base64: &str) -> Result<Value, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|error| error.to_string())?;
    let restore_name = format!("{RESTORE_PREFIX}{}", js_sys::Date::now() as i64);
    {
        let state_ref = state.borrow();
        let pool = state_ref.pool.as_ref().ok_or_else(unavailable)?;
        pool.import_db(&restore_name, &bytes)
            .map_err(|error| error.to_string())?;
    }
    // Opening the staged file also checks that it is a valid MockTest database.
    drop(open(&restore_name).map_err(|error| error.to_string())?);

    let (quizzes, attempts) = copy_restore_into_main(&restore_name)?;
    Ok(serde_json::json!({ "ok": true, "quizzes": quizzes, "attempts": attempts }))
}

/// Replaces the contents of the main database with the staged restore file.
/// Runs in one transaction, so a failure leaves the original data intact.
fn copy_restore_into_main(restore_name: &str) -> Result<(i64, i64), String> {
    let main = open(DB_NAME).map_err(|error| error.to_string())?;
    main.execute_batch("PRAGMA foreign_keys=OFF;")
        .map_err(|error| error.to_string())?;
    let result = copy_tables(&main, restore_name);
    if result.is_err() {
        let _ = main.execute_batch("ROLLBACK;");
    }
    let _ = main.execute_batch("DETACH DATABASE restore;");
    let _ = main.execute_batch("PRAGMA foreign_keys=ON;");
    result
}

fn copy_tables(main: &Connection, restore_name: &str) -> Result<(i64, i64), String> {
    let run = |sql: &str| main.execute_batch(sql).map_err(|error| error.to_string());
    run(format!("ATTACH DATABASE '{restore_name}' AS restore;").as_str())?;
    run("BEGIN;")?;
    // Children first, so no foreign key is left pointing at a missing row.
    run("DELETE FROM attempt_answers;")?;
    run("DELETE FROM attempts;")?;
    run("DELETE FROM answer_options;")?;
    run("DELETE FROM questions;")?;
    run("DELETE FROM quizzes;")?;
    run("INSERT INTO main.quizzes SELECT * FROM restore.quizzes;")?;
    run("INSERT INTO main.questions SELECT * FROM restore.questions;")?;
    run("INSERT INTO main.answer_options SELECT * FROM restore.answer_options;")?;
    run("INSERT INTO main.attempts SELECT * FROM restore.attempts;")?;
    run("INSERT INTO main.attempt_answers SELECT * FROM restore.attempt_answers;")?;
    run("COMMIT;")?;
    let counts = main
        .query_row(
            "SELECT (SELECT COUNT(*) FROM quizzes), (SELECT COUNT(*) FROM attempts)",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|error| error.to_string())?;
    run("DETACH DATABASE restore;")?;
    Ok(counts)
}

fn reset_data(state: &Rc<RefCell<WorkerState>>) -> Result<Value, String> {
    let state_ref = state.borrow();
    let connection = state_ref
        .connection
        .as_ref()
        .ok_or_else(|| "Database is not open yet.".to_string())?;
    connection
        .execute_batch(
            "DELETE FROM attempt_answers;
             DELETE FROM attempts;
             DELETE FROM answer_options;
             DELETE FROM questions;
             DELETE FROM quizzes;",
        )
        .map_err(|error| error.to_string())?;
    let now = js_sys::Date::now() as i64;
    seed_samples(connection, SAMPLES, now).map_err(|error| error.to_string())?;
    Ok(Value::Null)
}

fn handle_request(state: &Rc<RefCell<WorkerState>>, message: &str) -> Result<(), String> {
    let request: Request = serde_json::from_str(message).map_err(|error| error.to_string())?;
    let result: Result<Value, String> = match request.command {
        Command::BackupDatabase => backup_database(state),
        Command::RestoreDatabase { data_base64 } => restore_database(state, &data_base64),
        Command::ResetData => reset_data(state),
        command => {
            let state_ref = state.borrow();
            match state_ref.connection.as_ref() {
                None => Err("Database is not open yet.".to_string()),
                Some(connection) => {
                    let now = js_sys::Date::now() as i64;
                    dispatch(connection, command, now).map_err(|error| error.to_string())
                }
            }
        }
    };
    respond(state, request.id, result);
    Ok(())
}

/// Waits inside the worker. Used between attempts to open the saved data.
async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve: js_sys::Function, _reject| {
        let _ = worker_scope().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Opens the saved-data storage. Browsers allow only one live handle to each
/// file, so a handle left behind by a reload or another tab can make the first
/// attempt fail. Failed attempts register nothing, so retrying is safe.
async fn open_storage() -> Result<OpfsSAHPoolUtil, JsValue> {
    const ATTEMPTS: i32 = 8;
    let mut last_error = String::new();
    for attempt in 1..=ATTEMPTS {
        match install::<WasmOsCallback>(&OpfsSAHPoolCfg::default(), true).await {
            Ok(pool) => return Ok(pool),
            Err(error) => last_error = error.to_string(),
        }
        if attempt < ATTEMPTS {
            sleep_ms(250 * attempt).await;
        }
    }
    Err(JsValue::from_str(&format!(
        "MockTest couldn't open your saved data. It may be open in another tab or window. Close the other MockTest tabs, then reload this page. ({last_error})"
    )))
}

#[wasm_bindgen]
pub async fn start() -> Result<(), JsValue> {
    set_once();
    let pool = open_storage().await?;
    let conn = open(DB_NAME).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let now = js_sys::Date::now() as i64;
    seed_samples(&conn, SAMPLES, now).map_err(|error| JsValue::from_str(&error.to_string()))?;

    let state_rc = state();
    {
        let mut state_mut = state_rc.borrow_mut();
        state_mut.connection = Some(conn);
        state_mut.pool = Some(pool);
    }

    let scope = state_rc.borrow().scope.clone();
    let handler_state = state_rc.clone();
    let handler = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
        let Some(data) = event.data().as_string() else {
            return;
        };
        let _ = handle_request(&handler_state, &data);
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    let handler_fn: &js_sys::Function = handler.as_ref().unchecked_ref();
    scope.set_onmessage(Some(handler_fn));
    // The handler must live as long as the worker.
    handler.forget();
    Ok(())
}
