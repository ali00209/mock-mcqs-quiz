use leptos::prelude::*;
use web_sys::Storage;

const STORAGE_KEY: &str = "palette";
const MODE_KEY: &str = "mode";
const BACKGROUND_KEY: &str = "background";
const CUSTOM_KEY: &str = "custom";
const CUSTOM_STORAGE_KEY: &str = "custom-palette";
const CUSTOM_STYLE_ID: &str = "custom-palette-style";
/// The four colours the reader sets for each mode. Other colours are derived from these.
const CUSTOM_FIELDS: [&str; 4] = ["Background", "Cards", "Text", "Accent"];

/// Appearance modes: (key, label). "system" follows the operating system setting.
pub const MODES: [(&str, &str); 3] = [("system", "System"), ("light", "Light"), ("dark", "Dark")];

/// Colour schemes the reader can choose from: (key, label, swatch colour).
/// The first entry is the default and has no `data-palette` attribute.
pub const PALETTES: [(&str, &str, &str); 9] = [
    ("indigo", "Indigo", "#4b57d6"),
    ("ocean", "Ocean", "#0f7c8c"),
    ("forest", "Forest", "#2f7d4f"),
    ("sepia", "Sepia", "#9a5b2e"),
    ("slate", "Slate", "#475d7d"),
    ("lavender", "Lavender", "#6f5bc4"),
    ("mint", "Mint", "#2a8068"),
    ("rose", "Rose", "#b5536b"),
    (CUSTOM_KEY, "Custom", "#8a8fa3"),
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
    if key == CUSTOM_KEY {
        ensure_custom_style();
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
    let custom = RwSignal::new(saved_custom());

    view! {
        <div class="flex flex-col gap-4">
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
            {move || (current.get() == CUSTOM_KEY).then(|| view! { <CustomEditor custom=custom/> })}
        </div>
    }
}

/// Light and dark colour inputs for the custom scheme. Each change is saved and shown at once.
#[component]
fn CustomEditor(custom: RwSignal<Vec<String>>) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-3">
            <div class="grid gap-3 sm:grid-cols-2">
                {[("Light", 0usize), ("Dark", 4usize)]
                    .into_iter()
                    .map(|(title, offset)| {
                        view! {
                            <div class="rounded-2xl border border-rule bg-field-bg p-4">
                                <h3 class="m-0 mb-3 font-display font-bold text-sm text-field-ink">{title}" colours"</h3>
                                <div class="grid gap-2">
                                    {CUSTOM_FIELDS
                                        .iter()
                                        .enumerate()
                                        .map(|(field, &label)| {
                                            let index = offset + field;
                                            view! {
                                                <label class="flex items-center justify-between gap-3 text-sm text-field-ink">
                                                    <span>{label}</span>
                                                    <input
                                                        type="color"
                                                        aria-label=format!("{title} {label}")
                                                        class="h-9 w-14 cursor-pointer rounded-full border border-rule bg-surface p-1"
                                                        prop:value=move || custom.with(|v| v[index].clone())
                                                        on:input=move |ev| {
                                                            let value = event_target_value(&ev);
                                                            if is_hex_colour(&value) {
                                                                custom.update(|v| v[index] = value.clone());
                                                                save_custom(&custom.get_untracked());
                                                                ensure_custom_style();
                                                            }
                                                        }
                                                    />
                                                </label>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
            <div>
                <button
                    type="button"
                    class="inline-flex h-9 items-center rounded-full border border-rule bg-field-bg px-4 font-display font-bold text-sm text-field-ink cursor-pointer hover:bg-muted"
                    on:click=move |_| {
                        custom.set(default_custom());
                        save_custom(&default_custom());
                        ensure_custom_style();
                    }
                >
                    "Reset custom colours"
                </button>
            </div>
        </div>
    }
}

fn default_custom() -> Vec<String> {
    ["#f5f6fa", "#ffffff", "#1b2233", "#4b57d6", "#0d1120", "#161c2e", "#e8ebf5", "#8d93f5"]
        .iter()
        .map(|c| c.to_string())
        .collect()
}

/// The reader's saved custom colours: 4 light then 4 dark. Falls back to the defaults if missing or invalid.
fn saved_custom() -> Vec<String> {
    let parts: Vec<String> = storage()
        .and_then(|store| store.get_item(CUSTOM_STORAGE_KEY).ok().flatten())
        .unwrap_or_default()
        .split(',')
        .map(str::to_string)
        .collect();
    if parts.len() == 8 && parts.iter().all(|c| is_hex_colour(c)) {
        parts
    } else {
        default_custom()
    }
}

fn save_custom(values: &[String]) {
    if let Some(store) = storage() {
        let _ = store.set_item(CUSTOM_STORAGE_KEY, &values.join(","));
    }
}

fn to_rgb(hex: &str) -> [u8; 3] {
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    [channel(1), channel(3), channel(5)]
}

fn to_hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Blends colour `a` towards colour `b` by `t` (0 gives `a`, 1 gives `b`).
fn mix(a: &str, b: &str, t: f32) -> String {
    let (x, y) = (to_rgb(a), to_rgb(b));
    to_hex([0, 1, 2].map(|i| (x[i] as f32 + (y[i] as f32 - x[i] as f32) * t).round() as u8))
}

/// Black or white text, whichever reads better on the colour.
fn on_colour(hex: &str) -> &'static str {
    let c = to_rgb(hex);
    let luma = 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32;
    if luma > 150.0 { "#0b0e1d" } else { "#ffffff" }
}

/// The colour tokens one mode needs, derived from the four colours the reader chose.
fn tokens(bg: &str, surface: &str, ink: &str, accent: &str, dark: bool) -> String {
    let (ok, bad, warn) = if dark {
        ("#5cc08a", "#ef7b70", "#dfae5c")
    } else {
        ("#2f7a55", "#b8402f", "#8f6417")
    };
    let topbar = if dark { mix(accent, bg, 0.7) } else { mix(accent, ink, 0.35) };
    let tint = mix(surface, accent, 0.14);
    let vars = [
        ("bg", bg.to_string()),
        ("surface", surface.to_string()),
        ("ink", ink.to_string()),
        ("muted", mix(ink, surface, 0.4)),
        ("line", mix(surface, ink, 0.14)),
        ("topbar", topbar),
        ("accent", accent.to_string()),
        ("accent-deep", mix(accent, ink, 0.2)),
        ("accent-tint", tint.clone()),
        ("accent-text", on_colour(accent).to_string()),
        ("ok", ok.to_string()),
        ("bad", bad.to_string()),
        ("warn", warn.to_string()),
        ("field-bg", surface.to_string()),
        ("field-ink", ink.to_string()),
        ("tint", tint),
    ];
    vars.iter().map(|(k, v)| format!("--{k}: {v};")).collect::<Vec<_>>().join(" ")
}

/// Rules for the custom palette: light by default, dark under data-mode="dark",
/// and dark when the system is dark unless data-mode="light".
fn custom_css(values: &[String]) -> String {
    let light = tokens(&values[0], &values[1], &values[2], &values[3], false);
    let dark = tokens(&values[4], &values[5], &values[6], &values[7], true);
    format!(
        "html[data-palette=\"custom\"] {{ {light} color-scheme: light; }}\n\
         html[data-palette=\"custom\"][data-mode=\"dark\"] {{ {dark} color-scheme: dark; }}\n\
         @media (prefers-color-scheme: dark) {{ html[data-palette=\"custom\"]:not([data-mode=\"light\"]) {{ {dark} color-scheme: dark; }} }}"
    )
}

/// Writes the custom palette's rules into one <style> element in the page head, creating it if needed.
fn ensure_custom_style() {
    let Some(document) = crate::ui::win().document() else {
        return;
    };
    let style = match document.get_element_by_id(CUSTOM_STYLE_ID) {
        Some(existing) => existing,
        None => {
            let Ok(created) = document.create_element("style") else {
                return;
            };
            created.set_id(CUSTOM_STYLE_ID);
            let Some(head) = document.head() else {
                return;
            };
            let _ = head.append_child(&created);
            created
        }
    };
    style.set_text_content(Some(&custom_css(&saved_custom())));
}

