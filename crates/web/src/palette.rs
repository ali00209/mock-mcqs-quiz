use leptos::prelude::*;
use web_sys::Storage;

const STORAGE_KEY: &str = "palette";
const MODE_KEY: &str = "mode";
const BACKGROUND_KEY: &str = "background";

/// Appearance modes: (key, label). "system" follows the operating system setting.
pub const MODES: [(&str, &str); 3] = [("system", "System"), ("light", "Light"), ("dark", "Dark")];

/// Colour schemes the reader can choose from: (key, label, swatch colour).
/// The first entry is the default and has no `data-palette` attribute.
pub const PALETTES: [(&str, &str, &str); 8] = [
    ("indigo", "Indigo", "#4b57d6"),
    ("ocean", "Ocean", "#0f7c8c"),
    ("forest", "Forest", "#2f7d4f"),
    ("sepia", "Sepia", "#9a5b2e"),
    ("slate", "Slate", "#475d7d"),
    ("lavender", "Lavender", "#6f5bc4"),
    ("mint", "Mint", "#2a8068"),
    ("rose", "Rose", "#b5536b"),
];

fn storage() -> Option<Storage> {
    crate::ui::win().local_storage().ok().flatten()
}

/// The palette saved in this browser, or the default when none is saved.
fn saved_key() -> &'static str {
    let saved = storage()
        .and_then(|store| store.get_item(STORAGE_KEY).ok().flatten())
        .unwrap_or_default();
    PALETTES
        .iter()
        .map(|palette| palette.0)
        .find(|key| *key == saved.as_str())
        .unwrap_or(PALETTES[0].0)
}

/// Applies a palette to the page by setting `data-palette` on the root element.
fn apply(key: &str) {
    let Some(root) = crate::ui::win().document().and_then(|doc| doc.document_element()) else {
        return;
    };
    if key == PALETTES[0].0 {
        let _ = root.remove_attribute("data-palette");
    } else {
        let _ = root.set_attribute("data-palette", key);
    }
}

/// Restores the saved palette. Call once at startup, before the first render.
pub fn init() {
    apply(saved_key());
    apply_mode(saved_mode());
    apply_background(saved_background().as_deref());
}

/// A saved "#rrggbb" background chosen by the reader, if any.
fn saved_background() -> Option<String> {
    storage()
        .and_then(|store| store.get_item(BACKGROUND_KEY).ok().flatten())
        .filter(|value| is_hex_colour(value))
}

fn is_hex_colour(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Sets the page background to the reader's colour, or clears it so the scheme's colour applies.
fn apply_background(colour: Option<&str>) {
    let Some(root) = crate::ui::win().document().and_then(|doc| doc.document_element()) else {
        return;
    };
    match colour {
        Some(hex) => {
            let _ = root.set_attribute("style", &format!("--bg: {hex}"));
        }
        None => {
            let _ = root.remove_attribute("style");
        }
    }
}

#[component]
pub fn BackgroundPicker() -> impl IntoView {
    let initial = saved_background();
    let custom = RwSignal::new(initial.is_some());
    let current = RwSignal::new(initial.unwrap_or_else(|| "#ffffff".to_string()));

    view! {
        <div class="flex flex-wrap items-center gap-2">
            <input
                type="color"
                aria-label="Background colour"
                class="h-9 w-14 cursor-pointer rounded-full border border-rule bg-field-bg p-1"
                prop:value=move || current.get()
                on:input=move |ev| {
                    let value = event_target_value(&ev);
                    if is_hex_colour(&value) {
                        current.set(value.clone());
                        custom.set(true);
                        if let Some(store) = storage() {
                            let _ = store.set_item(BACKGROUND_KEY, &value);
                        }
                        apply_background(Some(&value));
                    }
                }
            />
            <button
                type="button"
                disabled=move || !custom.get()
                class="inline-flex h-9 items-center rounded-full border border-rule bg-field-bg px-4 font-display font-bold text-sm text-field-ink cursor-pointer hover:bg-muted disabled:cursor-not-allowed disabled:opacity-50"
                on:click=move |_| {
                    custom.set(false);
                    if let Some(store) = storage() {
                        let _ = store.remove_item(BACKGROUND_KEY);
                    }
                    apply_background(None);
                }
            >
                "Use scheme background"
            </button>
        </div>
    }
}

/// The mode saved in this browser, or "system" when none is saved.
fn saved_mode() -> &'static str {
    let saved = storage()
        .and_then(|store| store.get_item(MODE_KEY).ok().flatten())
        .unwrap_or_default();
    MODES
        .iter()
        .map(|mode| mode.0)
        .find(|key| *key == saved.as_str())
        .unwrap_or(MODES[0].0)
}

/// Sets `data-mode` on the root element; "system" removes it so the OS setting applies.
fn apply_mode(key: &str) {
    let Some(root) = crate::ui::win().document().and_then(|doc| doc.document_element()) else {
        return;
    };
    if key == MODES[0].0 {
        let _ = root.remove_attribute("data-mode");
    } else {
        let _ = root.set_attribute("data-mode", key);
    }
}

fn mode_class(active: bool) -> &'static str {
    if active {
        "inline-flex h-9 items-center rounded-full border border-primary bg-muted px-4 font-display font-bold text-sm text-ink cursor-pointer"
    } else {
        "inline-flex h-9 items-center rounded-full border border-rule bg-field-bg px-4 font-display font-bold text-sm text-field-ink cursor-pointer hover:bg-muted"
    }
}

#[component]
pub fn ModePicker() -> impl IntoView {
    let current = RwSignal::new(saved_mode());

    view! {
        <div class="flex flex-wrap items-center gap-1.5" role="radiogroup" aria-label="Light or dark">
            {MODES
                .iter()
                .map(|&(key, label)| {
                    view! {
                        <button
                            type="button"
                            role="radio"
                            aria-checked=move || (current.get() == key).to_string()
                            class=move || mode_class(current.get() == key)
                            on:click=move |_| {
                                current.set(key);
                                if let Some(store) = storage() {
                                    let _ = store.set_item(MODE_KEY, key);
                                }
                                apply_mode(key);
                            }
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
pub fn PalettePicker() -> impl IntoView {
    let current = RwSignal::new(saved_key());

    view! {
        <div class="flex flex-wrap items-center gap-1.5" role="radiogroup" aria-label="Colour scheme">
            {PALETTES
                .iter()
                .map(|&(key, label, swatch)| {
                    view! {
                        <button
                            type="button"
                            role="radio"
                            aria-checked=move || (current.get() == key).to_string()
                            title=label
                            class=move || {
                                if current.get() == key {
                                    "inline-flex h-9 items-center gap-2 rounded-full border border-primary bg-muted px-3 font-display font-bold text-sm text-ink cursor-pointer"
                                } else {
                                    "inline-flex h-9 items-center gap-2 rounded-full border border-rule bg-field-bg px-3 font-display font-bold text-sm text-field-ink cursor-pointer hover:bg-muted"
                                }
                            }
                            on:click=move |_| {
                                current.set(key);
                                if let Some(store) = storage() {
                                    let _ = store.set_item(STORAGE_KEY, key);
                                }
                                apply(key);
                            }
                        >
                            <span
                                class="inline-block size-3.5 rounded-full border border-rule"
                                style=format!("background-color: {swatch}")
                            ></span>
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
