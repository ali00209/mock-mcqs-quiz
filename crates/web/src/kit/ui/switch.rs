use leptos::prelude::*;
use leptos_ui::clx;
use tw_merge::tw_merge;

mod components {
    use super::*;
    clx! {SwitchLabel, span, "text-sm font-medium"}
}

pub use components::*;

/// A toggle bound to `checked`. `on_checked_change` receives the new value.
#[component]
pub fn Switch(
    #[prop(optional, into)] id: String,
    #[prop(into)] checked: Signal<bool>,
    #[prop(optional)] on_checked_change: Option<Callback<bool>>,
    #[prop(into, optional, default = "Toggle switch".to_string())] aria_label: String,
    #[prop(into, optional)] class: String,
) -> impl IntoView {
    let state = move || if checked.get() { "checked" } else { "unchecked" };

    let track_class = tw_merge!(
        "inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border-2 border-ink transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=unchecked]:bg-field-bg",
        class
    );

    view! {
        <button
            data-name="Switch"
            id=id
            type="button"
            role="switch"
            aria-checked=move || checked.get().to_string()
            aria-label=aria_label
            data-state=state
            class=track_class
            on:click=move |_| {
                if let Some(on_change) = on_checked_change {
                    on_change.run(!checked.get_untracked());
                }
            }
        >
            <span
                data-state=state
                class="pointer-events-none block size-5 rounded-full bg-surface ring-0 shadow-lg transition-transform data-[state=checked]:translate-x-5 data-[state=unchecked]:translate-x-0"
            />
        </button>
    }
}
