// Quiz editor. Edits a local copy (`draft`) and saves it through the worker,
// which runs the same validation rules as the domain crate.
use leptos::prelude::*;
use leptos::task::spawn_local;
use mocktest_domain::{
    AnswerOption, MAX_OPTIONS, MAX_QUESTIONS, MIN_OPTIONS, Question, QuestionKind, Quiz, QuizSource,
    parse_quiz_text,
};

use crate::api;
use crate::icons;
use crate::ui::{self, Ui};

#[component]
pub fn Editor(ui: Ui, id: String) -> impl IntoView {
    let loaded = RwSignal::new(None::<Result<Quiz, String>>);
    spawn_local(async move {
        loaded.set(Some(api::get_quiz(&id).await));
    });

    move || match loaded.get() {
        None => view! { <p class="muted">"Loading…"</p> }.into_any(),
        Some(Err(error)) => view! { <p class="error">{error}</p> }.into_any(),
        Some(Ok(quiz)) if quiz.source == QuizSource::Sample => {
            let id = quiz.id.clone();
            view! {
                <section class="panel">
                    <h1>{quiz.title.clone()}</h1>
                    <p class="muted">"Sample quizzes are read-only. Make an editable copy to change it."</p>
                    <button class="primary" on:click=move |_| copy_for_editing(ui, id.clone())>
                        "Make an editable copy"
                    </button>
                </section>
            }
            .into_any()
        }
        Some(Ok(quiz)) => view! { <EditForm ui=ui quiz=quiz/> }.into_any(),
    }
}

fn copy_for_editing(ui: Ui, id: String) {
    spawn_local(async move {
        match api::duplicate_quiz(&id).await {
            Ok(copy) => ui::go(&format!("#/edit/{}", copy.id)),
            Err(error) => ui.notify(error, true),
        }
    });
}

fn parse_minutes(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // Anything unparseable becomes 0 so validation reports it.
    Some(text.parse::<f64>().map(|m| (m * 60.0).round().max(0.0) as u32).unwrap_or(0))
}

fn parse_percent(text: &str) -> Option<u8> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.parse::<u8>().unwrap_or(0))
}

