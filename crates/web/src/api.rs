// Client for the database worker. Every request is a JSON string sent to the
// worker; the reply carries the same id and is delivered through a JS Promise.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use js_sys::{Array, Function, Promise};
use mocktest_domain::{
    AttemptReview, AttemptView, BootstrapData, Command, Quiz, Request, Response, SaveAnswerResult,
};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, MessageEvent, Url, Worker, WorkerOptions, WorkerType};

const READY_ID: u64 = 0;

thread_local! {
    static WORKER: RefCell<Option<Worker>> = const { RefCell::new(None) };
    static READY: RefCell<Option<Promise>> = const { RefCell::new(None) };
    static PENDING: RefCell<HashMap<u64, (Function, Function)>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

fn js_err(value: JsValue) -> String {
    crate::ui::js_error(value)
}

/// Starts the worker once. Its wasm glue is copied into /worker/ by Trunk.
fn ensure_worker() -> Result<Worker, JsValue> {
    if let Some(worker) = WORKER.with(|w| w.borrow().clone()) {
        return Ok(worker);
    }
    let worker = spawn_worker()?;

    let ready = Promise::new(&mut |resolve: Function, reject: Function| {
        PENDING.with(|p| {
            p.borrow_mut().insert(READY_ID, (resolve, reject));
        });
    });

    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
        if let Some(text) = event.data().as_string() {
            deliver(&text);
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // A worker that fails to load never replies. Turn that into a readable error.
    // (Developers: if the worker files are missing, run scripts/build-worker.sh.)
    let onerror = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        deliver(
            &serde_json::json!({
                "id": READY_ID,
                "ok": false,
                "error": "The app couldn't start its storage. Reload the page. If this keeps happening, try another browser.",
            })
            .to_string(),
        );
    }) as Box<dyn FnMut(web_sys::Event)>);
    let _ = worker.add_event_listener_with_callback("error", onerror.as_ref().unchecked_ref());
    onerror.forget();

    READY.with(|r| *r.borrow_mut() = Some(ready));
    WORKER.with(|w| *w.borrow_mut() = Some(worker.clone()));
    Ok(worker)
}

/// Starts the worker from a small generated module. A browser can only start a
/// worker from a script, so this is the one piece of JavaScript in the project;
/// it is produced here in Rust and never written to a file.
fn spawn_worker() -> Result<Worker, JsValue> {
    let base = app_base_url()?;
    let script = format!(
        "import init, {{ start }} from \"{base}worker/mocktest_worker.js\";\n\
         init().then(() => start()).then(\n\
           () => postMessage(JSON.stringify({{ id: {READY_ID}, ok: true, data: null }})),\n\
           (e) => postMessage(JSON.stringify({{ id: {READY_ID}, ok: false, error: String(e) }})),\n\
         );\n"
    );
    let parts: JsValue = Array::of1(&JsValue::from_str(&script)).into();
    let bag = BlobPropertyBag::new();
    bag.set_type("text/javascript");
    let blob = Blob::new_with_str_sequence_and_options(&parts, &bag)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    Worker::new_with_options(&url, &options)
}

/// The folder the app is served from, with a trailing slash. This is "/" locally
/// and "/<repo>/" on GitHub Pages, so the worker is found in both places.
fn app_base_url() -> Result<String, JsValue> {
    let href = crate::ui::win().location().href()?;
    let without_suffix = href.split(['?', '#']).next().unwrap_or_default();
    let end = without_suffix.rfind('/').map_or(0, |i| i + 1);
    Ok(without_suffix[..end].to_string())
}

fn deliver(text: &str) {
    let Ok(reply) = serde_json::from_str::<Response>(text) else {
        return;
    };
    let entry = PENDING.with(|p| p.borrow_mut().remove(&reply.id));
    if let Some((resolve, _reject)) = entry {
        let _ = resolve.call1(&JsValue::NULL, &JsValue::from_str(text));
    }
}

