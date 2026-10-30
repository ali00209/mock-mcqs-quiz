// Taking an attempt and reviewing a finished one.
use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use mocktest_domain::{
    AttemptQuestionView, AttemptReview, AttemptStatus, AttemptView, QuestionKind, ReviewQuestion,
};
use wasm_bindgen::{JsCast, prelude::*};

use crate::icons;

use crate::api;
use crate::kit::ui::badge::{Badge, BadgeVariant};
use crate::kit::ui::checkbox::Checkbox;
use crate::kit::ui::radio_button::{RadioGroup, RadioGroupItem};
use crate::ui::{self, Ui};
use crate::kit::ui::button::{Button, ButtonVariant};
use crate::kit::ui::carousel::{Carousel, CarouselContent, CarouselFirst, CarouselItem, CarouselLast, CarouselNext, CarouselPrevious};

#[derive(Clone, Default)]
struct Answer {
    selected: Vec<String>,
    flagged: bool,
}

/// Handles shared by every part of the attempt screen. All fields are Copy.
#[derive(Clone, Copy)]
struct Session {
    attempt_id: StoredValue<String>,
    answers: RwSignal<HashMap<String, Answer>>,
    index: RwSignal<usize>,
    now: RwSignal<i64>,
    /// Problem reported by the last action, shown at the top of the page.
    problem: RwSignal<Option<String>>,
    /// Questions whose answer has been judged in "answer on the go" mode and can no longer be changed.
    locks: RwSignal<HashMap<String, bool>>,
}

impl Session {
    fn answer(&self, question_id: &str) -> Answer {
        self.answers.with(|m| m.get(question_id).cloned().unwrap_or_default())
    }

    /// Updates the local copy at once, then saves it. The worker reports when
    /// the time has run out, and the attempt then moves to review.
    fn save(&self, question_id: String, selected: Vec<String>, flagged: bool) {
        let problem = self.problem;
        self.answers.update(|m| {
            m.insert(question_id.clone(), Answer { selected: selected.clone(), flagged });
        });
        let attempt_id = self.attempt_id.get_value();
        problem.set(None);
        spawn_local(async move {
            match api::save_answer(&attempt_id, &question_id, selected, flagged).await {
                Ok(result) if result.auto_submitted => ui::go(&format!("#/review/{attempt_id}")),
                Ok(_) => {}
                Err(error) => problem.set(Some(error)),
            }
        });
    }

    fn choose(&self, question_id: &str, kind: QuestionKind, option_id: &str, checked: bool) {
        // In "answer on the go" mode a judged answer cannot be changed.
        if self.is_locked(question_id) {
            return;
        }
        let current = self.answer(question_id);
        let mut selected: Vec<String> = match kind {
            QuestionKind::Single => {
                if checked {
                    vec![option_id.to_string()]
                } else {
                    Vec::new()
                }
            }
            QuestionKind::Multi => current
                .selected
                .iter()
                .filter(|x| x.as_str() != option_id)
                .cloned()
                .collect(),
        };
        if kind == QuestionKind::Multi && checked {
            selected.push(option_id.to_string());
        }
        self.save(question_id.to_string(), selected, current.flagged);
    }

    fn set_flag(&self, question_id: &str, flagged: bool) {
        let selected = self.answer(question_id).selected;
        self.save(question_id.to_string(), selected, flagged);
    }

    /// In "answer on the go" mode a judged answer cannot be changed.
    fn is_locked(&self, question_id: &str) -> bool {
        self.locks.with(|m| m.get(question_id).copied().unwrap_or(false))
    }
}