#[component]
fn EditForm(ui: Ui, quiz: Quiz) -> impl IntoView {
    let draft = RwSignal::new(quiz.clone());
    let tags_text = RwSignal::new(quiz.tags.join(", "));
    let minutes_text = RwSignal::new(
        quiz.settings
            .time_limit_secs
            .map(|s| (f64::from(s) / 60.0).to_string())
            .unwrap_or_default(),
    );
    let pass_text = RwSignal::new(quiz.settings.pass_percent.map(|p| p.to_string()).unwrap_or_default());
    let issues = RwSignal::new(Vec::<String>::new());

    let save = move || {
        let mut next = draft.get_untracked();
        next.tags = tags_text.get_untracked().split(',').map(|t| t.to_string()).collect();
        next.settings.time_limit_secs = parse_minutes(&minutes_text.get_untracked());
        next.settings.pass_percent = parse_percent(&pass_text.get_untracked());
        let next = next.normalized();

        let found: Vec<String> = next.validation_issues().into_iter().map(|issue| issue.message).collect();
        let blocked = !found.is_empty();
        issues.set(found);
        if blocked {
            return;
        }
        spawn_local(async move {
            match api::save_quiz(next).await {
                Ok(saved) => {
                    draft.set(saved);
                    ui.notify("Saved.", false);
                }
                Err(error) => ui.notify(error, true),
            }
        });
    };

    view! {
        <div class="page-head">
            <h1>"Edit quiz"</h1>
            <div class="actions">
                <a class="button" href="#/">"Back"</a>
                <button class="primary" on:click=move |_| save()>"Save"</button>
            </div>
        </div>
        {move || {
            (!issues.get().is_empty()).then(|| {
                view! {
                    <div class="panel error" role="alert">
                        <strong>"Fix these before saving:"</strong>
                        <ul>{issues.get().into_iter().map(|issue| view! { <li>{issue}</li> }).collect_view()}</ul>
                    </div>
                }
            })
        }}

        <section class="panel">
            <h2>"Details"</h2>
            <label class="field">
                <span>"Title"</span>
                <input type="text" prop:value=move || draft.with(|q| q.title.clone())
                    on:input=move |ev| draft.update(|q| q.title = event_target_value(&ev))/>
            </label>
            <label class="field">
                <span>"Description"</span>
                <textarea rows="3" prop:value=move || draft.with(|q| q.description.clone())
                    on:input=move |ev| draft.update(|q| q.description = event_target_value(&ev))></textarea>
            </label>
            <label class="field">
                <span>"Category"</span>
                <input type="text" prop:value=move || draft.with(|q| q.category.clone().unwrap_or_default())
                    on:input=move |ev| draft.update(|q| q.category = Some(event_target_value(&ev)))/>
            </label>
            <label class="field">
                <span>"Tags (comma separated)"</span>
                <input type="text" prop:value=move || tags_text.get()
                    on:input=move |ev| tags_text.set(event_target_value(&ev))/>
            </label>
        </section>

        <section class="panel">
            <h2>"Settings"</h2>
            <label class="field">
                <span>"Time limit in minutes (blank for none)"</span>
                <input type="number" min="0" prop:value=move || minutes_text.get()
                    on:input=move |ev| minutes_text.set(event_target_value(&ev))/>
            </label>
            <label class="field">
                <span>"Pass mark in percent (blank for none)"</span>
                <input type="number" min="0" max="100" prop:value=move || pass_text.get()
                    on:input=move |ev| pass_text.set(event_target_value(&ev))/>
            </label>
            <label class="check">
                <input type="checkbox"
                    prop:checked=move || draft.with(|q| q.settings.shuffle_questions)
                    on:change=move |ev| draft.update(|q| q.settings.shuffle_questions = event_target_checked(&ev))/>
                <span>"Shuffle questions"</span>
            </label>
            <label class="check">
                <input type="checkbox"
                    prop:checked=move || draft.with(|q| q.settings.shuffle_options)
                    on:change=move |ev| draft.update(|q| q.settings.shuffle_options = event_target_checked(&ev))/>
                <span>"Shuffle answer options"</span>
            </label>
            <label class="check">
                <input type="checkbox"
                    prop:checked=move || draft.with(|q| q.settings.one_at_a_time)
                    on:change=move |ev| draft.update(|q| q.settings.one_at_a_time = event_target_checked(&ev))/>
                <span>"Show one question at a time"</span>
            </label>
        </section>

        {text_import(ui, draft)}

        <section class="panel">
            <h2>"Questions"</h2>
            <For
                each=move || draft.get().questions
                key=|q| q.id.clone()
                children=move |q: Question| question_editor(draft, q)
            />
            <button
                disabled=move || draft.with(|q| q.questions.len() >= MAX_QUESTIONS)
                on:click=move |_| draft.update(|q| q.questions.push(Question::blank()))>
                "Add question"
            </button>
        </section>

        <div class="save-bar">
            <button class="primary" on:click=move |_| save()>"Save quiz"</button>
        </div>
    }
}

fn question_editor(draft: RwSignal<Quiz>, q: Question) -> impl IntoView {
    let qid = StoredValue::new(q.id.clone());
    let index = move || draft.with(|d| d.questions.iter().position(|x| x.id == qid.get_value()).unwrap_or(0));
    let count = move || draft.with(|d| d.questions.len());

    view! {
        <div class="question">
            <div class="question-head">
                <strong>{move || format!("Question {}", index() + 1)}</strong>
                <div class="actions">
                    <button disabled=move || index() == 0
                        on:click=move |_| move_question(draft, qid.get_value(), -1)>"Up"</button>
                    <button disabled=move || index() + 1 >= count()
                        on:click=move |_| move_question(draft, qid.get_value(), 1)>"Down"</button>
                    <button class="danger"
                        on:click=move |_| draft.update(|d| d.questions.retain(|x| x.id != qid.get_value()))>
                        "Remove"
                    </button>
                </div>
            </div>
            <label class="field">
                <span>"Prompt"</span>
                <textarea rows="2" on:input=move |ev| {
                    let text = event_target_value(&ev);
                    update_question(draft, &qid.get_value(), |x| x.prompt = text);
                }>{q.prompt.clone()}</textarea>
            </label>
            <label class="field">
                <span>"Type"</span>
                <select
                    prop:value=move || kind_name(draft, &qid.get_value())
                    on:change=move |ev| set_kind(draft, &qid.get_value(), &event_target_value(&ev))>
                    <option value="single">"Single answer"</option>
                    <option value="multi">"Multiple answers"</option>
                </select>
            </label>
            <div class="options">
                <For
                    each=move || question_options(draft, &qid.get_value())
                    key=|o| o.id.clone()
                    children=move |o: AnswerOption| option_row(draft, qid, o)
                />
            </div>
            <div class="actions">
                <button
                    disabled=move || question_option_count(draft, &qid.get_value()) >= MAX_OPTIONS
                    on:click=move |_| add_option(draft, &qid.get_value())>
                    "Add option"
                </button>
            </div>
            <label class="field">
                <span>"Explanation (shown after submitting)"</span>
                <textarea rows="2" on:input=move |ev| {
                    let text = event_target_value(&ev);
                    update_question(draft, &qid.get_value(), |x| x.explanation = text);
                }>{q.explanation.clone()}</textarea>
            </label>
        </div>
    }
}

