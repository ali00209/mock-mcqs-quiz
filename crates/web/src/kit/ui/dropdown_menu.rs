use leptos::prelude::*;

use crate::kit::ui::button::{Button, ButtonVariant};

#[derive(Clone, Copy)]
struct MenuContext {
    open: RwSignal<bool>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum DropdownMenuActionVariant {
    #[default]
    Default,
    Destructive,
}

/// A button that opens a menu of actions. The menu closes on an action, an outside click or Escape.
#[component]
pub fn DropdownMenu(children: Children) -> impl IntoView {
    provide_context(MenuContext { open: RwSignal::new(false) });
    view! { <div data-name="DropdownMenu" class="relative inline-flex">{children()}</div> }
}

#[component]
pub fn DropdownMenuTrigger(children: Children) -> impl IntoView {
    let ctx = expect_context::<MenuContext>();
    view! {
        <Button
            variant=ButtonVariant::Outline
            attr:aria-haspopup="menu"
            attr:aria-expanded=move || ctx.open.get().to_string()
            on:click=move |_| ctx.open.update(|o| *o = !*o)
        >
            {children()}
        </Button>
    }
}

#[component]
pub fn DropdownMenuContent(children: ChildrenFn) -> impl IntoView {
    let ctx = expect_context::<MenuContext>();
    view! {
        <Show when=move || ctx.open.get()>
            <div class="fixed inset-0 z-40" aria-hidden="true" on:click=move |_| ctx.open.set(false)></div>
            <div
                role="menu"
                class="absolute top-full right-0 z-50 mt-1 flex min-w-48 flex-col border-2 border-ink bg-surface p-1 shadow-lg"
                on:click=move |_| ctx.open.set(false)
                on:keydown=move |ev| {
                    if ev.key() == "Escape" {
                        ctx.open.set(false);
                    }
                }
            >
                {children()}
            </div>
        </Show>
    }
}

/// One item in the menu. With `href` it is a link; otherwise it is a button, and the caller
/// attaches its click handler to a wrapper element.
#[component]
pub fn DropdownMenuAction(
    children: Children,
    #[prop(optional, into)] href: Option<String>,
    #[prop(optional)] variant: DropdownMenuActionVariant,
) -> impl IntoView {
    let class = match variant {
        DropdownMenuActionVariant::Default => {
            "flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left text-sm text-ink no-underline hover:bg-muted"
        }
        DropdownMenuActionVariant::Destructive => {
            "flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left text-sm text-bad no-underline hover:bg-muted"
        }
    };
    match href {
        Some(href) => view! { <a role="menuitem" href=href class=class>{children()}</a> }.into_any(),
        None => view! { <button type="button" role="menuitem" class=class>{children()}</button> }.into_any(),
    }
}
