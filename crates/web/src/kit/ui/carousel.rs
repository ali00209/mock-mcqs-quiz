use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys;
use tw_merge::tw_merge;

use crate::kit::ui::button::{Button, ButtonVariant};

#[derive(Clone, Copy)]
struct CarouselContext {
    index: RwSignal<usize>,
}

/// Arrow keys, Home and End move between slides. Only when the carousel itself has focus,
/// so typing in a field inside a slide keeps its own arrow keys.
fn carousel_key(index: RwSignal<usize>, count: Signal<usize>, ev: leptos::ev::KeyboardEvent) {
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
    provide_context(CarouselContext { index });
    let merged_class = tw_merge!("relative w-full", class);
    view! {
        <section
            data-name="Carousel"
            role="region"
            aria-roledescription="carousel"
            aria-label="Questions"
            tabindex="0"
            class=merged_class
            on:keydown=move |ev| carousel_key(index, count, ev)
        >
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

/// A slide. Outside a `Carousel` every item is shown, so one view serves both modes.
#[component]
pub fn CarouselItem(#[prop(into)] position: Signal<usize>, children: Children, #[prop(optional, into)] class: String) -> impl IntoView {
    let ctx = use_context::<CarouselContext>();
    let is_hidden = move || ctx.map(|c| c.index.get() != position.get()).unwrap_or(false);
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