fn option_number(draft: RwSignal<Quiz>, qid: &str, oid: &str) -> usize {
    draft.with(|d| {
        d.questions
            .iter()
            .find(|q| q.id == qid)
            .and_then(|q| q.options.iter().position(|o| o.id == oid))
            .map_or(0, |i| i + 1)
    })
}

fn option_row(draft: RwSignal<Quiz>, qid: StoredValue<String>, o: AnswerOption) -> impl IntoView {
    let oid = StoredValue::new(o.id.clone());
    view! {
        <div class="option-row">
            <label class="correct">
                <input type="checkbox"
                    prop:checked=move || option_is_correct(draft, &qid.get_value(), &oid.get_value())
                    on:change=move |ev| mark_correct(draft, &qid.get_value(), &oid.get_value(), event_target_checked(&ev))/>
                <span>"Correct"</span>
            </label>
            <input type="text" value={o.text.clone()} placeholder="Option text"
                aria-label=move || format!("Option {} text", option_number(draft, &qid.get_value(), &oid.get_value()))
                on:input=move |ev| {
                    let text = event_target_value(&ev);
                    update_option(draft, &qid.get_value(), &oid.get_value(), |x| x.text = text);
                }/>
            <button class="icon-button"
                disabled=move || question_option_count(draft, &qid.get_value()) <= MIN_OPTIONS
                title="Remove option"
                aria-label=move || format!("Remove option {}", option_number(draft, &qid.get_value(), &oid.get_value()))
                on:click=move |_| remove_option(draft, &qid.get_value(), &oid.get_value())>
                {icons::cross()}
            </button>
        </div>
    }
}



fn update_question(draft: RwSignal<Quiz>, qid: &str, f: impl FnOnce(&mut Question)) {
    draft.update(|d| {
        if let Some(q) = d.questions.iter_mut().find(|q| q.id == qid) {
            f(q);
        }
    });
}

fn update_option(draft: RwSignal<Quiz>, qid: &str, oid: &str, f: impl FnOnce(&mut AnswerOption)) {
    update_question(draft, qid, |q| {
        if let Some(o) = q.options.iter_mut().find(|o| o.id == oid) {
            f(o);
        }
    });
}

fn question_options(draft: RwSignal<Quiz>, qid: &str) -> Vec<AnswerOption> {
    draft.with(|d| {
        d.questions
            .iter()
            .find(|q| q.id == qid)
            .map(|q| q.options.clone())
            .unwrap_or_default()
    })
}

fn question_option_count(draft: RwSignal<Quiz>, qid: &str) -> usize {
    draft.with(|d| d.questions.iter().find(|q| q.id == qid).map_or(0, |q| q.options.len()))
}

fn option_is_correct(draft: RwSignal<Quiz>, qid: &str, oid: &str) -> bool {
    draft.with(|d| {
        d.questions
            .iter()
            .find(|q| q.id == qid)
            .and_then(|q| q.options.iter().find(|o| o.id == oid))
            .is_some_and(|o| o.is_correct)
    })
}

fn kind_name(draft: RwSignal<Quiz>, qid: &str) -> &'static str {
    draft.with(|d| match d.questions.iter().find(|q| q.id == qid).map(|q| q.kind) {
        Some(QuestionKind::Multi) => "multi",
        _ => "single",
    })
}

fn set_kind(draft: RwSignal<Quiz>, qid: &str, value: &str) {
    let kind = if value == "multi" { QuestionKind::Multi } else { QuestionKind::Single };
    update_question(draft, qid, |q| {
        q.kind = kind;
        if kind == QuestionKind::Single {
            // Keep only the first correct option for single-answer questions.
            let mut seen = false;
            for o in &mut q.options {
                if o.is_correct && seen {
                    o.is_correct = false;
                }
                seen |= o.is_correct;
            }
        }
    });
}

