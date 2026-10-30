use leptos::prelude::*;
use tw_merge::tw_merge;

use crate::kit::ui::button::{Button, ButtonVariant};

#[derive(Clone, Copy)]
struct CarouselContext {
    index: RwSignal<usize>,
}

/// Shows one slide at a time. `index` is the slide on screen. Slides are `CarouselItem`s
/// whose `position` matches their place in the carousel.
#[component]
pub fn Carousel(index: RwSignal<usize>, #[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    provide_context(CarouselContext { index });
    let merged_class = tw_merge!("relative w-full", class);
    view! {
        <section data-name="Carousel" role="region" aria-roledescription="carousel" class=merged_class>
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