#[component]
pub fn Attempt(ui: Ui, id: String) -> impl IntoView {
    let state = RwSignal::new(None::<Result<AttemptView, String>>);
    let answers = RwSignal::new(HashMap::<String, Answer>::new());
    let index = RwSignal::new(0usize);
    let now = RwSignal::new(ui::now_ms());
    let problem = RwSignal::new(None::<String>);
    let locks = RwSignal::new(HashMap::<String, bool>::new());
    let session = Session {
        attempt_id: StoredValue::new(id.clone()),
        answers,
        index,
        now,
        problem,
        locks,
    };

    // Opening the attempt restarts its clock, if the user left it paused.
    spawn_local(async move {
        let resumed = api::resume_attempt(&id).await;
        let fetched = api::get_attempt(&id).await;
        match (resumed, fetched) {
            (Err(error), _) | (_, Err(error)) => state.set(Some(Err(error))),
            (Ok(()), Ok(view)) if view.status == AttemptStatus::InProgress => {
                let saved: HashMap<String, Answer> = view
                    .answers
                    .iter()
                    .map(|a| {
                        (a.question_id.clone(), Answer { selected: a.selected.clone(), flagged: a.flagged })
                    })
                    .collect();
                answers.set(saved);
                state.set(Some(Ok(view)));
            }
            (Ok(()), Ok(view)) => ui::go(&format!("#/review/{}", view.id)),
        }
    });

    // Stops the clock when the user leaves: on leaving this page, and when the window closes.
    let pause = move || {
        let attempt_id = session.attempt_id.get_value();
        spawn_local(async move {
            // Best effort: a page that is closing may not get to finish this request.
            let _ = api::pause_attempt(&attempt_id).await;
        });
    };
    on_cleanup(move || pause());
    let on_hide = Closure::<dyn FnMut()>::new(move || pause());
    let _ = ui::win().add_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref());
    let on_hide = StoredValue::new_local(on_hide);
    on_cleanup(move || {
        let listener: js_sys::Function =
            on_hide.with_value(|callback| callback.as_ref().unchecked_ref::<js_sys::Function>().clone());
        let _ = ui::win().remove_event_listener_with_callback("pagehide", &listener);
    });

    // Refresh the clock every second.
    let tick = Closure::wrap(Box::new(move || now.set(ui::now_ms())) as Box<dyn FnMut()>);
    let handle = ui::win()
        .set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 1000)
        .unwrap_or(0);
    // Owned by this page, so the closure is freed when the page unmounts.
    let _tick = StoredValue::new_local(tick);
    on_cleanup(move || ui::win().clear_interval_with_handle(handle));

    // When time runs out, ask the worker to finalize the attempt.
    Effect::new(move |_| {
        let current = now.get();
        let deadline = state.with(|s| match s {
            Some(Ok(view)) => view.deadline_at,
            _ => None,
        });
        if let Some(deadline) = deadline {
            if current >= deadline {
                let attempt_id = session.attempt_id.get_value();
                spawn_local(async move {
                    if let Ok(view) = api::get_attempt(&attempt_id).await {
                        if view.status != AttemptStatus::InProgress {
                            ui::go(&format!("#/review/{attempt_id}"));
                        }
                    }
                });
            }
        }
    });

    // Attempt errors show as toasts instead of a panel.
    Effect::new(move |_| {
        if let Some(message) = session.problem.get() {
            ui.notify(message, true);
            session.problem.set(None);
        }
    });

    let submit = move || {
        let attempt_id = session.attempt_id.get_value();
        let ids: Vec<String> = state.with_untracked(|s| match s {
            Some(Ok(view)) => view.questions.iter().map(|q| q.id.clone()).collect(),
            _ => Vec::new(),
        });
        let unanswered = ids.iter().filter(|qid| session.answer(qid.as_str()).selected.is_empty()).count();
        let message = if unanswered > 0 {
            format!("{unanswered} unanswered question(s). Submit anyway?")
        } else {
            "Submit this attempt? You can't change your answers afterwards.".to_string()
        };
        let action = if unanswered > 0 { "Submit anyway" } else { "Submit" };
        ui.ask("Submit attempt", &message, action, false, move || {
            spawn_local(async move {
                match api::submit_attempt(&attempt_id).await {
                    Ok(_) => ui::go(&format!("#/review/{attempt_id}")),
                    Err(error) => problem.set(Some(error)),
                }
            });
        });
    };

    let quit = move || {
        let attempt_id = session.attempt_id.get_value();
        ui.ask(
            "Quit attempt",
            "Quit this attempt? It will be discarded and can't be resumed.",
            "Quit",
            true,
            move || {
                spawn_local(async move {
                    match api::delete_attempt(&attempt_id).await {
                        Ok(()) => ui::go("#/"),
                        Err(error) => problem.set(Some(error)),
                    }
                });
            },
        );
    };

    move || match state.get() {
        None => view! { <p class="text-muted-foreground">"Loading…"</p> }.into_any(),
        Some(Err(error)) => view! { <p class="mb-7 border-t-2 border-bad bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]" role="alert">{error}</p> }.into_any(),
        Some(Ok(view)) => attempt_view(session, view, submit, quit).into_any(),
    }
}

