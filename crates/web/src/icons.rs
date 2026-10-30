// Drawn icons: one stroke weight, sized in em, hidden from screen readers.
// Callers provide the text or label that carries the meaning.
use leptos::prelude::*;

pub fn check() -> impl IntoView {
    view! {
        <svg class="size-[1em] fill-none stroke-current stroke-2 align-[-0.15em] [stroke-linecap:round] [stroke-linejoin:round]" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>
    }
}

pub fn cross() -> impl IntoView {
    view! {
        <svg class="size-[1em] fill-none stroke-current stroke-2 align-[-0.15em] [stroke-linecap:round] [stroke-linejoin:round]" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg>
    }
}

pub fn dot() -> impl IntoView {
    view! {
        <svg class="size-[1em] fill-none stroke-current stroke-2 align-[-0.15em] [stroke-linecap:round] [stroke-linejoin:round]" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="6"/></svg>
    }
}
