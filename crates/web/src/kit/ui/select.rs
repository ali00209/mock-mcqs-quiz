use leptos::prelude::*;
use rust_icons::{Check, ChevronDown};
use tw_merge::tw_merge;

/// A single-choice dropdown. `options` are (value, label) pairs; `value` holds the chosen value.
#[component]
pub fn Select(
    value: RwSignal<String>,
    options: Vec<(&'static str, &'static str)>,
    #[prop(into)] aria_label: String,
    #[prop(optional, into)] class: String,
    #[prop(optional)] on_change: Option<Callback<String>>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let options = StoredValue::new(options);
    let label = move || {
        options.with_value(|list| {
            list.iter()
                .find(|(v, _)| *v == value.get())
                .map(|(_, l)| l.to_string())
                .unwrap_or_default()
        })
    };
    let merged_class = tw_merge!("relative block w-full", class);

    view! {
        <div data-name="Select" class=merged_class>
            <button
                type="button"
                role="combobox"
                aria-label=aria_label
                aria-haspopup="listbox"
                aria-expanded=move || open.get().to_string()
                class="flex h-10 w-full cursor-pointer items-center justify-between gap-2 border-2 border-ink bg-field-bg px-3 text-left text-sm text-field-ink"
                on:click=move |_| open.update(|o| *o = !*o)
            >
                <span class="truncate">{label}</span>
                <ChevronDown class="size-4 shrink-0" />
            </button>
            <Show when=move || open.get()>
                <div class="fixed inset-0 z-40" aria-hidden="true" on:click=move |_| open.set(false)></div>
                <ul role="listbox" class="absolute top-full right-0 left-0 z-50 mt-1 flex flex-col border-2 border-ink bg-surface p-1 shadow-lg">
                    {move || {
                        options.with_value(|list| {
                            list.iter()
                                .map(|(v, l)| {
                                    let value_for_click = v.to_string();
                                    let value_for_check = v.to_string();
                                    let label = l.to_string();
                                    view! {
                                        <li
                                            role="option"
                                            aria-selected=move || (value.get() == value_for_check).to_string()
                                            class="group flex cursor-pointer items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-muted"
                                            on:click=move |_| {
                                                value.set(value_for_click.clone());
                                                if let Some(cb) = on_change {
                                                    cb.run(value_for_click.clone());
                                                }
                                                open.set(false);
                                            }
                                        >
                                            <span class="flex-1">{label}</span>
                                            <Check class="size-4 opacity-0 group-aria-selected:opacity-100" />
                                        </li>
                                    }
                                })
                                .collect_view()
                        })
                    }}
                </ul>
            </Show>
        </div>
    }
}