fn attempt_view(
    session: Session,
    view: AttemptView,
    submit: impl Fn() + Copy + 'static,
    quit: impl Fn() + Copy + 'static,
) -> impl IntoView {
    let total = view.questions.len();
    let one = view.one_at_a_time;
    let deadline = view.deadline_at;
    let title = view.quiz_title.clone();
    let nav_ids: Vec<String> = view.questions.iter().map(|q| q.id.clone()).collect();
    let cards: Vec<_> = view
        .questions
        .into_iter()
        .enumerate()
        .map(|(position, q)| {
            view! { <CarouselItem position=position>{question_card(session, q, position, one)}</CarouselItem> }.into_any()
        })
        .collect();

    view! {
        <div class="mb-4 flex flex-wrap items-baseline justify-between gap-x-6 gap-y-3">
            <h1 class="m-0 mb-1.5 font-display font-stretch-[125%] font-black uppercase text-[40px] leading-[1.02] tracking-[-0.01em] max-[600px]:text-[30px]">{title}</h1>
            <div class="border-2 border-ink px-3 py-1.5 font-mono text-lg font-semibold tabular-nums text-ink" role="timer">{move || match deadline {
                Some(d) => format!("Time left {}", ui::fmt_clock(d - session.now.get())),
                None => String::new(),
            }}</div>
        </div>
        {one.then(|| {
            view! {
                <nav class="mb-[18px] flex flex-wrap gap-1.5">
                    {nav_ids
                        .iter()
                        .enumerate()
                        .map(|(position, qid)| nav_button(session, qid.clone(), position))
                        .collect_view()}
                </nav>
            }
        })}
        {if one {
            view! { <Carousel index=session.index count=total><CarouselContent>{cards}</CarouselContent></Carousel> }.into_any()
        } else {
            view! { <div>{cards}</div> }.into_any()
        }}
        <div class="flex flex-wrap items-center gap-2 justify-between mt-3">
            {(one && total >= 2).then(|| {
                view! {
                    <div class="flex flex-wrap items-center gap-2">
                        <CarouselFirst index=session.index/>
                        <CarouselPrevious index=session.index/>
                        <CarouselNext index=session.index count=total/>
                        <CarouselLast index=session.index count=total/>
                    </div>
                }
            })}
            <Button variant=ButtonVariant::Destructive on:click=move |_| quit()>"Quit attempt"</Button>
            <Button variant=ButtonVariant::Default class="ml-auto" on:click=move |_| submit()>"Submit attempt"</Button>
        </div>
    }
}

fn nav_label(session: Session, qid: &str, position: usize) -> String {
    let answer = session.answer(qid);
    let mut label = format!("Question {}", position + 1);
    label.push_str(if answer.selected.is_empty() { ", not answered" } else { ", answered" });
    if answer.flagged {
        label.push_str(", flagged");
    }
    label
}

fn nav_button(session: Session, qid: String, position: usize) -> impl IntoView {
    let label_qid = qid.clone();
    view! {
        <Button
            class=move || {
                let answer = session.answer(&qid);
                let current = session.index.get() == position;
                let mut class = String::from("min-h-11 min-w-11 p-0 border-2 border-ink bg-field-bg font-mono text-sm font-semibold normal-case tracking-normal text-field-ink cursor-pointer hover:not-disabled:bg-ink hover:not-disabled:text-surface disabled:cursor-not-allowed disabled:opacity-50");
                if !answer.selected.is_empty() {
                    class.push_str(" bg-muted");
                }
                if current {
                    class.push_str(" bg-primary border-primary text-primary-foreground");
                }
                if answer.flagged {
                    class.push_str(if current {
                        " border-[#8a5a0b] text-primary-foreground"
                    } else {
                        " border-[#8a5a0b] text-[#8a5a0b]"
                    });
                }
                class
            }
            attr:aria-label=move || nav_label(session, &label_qid, position)
            attr:aria-current=move || if session.index.get() == position { "step" } else { "false" }
            on:click=move |_| session.index.set(position)>
            {position + 1}
        </Button>
    }
}

