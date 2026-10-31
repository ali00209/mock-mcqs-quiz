use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys;
use tw_merge::tw_merge;

use crate::kit::ui::button::{Button, ButtonVariant};

/// Arrow keys, Home and End move between slides. Only when the carousel itself has focus,
/// so typing in a field inside a slide keeps its own arrow keys.
fn carousel_key(index: RwSignal<usize>, count: Signal<usize>, used: RwSignal<bool>, ev: leptos::ev::KeyboardEvent) {
    let on_carousel = ev
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .map(|el| el.get_attribute("data-name").as_deref() == Some("Carousel"))
        .unwrap_or(false);
    if !on_carousel {
        return;
    }
    let last = count.get().saturating_sub(1);
    match ev.key().as_str() {
        "ArrowLeft" => index.update(|i| *i = i.saturating_sub(1)),
        "ArrowRight" => index.update(|i| {
            if *i < last {
                *i += 1;
            }
        }),
        "Home" => index.set(0),
        "End" => index.set(last),
        _ => return,
    }
    used.set(true);
    ev.prevent_default();
}

/// Shows one slide at a time. `index` is the slide on screen. Slides are `CarouselItem`s
/// whose `position` matches their place in the carousel.
#[component]
pub fn Carousel(
    index: RwSignal<usize>,
    #[prop(into)] count: Signal<usize>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let used = RwSignal::new(false);
    let merged_class = tw_merge!("relative w-full", class);
    view! {
        <section
            data-name="Carousel"
            role="region"
            aria-roledescription="carousel"
            aria-label="Questions"
            tabindex="0"
            class=merged_class
            on:keydown=move |ev| carousel_key(index, count, used, ev)
        >
            {move || {
                (count.get() >= 2 && !used.get())
                    .then(|| {
                        view! {
                            <p class="mb-3 text-[13px] text-muted-foreground">
                                "Tip: Tab to the question area, then use ← and → to move between questions."
                            </p>
                        }
                    })
            }}
            <p class="sr-only" aria-live="polite">
                {move || {
                    let total = count.get();
                    if total == 0 {
                        "No questions".to_string()
                    } else {
                        format!("Question {} of {}", index.get().min(total - 1) + 1, total)
                    }
                }}
            </p>
            {children()}
        </section>
    }
}

#[component]
pub fn CarouselContent(children: Children, #[prop(optional, into)] class: String) -> impl IntoView {
    let merged_class = tw_merge!("flex flex-col", class);
    view! { <div data-name="CarouselContent" class=merged_class>{children()}</div> }
}

/// A slide. It shows when `current` equals its `position`. Items are built before the carousel
/// exists, so the current index is passed in rather than read from context. For a list that is
/// not a carousel, pass the item's own position as `current`, and every item shows.
#[component]
pub fn CarouselItem(
    #[prop(into)] position: Signal<usize>,
    #[prop(into)] current: Signal<usize>,
    children: Children,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let is_hidden = move || current.get() != position.get();
    view! {
        <div
            data-name="CarouselItem"
            role="group"
            aria-roledescription="slide"
            aria-hidden=move || is_hidden().to_string()
            class=move || tw_merge!(if is_hidden() { "hidden" } else { "" }, class.clone())
        >
            {children()}
        </div>
    }
}

#[component]
pub fn CarouselPrevious(index: RwSignal<usize>, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        // aria-disabled instead of disabled: a disabled button loses keyboard focus.
        <Button
            variant=ButtonVariant::Outline
            class=class
            attr:aria-disabled=move || (index.get() == 0).then_some("true")
            on:click=move |_| index.update(|i| *i = i.saturating_sub(1))
        >
            "Previous"
        </Button>
    }
}

#[component]
pub fn CarouselNext(index: RwSignal<usize>, #[prop(into)] count: Signal<usize>, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        <Button
            variant=ButtonVariant::Outline
            class=class
            attr:aria-disabled=move || (index.get() + 1 >= count.get()).then_some("true")
            on:click=move |_| {
                if index.get() + 1 < count.get() {
                    index.update(|i| *i += 1);
                }
            }
        >
            "Next"
        </Button>
    }
}

#[component]
pub fn CarouselFirst(index: RwSignal<usize>, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        <Button
            variant=ButtonVariant::Outline
            class=class
            attr:aria-disabled=move || (index.get() == 0).then_some("true")
            on:click=move |_| index.set(0)
        >
            "First"
        </Button>
    }
}

#[component]
pub fn CarouselLast(
    index: RwSignal<usize>,
    #[prop(into)] count: Signal<usize>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    view! {
        <Button
            variant=ButtonVariant::Outline
            class=class
            attr:aria-disabled=move || (index.get() + 1 >= count.get()).then_some("true")
            on:click=move |_| index.set(count.get().saturating_sub(1))
        >
            "Last"
        </Button>
    }
}

/// A number box that jumps to a question. It shows the question on screen and the total.
#[component]
pub fn CarouselGoTo(
    index: RwSignal<usize>,
    #[prop(into)] count: Signal<usize>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let text = RwSignal::new(String::from("1"));
    Effect::new(move |_| {
        let shown = index.get() + 1;
        text.set(shown.to_string());
    });
    let merged_class = tw_merge!("flex flex-wrap items-center gap-3", class);
    view! {
        <div class=merged_class>
            <label class="flex items-center gap-2">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Go to question"</span>
                <input
                    type="number"
                    inputmode="numeric"
                    min="1"
                    max=move || count.get().max(1).to_string()
                    disabled=move || count.get() == 0
                    class="h-9 w-20 border-2 border-ink bg-field-bg px-3 font-mono text-field-ink disabled:opacity-50"
                    prop:value=move || text.get()
                    on:input=move |ev| {
                        let typed = event_target_value(&ev);
                        text.set(typed.clone());
                        if let Ok(n) = typed.trim().parse::<usize>() {
                            if n >= 1 && n <= count.get_untracked() {
                                index.set(n - 1);
                            }
                        }
                    }
                />
            </label>
            <span class="text-sm text-muted-foreground">
                {move || {
                    let total = count.get();
                    let typed = text.get().trim().parse::<usize>().ok();
                    match (total, typed) {
                        (0, _) => "No questions yet".to_string(),
                        (n, Some(v)) if v < 1 || v > n => format!("Enter 1 to {n}"),
                        (n, _) => format!("of {n} question{}", if n == 1 { "" } else { "s" }),
                    }
                }}
            </span>
        </div>
    }
}
