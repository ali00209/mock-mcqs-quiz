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
use crate::ui::{self, Ui};

#[derive(Clone, Default)]
struct Answer {
    selected: Vec<String>,
    flagged: bool,
}

/// Handles shared by every part of the attempt screen. All fields are Copy.
#[derive(Clone, Copy)]
struct Session {
    ui: Ui,
    attempt_id: StoredValue<String>,
    answers: RwSignal<HashMap<String, Answer>>,
    index: RwSignal<usize>,
    now: RwSignal<i64>,
}

impl Session {
    fn answer(&self, question_id: &str) -> Answer {
        self.answers.with(|m| m.get(question_id).cloned().unwrap_or_default())
    }

    /// Updates the local copy at once, then saves it. The worker reports when
    /// the time has run out, and the attempt then moves to review.
    fn save(&self, question_id: String, selected: Vec<String>, flagged: bool) {
        let ui = self.ui;
        self.answers.update(|m| {
            m.insert(question_id.clone(), Answer { selected: selected.clone(), flagged });
        });
        let attempt_id = self.attempt_id.get_value();
        spawn_local(async move {
            match api::save_answer(&attempt_id, &question_id, selected, flagged).await {
                Ok(result) if result.auto_submitted => ui::go(&format!("#/review/{attempt_id}")),
                Ok(_) => {}
                Err(error) => ui.notify(error, true),
            }
        });
    }