async fn ready() -> Result<(), String> {
    let promise = READY
        .with(|r| r.borrow().clone())
        .ok_or("The database worker is not running.")?;
    let text = JsFuture::from(promise).await.map_err(js_err)?.as_string().unwrap_or_default();
    let reply: Response = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if reply.ok {
        Ok(())
    } else {
        Err(reply.error.unwrap_or_else(|| "The database could not start.".to_string()))
    }
}

async fn call(command: Command) -> Result<serde_json::Value, String> {
    let worker = ensure_worker().map_err(js_err)?;
    ready().await?;

    let id = NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    let reply = Promise::new(&mut |resolve: Function, reject: Function| {
        PENDING.with(|p| {
            p.borrow_mut().insert(id, (resolve, reject));
        });
    });

    let request = serde_json::to_string(&Request { id, command }).map_err(|e| e.to_string())?;
    worker.post_message(&JsValue::from_str(&request)).map_err(js_err)?;

    let text = JsFuture::from(reply).await.map_err(js_err)?.as_string().unwrap_or_default();
    let reply: Response = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if reply.ok {
        Ok(reply.data.unwrap_or(serde_json::Value::Null))
    } else {
        Err(reply.error.unwrap_or_else(|| "Unknown database error.".to_string()))
    }
}

async fn typed<T: DeserializeOwned>(command: Command) -> Result<T, String> {
    let value = call(command).await?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}

pub async fn bootstrap() -> Result<BootstrapData, String> {
    typed(Command::Bootstrap).await
}

pub async fn get_quiz(id: &str) -> Result<Quiz, String> {
    typed(Command::GetQuiz { id: id.into() }).await
}

pub async fn create_quiz() -> Result<Quiz, String> {
    typed(Command::CreateQuiz).await
}

pub async fn save_quiz(quiz: Quiz) -> Result<Quiz, String> {
    typed(Command::SaveQuiz { quiz }).await
}

pub async fn delete_quiz(id: &str) -> Result<(), String> {
    typed(Command::DeleteQuiz { id: id.into() }).await
}

pub async fn duplicate_quiz(id: &str) -> Result<Quiz, String> {
    typed(Command::DuplicateQuiz { id: id.into() }).await
}

pub async fn import_quiz(json: String) -> Result<Quiz, String> {
    typed(Command::ImportQuiz { json }).await
}

pub async fn export_quiz(id: &str) -> Result<String, String> {
    typed(Command::ExportQuiz { id: id.into() }).await
}

pub async fn start_attempt(quiz_id: &str, seed: u64) -> Result<AttemptView, String> {
    typed(Command::StartAttempt { quiz_id: quiz_id.into(), seed }).await
}

pub async fn get_attempt(id: &str) -> Result<AttemptView, String> {
    typed(Command::GetAttempt { id: id.into() }).await
}

pub async fn save_answer(
    attempt_id: &str,
    question_id: &str,
    selected: Vec<String>,
    flagged: bool,
) -> Result<SaveAnswerResult, String> {
    typed(Command::SaveAnswer {
        attempt_id: attempt_id.into(),
        question_id: question_id.into(),
        selected,
        flagged,
    })
    .await
}

pub async fn submit_attempt(id: &str) -> Result<AttemptReview, String> {
    typed(Command::SubmitAttempt { id: id.into() }).await
}

pub async fn get_review(id: &str) -> Result<AttemptReview, String> {
    typed(Command::GetReview { id: id.into() }).await
}

pub async fn backup() -> Result<String, String> {
    typed(Command::BackupDatabase).await
}

pub async fn restore(data_base64: String) -> Result<serde_json::Value, String> {
    typed(Command::RestoreDatabase { data_base64 }).await
}

pub async fn reset() -> Result<(), String> {
    typed(Command::ResetData).await
}

pub async fn clear_history() -> Result<(), String> {
    typed(Command::ClearHistory).await
}