fn question_card(session: Session, q: AttemptQuestionView, position: usize, one: bool) -> impl IntoView {
    let qid = StoredValue::new(q.id.clone());
    let kind = q.kind;

    // A single-answer question shares one RadioGroup value. Each change is saved from this effect.
    let picked = RwSignal::new(session.answer(&q.id).selected.first().cloned().unwrap_or_default());
    Effect::new(move |first: Option<()>| {
        let value = picked.get();
        if first.is_some() {
            let id = qid.get_value();
            // Untracked, so this effect does not re-run when the answers change.
            let flagged = session.answers.with_untracked(|m| m.get(&id).is_some_and(|a| a.flagged));
            let selected = if value.is_empty() { Vec::new() } else { vec![value] };
            session.save(id, selected, flagged);
        }
    });

    // Empty unless the quiz gives feedback on the go, so no feedback is shown otherwise.
    let show_feedback = !q.correct_option_ids.is_empty();
    let correct_key = StoredValue::new(q.correct_option_ids.clone());
    let correct_text = StoredValue::new(
        q.options
            .iter()
            .filter(|option| q.correct_option_ids.contains(&option.id))
            .map(|option| option.text.clone())
            .collect::<Vec<_>>()
            .join(", "),
    );
    let explanation = StoredValue::new(q.explanation.clone());

    // In "answer on the go" mode a judged answer locks further changes.
    Effect::new(move |_: Option<()>| {
        let chosen = session.answer(&qid.get_value()).selected;
        let needed = correct_key.with_value(|ids| ids.len());
        if show_feedback && chosen.len() >= needed {
            session.locks.update(|m| { m.insert(qid.get_value(), true); });
        }
    });

    // Built inside the group so each item can read the group's value.
    let option_list = q.options.clone();
    let build_choices = move || -> Vec<AnyView> {
        option_list.clone().into_iter().map(|o| {
            let oid = StoredValue::new(o.id.clone());
            let text = o.text;
            match kind {
                QuestionKind::Single => view! {
                    <label class="flex cursor-pointer items-start gap-3 border-[1.5px] border-ink bg-field-bg px-3.5 py-3 text-field-ink hover:border-primary has-[[aria-checked=true]]:border-2 has-[[aria-checked=true]]:border-primary has-[[aria-checked=true]]:bg-muted [&>span]:whitespace-pre-wrap">
                        <RadioGroupItem value=oid.get_value() disabled=Signal::derive(move || session.is_locked(&qid.get_value())) />
                        <span>{text}</span>
                    </label>
                }
                .into_any(),
                QuestionKind::Multi => view! {
                    <label class="flex cursor-pointer items-start gap-3 border-[1.5px] border-ink bg-field-bg px-3.5 py-3 text-field-ink hover:border-primary has-[[aria-checked=true]]:border-2 has-[[aria-checked=true]]:border-primary has-[[aria-checked=true]]:bg-muted [&>span]:whitespace-pre-wrap">
                        <Checkbox
                            aria_label=text.clone()
                            disabled=Signal::derive(move || session.is_locked(&qid.get_value()))
                            checked=Signal::derive(move || session.answer(&qid.get_value()).selected.contains(&oid.get_value()))
                            on_checked_change=Callback::new(move |v: bool| session.choose(&qid.get_value(), kind, &oid.get_value(), v))/>
                        <span>{text}</span>
                    </label>
                }
                .into_any(),
            }
        })
        .collect::<Vec<AnyView>>()
    };
    let options = match kind {
        QuestionKind::Single => view! { <RadioGroup class="flex flex-col gap-2.5" value=picked>{build_choices()}</RadioGroup> }.into_any(),
        QuestionKind::Multi => view! { <div class="flex flex-col gap-2.5">{build_choices()}</div> }.into_any(),
    };

    // Shown once the question has an answer, when the quiz marks answers as they go.
    let feedback = move || {
        let mut chosen = session.answer(&qid.get_value()).selected;
        // Judge only once every correct option has been picked, so a partly
        // answered multiple-answer question shows nothing yet.
        let needed = correct_key.with_value(|ids| ids.len());
        if !show_feedback || chosen.len() < needed {
            return None;
        }
        chosen.sort();
        let correct = correct_key.with_value(|ids| {
            let mut ids = ids.clone();
            ids.sort();
            ids == chosen
        });
        let (class, strong, text) = if correct {
            ("mt-3.5 border-2 border-ok px-3.5 py-3", "mb-1 block text-ok", "Correct.".to_string())
        } else {
            (
                "mt-3.5 border-2 border-bad px-3.5 py-3",
                "mb-1 block text-bad",
                format!("Not quite. The correct answer is {}.", correct_text.get_value()),
            )
        };
        let explanation = explanation.get_value();
        Some(view! {
            <div class={class} role="status">
                <strong class={strong}>{text}</strong>
                {(!explanation.is_empty()).then(|| view! { <p class="mt-1 mb-0">{explanation}</p> })}
            </div>
        })
    };

    view! {
        <section class="mb-[18px] bg-surface p-0 [overflow-wrap:anywhere]" class:hidden=move || one && session.index.get() != position>
            <div class="mb-3 flex flex-wrap items-center justify-between gap-2">
                <h2 class="m-0 font-display font-stretch-[112%] text-[15px] font-extrabold uppercase tracking-[0.06em]">{format!("Question {}", position + 1)}</h2>
                <span class="text-muted-foreground">{if kind == QuestionKind::Single { "Choose one" } else { "Choose all that apply" }}</span>
                <label class="flex cursor-pointer items-center gap-1.5 text-[13px] text-muted-foreground">
                    <Checkbox
                        aria_label="Flag this question"
                        checked=Signal::derive(move || session.answer(&qid.get_value()).flagged)
                        on_checked_change=Callback::new(move |v: bool| session.set_flag(&qid.get_value(), v))/>
                    "Flag"
                </label>
            </div>
            <p class="mt-2 mb-4 whitespace-pre-wrap text-[17px] leading-[1.5] [overflow-wrap:anywhere]">{q.prompt}</p>
            {options}
            {feedback}
        </section>
    }
}