    fn choose(&self, question_id: &str, kind: QuestionKind, option_id: &str, checked: bool) {
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
}

#[component]
pub fn Attempt(ui: Ui, id: String) -> impl IntoView {
    let state = RwSignal::new(None::<Result<AttemptView, String>>);
    let answers = RwSignal::new(HashMap::<String, Answer>::new());
    let index = RwSignal::new(0usize);
    let now = RwSignal::new(ui::now_ms());
    let session = Session {
        ui,
        attempt_id: StoredValue::new(id.clone()),
        answers,
        index,
        now,
    };

    spawn_local(async move {
        match api::get_attempt(&id).await {
            Ok(view) if view.status == AttemptStatus::InProgress => {
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
            Ok(view) => ui::go(&format!("#/review/{}", view.id)),
            Err(error) => state.set(Some(Err(error))),
        }
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
        if !ui::confirm(&message) {
            return;
        }
        spawn_local(async move {
            match api::submit_attempt(&attempt_id).await {
                Ok(_) => ui::go(&format!("#/review/{attempt_id}")),
                Err(error) => ui.notify(error, true),
            }
        });
    };

    move || match state.get() {
        None => view! { <p class="muted">"Loading…"</p> }.into_any(),
        Some(Err(error)) => view! { <p class="error">{error}</p> }.into_any(),
        Some(Ok(view)) => attempt_view(session, view, submit).into_any(),
    }
}

fn attempt_view(session: Session, view: AttemptView, submit: impl Fn() + Copy + 'static) -> impl IntoView {
    let total = view.questions.len();
    let one = view.one_at_a_time;
    let deadline = view.deadline_at;
    let title = view.quiz_title.clone();
    let nav_ids: Vec<String> = view.questions.iter().map(|q| q.id.clone()).collect();
    let cards: Vec<_> = view
        .questions
        .into_iter()
        .enumerate()
        .map(|(position, q)| question_card(session, q, position, one))
        .collect();

    view! {
        <div class="attempt-head">
            <h1>{title}</h1>
            <div class="timer" role="timer">{move || match deadline {
                Some(d) => format!("Time left {}", ui::fmt_clock(d - session.now.get())),
                None => String::new(),
            }}</div>
        </div>
        {one.then(|| {
            view! {
                <nav class="qnav">
                    {nav_ids
                        .iter()
                        .enumerate()
                        .map(|(position, qid)| nav_button(session, qid.clone(), position))
                        .collect_view()}
                </nav>
            }
        })}
        <div class="questions">{cards}</div>
        <div class="actions attempt-actions">
            {one.then(|| {
                view! {
                    <button disabled=move || session.index.get() == 0
                        on:click=move |_| session.index.update(|i| *i = i.saturating_sub(1))>"Previous"</button>
                }
            })}
            {one.then(|| {
                view! {
                    <button disabled=move || session.index.get() + 1 >= total
                        on:click=move |_| session.index.update(|i| *i += 1)>"Next"</button>
                }
            })}
            <button class="primary" on:click=move |_| submit()>"Submit attempt"</button>
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
        <button
            class=move || {
                let answer = session.answer(&qid);
                let mut class = String::from("qbtn");
                if session.index.get() == position {
                    class.push_str(" current");
                }
                if !answer.selected.is_empty() {
                    class.push_str(" answered");
                }
                if answer.flagged {
                    class.push_str(" flagged");
                }
                class
            }
            aria-label=move || nav_label(session, &label_qid, position)
            aria-current=move || if session.index.get() == position { "step" } else { "false" }
            on:click=move |_| session.index.set(position)>
            {position + 1}
        </button>
    }
}

fn question_card(session: Session, q: AttemptQuestionView, position: usize, one: bool) -> impl IntoView {
    let qid = StoredValue::new(q.id.clone());
    let kind = q.kind;
    let input_type = if kind == QuestionKind::Single { "radio" } else { "checkbox" };
    let group = format!("q-{}", q.id);

    let choices = q
        .options
        .into_iter()
        .map(|o| {
            let oid = StoredValue::new(o.id.clone());
            let group = group.clone();
            view! {
                <label class="choice">
                    <input type={input_type} name={group}
                        prop:checked=move || session.answer(&qid.get_value()).selected.contains(&oid.get_value())
                        on:change=move |ev| session.choose(&qid.get_value(), kind, &oid.get_value(), event_target_checked(&ev))/>
                    <span>{o.text}</span>
                </label>
            }
        })
        .collect_view();

    view! {
        <section class="panel question" class:hidden=move || one && session.index.get() != position>
            <div class="question-head">
                <strong>{format!("Question {}", position + 1)}</strong>
                <span class="muted">{if kind == QuestionKind::Single { "Choose one" } else { "Choose all that apply" }}</span>
                <label class="flag">
                    <input type="checkbox"
                        prop:checked=move || session.answer(&qid.get_value()).flagged
                        on:change=move |ev| session.set_flag(&qid.get_value(), event_target_checked(&ev))/>
                    " Flag"
                </label>
            </div>
            <p class="prompt">{q.prompt}</p>
            <div class="choices">{choices}</div>
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
        None => view! { <p class="muted">"Loading…"</p> }.into_any(),
        Some(Err(error)) => view! { <p class="error">{error}</p> }.into_any(),
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
        <div class="page-head">
            <div>
                <h1>{a.quiz_title.clone()}</h1>
                <p class="muted">{format!("{} · {}", ui::fmt_date(a.started_at), ui::status_label(a.status))}</p>
            </div>
            <div class="actions">
                <a class="button" href="#/">"Library"</a>
                {retake_quiz.map(|quiz_id| {
                    view! { <button class="primary" on:click=move |_| retake(ui, quiz_id.clone())>"Retake"</button> }
                })}
            </div>
        </div>
        <section class="panel score">
            <div class="big">{ui::pct(a.score_percent)}</div>
            <div>{format!("{} of {} correct", a.correct_count.unwrap_or(0), a.total_count)}</div>
            {review.pass_percent.map(|pass| {
                let (class, text) = if passed {
                    ("pass", "Passed".to_string())
                } else {
                    ("fail", format!("Not passed (pass mark {pass}%)"))
                };
                view! { <div class={class}>{text}</div> }
            })}
            <div class="muted">{format!("Time taken: {}", ui::fmt_duration(a.duration_ms))}</div>
        </section>
        {questions}
    }
}

fn review_question(index: usize, q: ReviewQuestion) -> impl IntoView {
    let (class, badge_class, badge_text) = if q.is_correct {
        ("panel review-q ok", "badge ok", "Correct")
    } else {
        ("panel review-q bad", "badge bad", "Incorrect")
    };
    let options: Vec<_> = q
        .options
        .into_iter()
        .map(|o| {
            let mut class = String::from("opt");
            if o.is_correct {
                class.push_str(" correct");
            }
            if o.selected {
                class.push_str(" picked");
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
            <div class="question-head">
                <strong>{format!("Question {}", index + 1)}</strong>
                <span class={badge_class}>{badge_text}</span>
                {q.flagged.then(|| view! { <span class="badge warn">"Flagged"</span> })}
            </div>
            <p class="prompt">{q.prompt}</p>
            <ul class="review-options">{options}</ul>
            {(!explanation.is_empty()).then(|| {
                view! { <p class="explanation"><strong>"Explanation: "</strong>{explanation}</p> }
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
