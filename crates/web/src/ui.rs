// Shared UI state and browser helpers.
use js_sys::{Array, Date, Function, Promise};
use leptos::prelude::*;
use leptos::prelude::LocalStorage;
use crate::kit::ui::button::{Button, ButtonVariant};
use mocktest_domain::AttemptStatus;
use wasm_bindgen::{JsCast, prelude::*};
use crate::kit::ui::toast_custom::_builder::ToastBuilder;
use crate::kit::ui::toast_custom::_context::ToasterContext;
use crate::kit::ui::toast_custom::_data::{ToastLevel, ToastPosition};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, Event, HtmlAnchorElement, HtmlInputElement, Url};

/// Signals every page can use. Copy, so it can be passed into any handler.
#[derive(Clone, Copy)]
pub struct Ui {
    /// Shows toast messages. Provided once at the app root.
    pub toaster: StoredValue<ToasterContext>,
    /// Ids of quizzes made in the editor that have not been saved yet.
    pub drafts: RwSignal<Vec<String>>,
    /// Bumped to make the library reload its data.
    pub version: RwSignal<u32>,
    /// The yes/no question currently shown in the app's dialog, if any.
    pub confirm: RwSignal<Option<Confirm>, LocalStorage>,
}

/// A yes/no question. `on_yes` runs only when the person confirms.
pub struct Confirm {
    title: String,
    message: String,
    action: String,
    destructive: bool,
    on_yes: Box<dyn FnOnce()>,
}

impl Ui {
    pub fn new(toaster: ToasterContext) -> Self {
        Self {
            toaster: StoredValue::new(toaster),
            drafts: RwSignal::new(Vec::new()),
            version: RwSignal::new(0),
            confirm: RwSignal::new_local(None),
        }
    }

    /// Shows a toast: an error when `error` is true, otherwise a success message.
    pub fn notify(self, message: impl Into<String>, error: bool) {
        let level = if error { ToastLevel::Error } else { ToastLevel::Success };
        self.toast(message.into(), level);
    }

    /// Shows an information toast.
    pub fn info(self, message: impl Into<String>) {
        self.toast(message.into(), ToastLevel::Info);
    }

    fn toast(self, message: String, level: ToastLevel) {
        self.toaster.with_value(|toaster| {
            toaster.toast(ToastBuilder::new(message).with_level(level).with_position(ToastPosition::BottomRight));
        });
    }

    pub fn reload(self) {
        self.version.update(|v| *v += 1);
    }

    /// Asks a yes/no question in the app's dialog. `on_yes` runs only if confirmed.
    pub fn ask(
        self,
        title: &str,
        message: &str,
        action: &str,
        destructive: bool,
        on_yes: impl FnOnce() + 'static,
    ) {
        self.confirm.set(Some(Confirm {
            title: title.to_string(),
            message: message.to_string(),
            action: action.to_string(),
            destructive,
            on_yes: Box::new(on_yes),
        }));
    }
}

/// The app's confirm dialog. Rendered once, at the root of the app.
#[component]
pub fn ConfirmDialog(ui: Ui) -> impl IntoView {
    let is_open = move || ui.confirm.with(|c| c.is_some());
    let title = move || ui.confirm.with(|c| c.as_ref().map(|c| c.title.clone()).unwrap_or_default());
    let message = move || ui.confirm.with(|c| c.as_ref().map(|c| c.message.clone()).unwrap_or_default());
    let action = move || ui.confirm.with(|c| c.as_ref().map(|c| c.action.clone()).unwrap_or_default());
    let destructive = move || ui.confirm.with(|c| c.as_ref().is_some_and(|c| c.destructive));

    let answer = move |yes: bool| {
        if let Some(pending) = ui.confirm.try_update(|c| c.take()).flatten() {
            if yes {
                (pending.on_yes)();
            }
        }
    };

    // Focus the safe choice (Cancel) when the dialog opens.
    Effect::new(move |_| {
        if is_open() {
            let cancel = win()
                .document()
                .and_then(|d| d.get_element_by_id("confirm-cancel"))
                .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok());
            if let Some(cancel) = cancel {
                let _ = cancel.focus();
            }
        }
    });

    view! {
        <Show when=is_open>
            <div class="fixed inset-0 z-[60] bg-black/50" aria-hidden="true" on:click=move |_| answer(false)></div>
            <div
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="confirm-title"
                aria-describedby="confirm-message"
                class="fixed top-1/2 left-1/2 z-[70] flex w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-md border bg-card p-6 text-card-foreground shadow-lg"
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "Escape" {
                        answer(false);
                    }
                }
            >
                <h2 id="confirm-title" class="text-lg leading-none font-semibold">{title}</h2>
                <p id="confirm-message" class="text-sm text-muted-foreground">{message}</p>
                <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
                    <Button variant=ButtonVariant::Outline attr:id="confirm-cancel" on:click=move |_| answer(false)>
                        "Cancel"
                    </Button>
                    {move || {
                        let variant = if destructive() { ButtonVariant::Destructive } else { ButtonVariant::Default };
                        view! { <Button variant=variant on:click=move |_| answer(true)>{action()}</Button> }
                    }}
                </div>
            </div>
        </Show>
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