#[component]
pub fn Review(ui: Ui, id: String) -> impl IntoView {
    let state = RwSignal::new(None::<Result<AttemptReview, String>>);
    spawn_local(async move {
        state.set(Some(api::get_review(&id).await));
    });

    move || match state.get() {
        None => view! { <p class="text-muted-foreground">"Loading…"</p> }.into_any(),
        Some(Err(error)) => view! { <p class="text-bad">{error}</p> }.into_any(),
        Some(Ok(review)) => review_view(ui, review).into_any(),
    }
}

fn review_view(ui: Ui, review: AttemptReview) -> impl IntoView {
    let a = review.attempt.clone();
    let retake_quiz = a.quiz_id.clone();
    let passed = a.passed == Some(true);
    let questions: Vec<_> = review
        .questions
        .into_iter()
        .enumerate()
        .map(|(i, q)| review_question(i, q))
        .collect();

    view! {
        <div class="mb-6 flex flex-wrap items-end justify-between gap-x-6 gap-y-3 border-t-[6px] border-ink pt-4">
            <div>
                <h1 class="m-0 mb-1.5 font-display font-stretch-[125%] font-black uppercase text-[40px] leading-[1.02] tracking-[-0.01em] max-[600px]:text-[30px]">{a.quiz_title.clone()}</h1>
                <p class="text-muted-foreground">{format!("{} · {}", ui::fmt_date(a.started_at), ui::status_label(a.status))}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
                <Button variant=ButtonVariant::Outline href="#/".to_string()>"Library"</Button>
                {retake_quiz.map(|quiz_id| {
                    view! { <Button variant=ButtonVariant::Default on:click=move |_| retake(ui, quiz_id.clone())>"Retake"</Button> }
                })}
            </div>
        </div>
        <section class="mb-7 border-b-2 border-ink bg-surface px-5 py-[34px] text-center [overflow-wrap:anywhere]">
            <div class="font-display font-stretch-[125%] font-black text-[84px] leading-none text-primary tabular-nums max-[600px]:text-[64px]">{ui::pct(a.score_percent)}</div>
            <div>{format!("{} of {} correct", a.correct_count.unwrap_or(0), a.total_count)}</div>
            {review.pass_percent.map(|pass| {
                let (class, text) = if passed {
                    ("font-display font-stretch-[112%] font-extrabold uppercase tracking-[0.06em] text-ok", "Passed".to_string())
                } else {
                    ("font-display font-stretch-[112%] font-extrabold uppercase tracking-[0.06em] text-bad", format!("Not passed (pass mark {pass}%)"))
                };
                view! { <div class={class}>{text}</div> }
            })}
            <p class="mx-auto mt-3.5 mb-2.5 max-w-[56ch]">{result_note(a.passed, a.correct_count.unwrap_or(0), a.total_count)}</p>
            <div class="text-muted-foreground">{format!("Time taken: {}", ui::fmt_duration(a.duration_ms))}</div>
        </section>
        {questions}
    }
}