fn mark_correct(draft: RwSignal<Quiz>, qid: &str, oid: &str, checked: bool) {
    update_question(draft, qid, |q| {
        if q.kind == QuestionKind::Single && checked {
            for o in &mut q.options {
                o.is_correct = false;
            }
        }
        if let Some(o) = q.options.iter_mut().find(|o| o.id == oid) {
            o.is_correct = checked;
        }
    });
}

fn add_option(draft: RwSignal<Quiz>, qid: &str) {
    update_question(draft, qid, |q| {
        if q.options.len() < MAX_OPTIONS {
            let position = q.options.len();
            q.options.push(AnswerOption::blank(position));
        }
    });
}

fn remove_option(draft: RwSignal<Quiz>, qid: &str, oid: &str) {
    update_question(draft, qid, |q| {
        if q.options.len() > MIN_OPTIONS {
            q.options.retain(|o| o.id != oid);
        }
    });
}

fn move_question(draft: RwSignal<Quiz>, qid: String, delta: isize) {
    draft.update(|d| {
        if let Some(i) = d.questions.iter().position(|q| q.id == qid) {
            let target = i as isize + delta;
            if target >= 0 && (target as usize) < d.questions.len() {
                d.questions.swap(i, target as usize);
            }
        }
    });
}

// ---------- Import from text ----------

const FORMAT_HELP: &str = "\
1. What is the powerhouse of the cell?
A) Nucleus
B) Mitochondria *              (* marks the correct answer; write \\* for a literal star)
C) Ribosome
Explanation: Mitochondria make ATP.

2. Pick the primary colours (multi)
a) Red *
b) Green
c) Blue *
d) Yellow";

fn text_import(ui: Ui, draft: RwSignal<Quiz>) -> impl IntoView {
    let text = RwSignal::new(String::new());
    let preview = RwSignal::new(None::<Result<Vec<Question>, Vec<String>>>);

    // `replace` swaps out the existing questions; otherwise the new ones go on the end.
    let apply = move |replace: bool| {
        let Some(Ok(questions)) = preview.get_untracked() else {
            return;
        };
        let count = questions.len();
        let current = draft.with_untracked(|q| q.questions.len());
        if !replace && current + count > MAX_QUESTIONS {
            ui.notify(format!("A quiz can have at most {MAX_QUESTIONS} questions."), true);
            return;
        }
        draft.update(|q| {
            if replace {
                q.questions = questions;
            } else {
                q.questions.extend(questions);
            }
        });
        preview.set(None);
        ui.notify(format!("Added {count} question(s). Check them below, then Save."), false);
    };

    view! {
        <section class="panel">
            <h2>"Import questions from text"</h2>
            <p class="muted">
                "Paste questions and answers. Put * after each correct option. The quiz title and settings are set in the fields above."
            </p>
            <details>
                <summary>"Show the format"</summary>
                <pre class="format">{FORMAT_HELP}</pre>
            </details>
            <textarea rows="8" placeholder="Paste your questions here"
                on:input=move |ev| text.set(event_target_value(&ev))></textarea>
            <div class="actions">
                <button on:click=move |_| preview.set(Some(parse_quiz_text(&text.get_untracked())))>
                    "Check text"
                </button>
            </div>
            {move || {
                preview.get().map(|result| match result {
                    Err(errors) => view! {
                        <div class="panel error" role="alert">
                            <strong>"Fix these, then check again:"</strong>
                            <ul>{errors.into_iter().map(|e| view! { <li>{e}</li> }).collect_view()}</ul>
                        </div>
                    }
                    .into_any(),
                    Ok(questions) => {
                        let names: Vec<String> = questions
                            .iter()
                            .map(|q| q.prompt.lines().next().unwrap_or_default().to_string())
                            .collect();
                        let count = names.len();
                        view! {
                            <div class="panel">
                                <p>{format!("Found {count} question(s):")}</p>
                                <ol>{names.into_iter().map(|name| view! { <li>{name}</li> }).collect_view()}</ol>
                                <div class="actions">
                                    <button class="primary" on:click=move |_| apply(true)>"Replace questions"</button>
                                    <button on:click=move |_| apply(false)>"Add to questions"</button>
                                </div>
                            </div>
                        }
                        .into_any()
                    }
                })
            }}
        </section>
    }
}
