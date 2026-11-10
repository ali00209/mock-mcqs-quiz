use leptos::prelude::*;
use web_sys::Storage;

const STORAGE_KEY: &str = "palette";
const MODE_KEY: &str = "mode";

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