/// One calm line under the result, in the product's own words.
fn result_note(passed: Option<bool>, correct: u32, total: u32) -> &'static str {
    if total > 0 && correct == total {
        "Every answer is correct."
    } else {
        match passed {
            Some(true) => "You met the pass mark. Look over the questions you missed to strengthen the next attempt.",
            Some(false) => "Not this time. The questions you missed below show where to focus. Retake when you're ready.",
            None => "Look over the questions you missed below to see where to focus next.",
        }
    }
}

fn review_question(index: usize, q: ReviewQuestion) -> impl IntoView {
    let (class, badge_class, badge_text) = if q.is_correct {
        ("mb-7 border-t-2 border-t-ok bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]", "text-[var(--ok)] border-[var(--ok)]", "Correct")
    } else {
        ("mb-7 border-t-2 border-t-bad bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]", "text-[var(--bad)] border-[var(--bad)]", "Incorrect")
    };
    let options: Vec<_> = q
        .options
        .into_iter()
        .map(|o| {
            let mut class = String::from("mb-1 px-2.5 py-2 text-muted-foreground [&>svg]:mr-1.5 [&>svg]:inline-block");
            if o.is_correct {
                class.push_str(" font-semibold text-ok");
            } else if o.selected {
                class.push_str(" text-bad");
            }
            let mark = if o.is_correct {
                icons::check().into_any()
            } else if o.selected {
                icons::cross().into_any()
            } else {
                icons::dot().into_any()
            };
            let yours = if o.selected && !o.is_correct { " (your answer)" } else { "" };
            let correct = if o.is_correct { " (correct answer)" } else { "" };
            view! { <li class={class}>{mark}{o.text}{yours}{correct}</li> }
        })
        .collect();
    let explanation = q.explanation.clone();

    view! {
        <section class={class}>
            <div class="mb-3 flex flex-wrap items-center justify-between gap-2">
                <h2 class="m-0 font-display font-stretch-[112%] text-[15px] font-extrabold uppercase tracking-[0.06em]">{format!("Question {}", index + 1)}</h2>
                <Badge variant=BadgeVariant::Outline class=badge_class>{badge_text}</Badge>
                {q.flagged.then(|| view! { <Badge variant=BadgeVariant::Outline class="text-[var(--warn)] border-[var(--warn)]">"Flagged"</Badge> })}
            </div>
            <p class="mt-2 mb-4 whitespace-pre-wrap text-[17px] leading-[1.5] [overflow-wrap:anywhere]">{q.prompt}</p>
            <ul class="my-2.5 list-none p-0">{options}</ul>
            {(!explanation.is_empty()).then(|| {
                view! { <p class="border-[1.5px] border-ink bg-surface px-3.5 py-3"><strong>"Explanation: "</strong>{explanation}</p> }
            })}
        </section>
    }
}

fn retake(ui: Ui, quiz_id: String) {
    spawn_local(async move {
        match api::start_attempt(&quiz_id, ui::new_seed()).await {
            Ok(attempt) => ui::go(&format!("#/attempt/{}", attempt.id)),
            Err(error) => ui.notify(error, true),
        }
    });
}
