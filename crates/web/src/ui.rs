// Shared UI state and browser helpers.
use js_sys::{Array, Date, Function, Promise};
use leptos::prelude::*;
use mocktest_domain::AttemptStatus;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, Event, HtmlAnchorElement, HtmlInputElement, Url};

/// Signals every page can use. Copy, so it can be passed into any handler.
#[derive(Clone, Copy)]
pub struct Ui {
    /// Banner message: (text, is_error).
    pub notice: RwSignal<Option<(String, bool)>>,
    /// Bumped to make the library reload its data.
    pub version: RwSignal<u32>,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            notice: RwSignal::new(None),
            version: RwSignal::new(0),
        }
    }

    pub fn notify(self, message: impl Into<String>, error: bool) {
        self.notice.set(Some((message.into(), error)));
    }

    pub fn reload(self) {
        self.version.update(|v| *v += 1);
    }
}

pub fn win() -> web_sys::Window {
    web_sys::window().expect("no window")
}

/// The current location hash without the leading '#'.
pub fn hash() -> String {
    win().location().hash().unwrap_or_default().trim_start_matches('#').to_string()
}

pub fn go(hash: &str) {
    let _ = win().location().set_hash(hash);
}

pub fn confirm(message: &str) -> bool {
    win().confirm_with_message(message).unwrap_or(false)
}

pub fn now_ms() -> i64 {
    Date::now() as i64
}

pub fn new_seed() -> u64 {
    (js_sys::Math::random() * 2_147_483_647.0) as u64
}

pub fn js_error(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}

pub fn today() -> String {
    String::from(Date::new_0().to_iso_string()).chars().take(10).collect()
}

pub fn slug(text: &str) -> String {
    let slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = slug.split('-').filter(|part| !part.is_empty()).collect::<Vec<_>>().join("-");
    if trimmed.is_empty() { "quiz".to_string() } else { trimmed }
}

pub fn fmt_date(ms: i64) -> String {
    if ms <= 0 {
        return "—".to_string();
    }
    String::from(Date::new(&JsValue::from_f64(ms as f64)).to_locale_string("default", &JsValue::UNDEFINED))
}

pub fn fmt_duration(ms: Option<i64>) -> String {
    match ms {
        None => "—".to_string(),
        Some(ms) => {
            let total = (ms as f64 / 1000.0).round() as i64;
            let (m, s) = (total / 60, total % 60);
            if m > 0 { format!("{m}m {s}s") } else { format!("{s}s") }
        }
    }
}

pub fn fmt_clock(ms: i64) -> String {
    let total = ((ms as f64) / 1000.0).ceil().max(0.0) as i64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

pub fn pct(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{}%", v.round() as i64),
        None => "—".to_string(),
    }
}

pub fn status_label(status: AttemptStatus) -> &'static str {
    match status {
        AttemptStatus::InProgress => "In progress",
        AttemptStatus::Submitted => "Submitted",
        AttemptStatus::Expired => "Expired (time ran out)",
    }
}

/// Saves text as a file the browser downloads.
pub fn download(filename: &str, text: &str, mime: &str) -> Result<(), JsValue> {
    let parts: JsValue = Array::of1(&JsValue::from_str(text)).into();
    let bag = BlobPropertyBag::new();
    bag.set_type(mime);
    let blob = Blob::new_with_str_sequence_and_options(&parts, &bag)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    let document = win().document().ok_or("no document")?;
    let link: HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    link.set_href(&url);
    link.set_download(filename);
    let body = document.body().ok_or("no document body")?;
    body.append_child(&link)?;
    link.click();
    link.remove();
    Ok(())
}

/// Opens the file picker and resolves with the chosen file's text, or None if cancelled.
pub async fn pick_text(accept: &str) -> Result<Option<String>, JsValue> {
    let document = win().document().ok_or("no document")?;
    let input: HtmlInputElement = document.create_element("input")?.dyn_into()?;
    input.set_type("file");
    input.set_accept(accept);

    let changed = Promise::new(&mut |resolve: Function, _reject: Function| {
        let on_change = Closure::once_into_js(move |_: Event| {
            let _ = resolve.call0(&JsValue::NULL);
        });
        let _ = input.add_event_listener_with_callback("change", on_change.unchecked_ref());
    });
    input.click();
    JsFuture::from(changed).await?;

    let Some(file) = input.files().and_then(|files| files.get(0)) else {
        return Ok(None);
    };
    let text = JsFuture::from(file.text()).await?;
    Ok(text.as_string())
}
