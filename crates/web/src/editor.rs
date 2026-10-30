// Quiz editor. Edits a local copy (`draft`) and saves it through the worker,
// which runs the same validation rules as the domain crate.
use leptos::ev::Event;
use leptos::prelude::*;
use leptos::task::spawn_local;
use mocktest_domain::{
    AnswerFeedback, AnswerOption, MAX_OPTIONS, MAX_QUESTIONS, MIN_OPTIONS, Question, QuestionKind,
    Quiz, QuizSource, parse_quiz_text,
};

use crate::api;
use crate::icons;
use crate::kit::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::kit::ui::checkbox::Checkbox;
use crate::kit::ui::input::{Input, InputType};
use crate::kit::ui::select_native::SelectNative;
use crate::kit::ui::textarea::Textarea;
use crate::ui::{self, Ui};

fn feedback_value(feedback: AnswerFeedback) -> &'static str {
    match feedback {
        AnswerFeedback::AfterSubmit => "after_submit",
        AnswerFeedback::OnTheGo => "on_the_go",
    }
}

fn feedback_from_value(value: &str) -> AnswerFeedback {
    if value == "on_the_go" {
        AnswerFeedback::OnTheGo
    } else {
        AnswerFeedback::AfterSubmit
    }
}

/// A text signal whose changes are written into the draft by `apply`.
/// The kit's inputs bind to signals, so each field keeps one of these.
fn synced(
    draft: RwSignal<Quiz>,
    initial: String,
    apply: impl Fn(&mut Quiz, &str) + 'static,
) -> RwSignal<String> {
    let local = RwSignal::new(initial);
    Effect::new(move |first: Option<()>| {
        let value = local.get();
        // The first run only reads the initial value; later runs write it through.
        if first.is_some() {
            draft.update(|quiz| apply(quiz, &value));
        }
    });
    local
}

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
                    <Button variant=ButtonVariant::Default on:click=move |_| copy_for_editing(ui, id.clone())>
                        "Make an editable copy"
                    </Button>
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
            Ok(copy) => {
                ui.drafts.update(|d| d.push(copy.id.clone()));
                ui::go(&format!("#/edit/{}", copy.id));
            }
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
    let title = synced(draft, quiz.title.clone(), |q, v| q.title = v.to_string());
    let description = synced(draft, quiz.description.clone(), |q, v| q.description = v.to_string());
    let category = synced(draft, quiz.category.clone().unwrap_or_default(), |q, v| {
        q.category = Some(v.to_string())
    });
    let feedback = synced(draft, feedback_value(quiz.settings.feedback).to_string(), |q, v| {
        q.settings.feedback = feedback_from_value(v)
    });
    let tags_text = RwSignal::new(quiz.tags.join(", "));
    let minutes_text = RwSignal::new(
        quiz.settings
            .time_limit_secs
            .map(|s| (f64::from(s) / 60.0).to_string())
            .unwrap_or_default(),
    );
    let pass_text = RwSignal::new(quiz.settings.pass_percent.map(|p| p.to_string()).unwrap_or_default());
    let issues = RwSignal::new(Vec::<String>::new());
    let saving = RwSignal::new(false);
    // The quiz as the form would save it right now.
    let build = move || {
        let mut next = draft.get_untracked();
        next.tags = tags_text.get_untracked().split(',').map(|t| t.to_string()).collect();
        next.settings.time_limit_secs = parse_minutes(&minutes_text.get_untracked());
        next.settings.pass_percent = parse_percent(&pass_text.get_untracked());
        next.normalized()
    };
    // The last saved state, compared on exit to catch unsaved changes.
    let baseline = StoredValue::new(build());
    let is_dirty = move || build() != baseline.get_value();

    // Validates and saves the quiz. Optionally starts an attempt from it afterwards.
    let commit = move |start_after: bool| {
        if saving.get_untracked() {
            return;
        }
        let next = build();

        let found: Vec<String> = next.validation_issues().into_iter().map(|issue| issue.message).collect();
        let blocked = !found.is_empty();
        issues.set(found);
        if blocked {
            return;
        }
        saving.set(true);
        let id = next.id.clone();
        spawn_local(async move {
            match api::save_quiz(next.clone()).await {
                Ok(_) => {
                    baseline.set_value(next);
                    ui.drafts.update(|d| d.retain(|draft| draft != &id));
                    if start_after {
                        match api::start_attempt(&id, ui::new_seed()).await {
                            Ok(attempt) => ui::go(&format!("#/attempt/{}", attempt.id)),
                            Err(error) => ui.notify(error, true),
                        }
                    } else {
                        ui.notify("Quiz saved.", false);
                        ui::go("#/");
                    }
                }
                Err(error) => ui.notify(error, true),
            }
            saving.set(false);
        });
    };
    let save = move || commit(false);
    // Saves first, so Start works in one click even with unsaved changes.
    let start = move || commit(true);

    // Leaving a quiz that was made here and never saved deletes it, so exit
    // doesn't leave an empty quiz in the library.
    let exit = move || {
        let id = draft.with_untracked(|q| q.id.clone());
        let is_draft = ui.drafts.with_untracked(|d| d.contains(&id));
        let discard = move || {
            if !is_draft {
                ui::go("#/");
                return;
            }
            let id = id.clone();
            spawn_local(async move {
                match api::delete_quiz(&id).await {
                    Ok(()) => {
                        ui.drafts.update(|d| d.retain(|draft| draft != &id));
                        ui.reload();
                        ui::go("#/");
                    }
                    Err(error) => ui.notify(error, true),
                }
            });
        };
        if is_dirty() {
            ui.ask(
                "Leave without saving",
                "Leave without saving? Your changes will be lost.",
                "Leave",
                true,
                discard,
            );
        } else {
            discard();
        }
    };

    view! {
        <div class="page-head">
            <h1>"Edit quiz"</h1>
        </div>
        <section class="panel">
            <h2>"Details"</h2>
            <label class="field">
                <span>"Title"</span>
                <Input bind_value=title/>
            </label>
            <label class="field">
                <span>"Description"</span>
                <Textarea rows=3u32 bind_value=description/>
            </label>
            <label class="field">
                <span>"Category"</span>
                <Input bind_value=category/>
            </label>
            <label class="field">
                <span>"Tags (comma separated)"</span>
                <Input bind_value=tags_text/>
            </label>
        </section>

        <section class="panel">
            <h2>"Settings"</h2>
            <label class="field">
                <span>"Time limit in minutes (blank for none)"</span>
                <Input r#type=InputType::Number min="0" bind_value=minutes_text/>
            </label>
            <label class="field">
                <span>"Pass mark in percent (blank for none)"</span>
                <Input r#type=InputType::Number min="0" max="100" bind_value=pass_text/>
            </label>
            <label class="check">
                <Checkbox
                    aria_label="Shuffle questions"
                    checked=Signal::derive(move || draft.with(|q| q.settings.shuffle_questions))
                    on_checked_change=Callback::new(move |v: bool| draft.update(|q| q.settings.shuffle_questions = v))/>
                <span>"Shuffle questions"</span>
            </label>
            <label class="check">
                <Checkbox
                    aria_label="Shuffle answer options"
                    checked=Signal::derive(move || draft.with(|q| q.settings.shuffle_options))
                    on_checked_change=Callback::new(move |v: bool| draft.update(|q| q.settings.shuffle_options = v))/>
                <span>"Shuffle answer options"</span>
            </label>
            <label class="check">
                <Checkbox
                    aria_label="Show one question at a time"
                    checked=Signal::derive(move || draft.with(|q| q.settings.one_at_a_time))
                    on_checked_change=Callback::new(move |v: bool| draft.update(|q| q.settings.one_at_a_time = v))/>
                <span>"Show one question at a time"</span>
            </label>
            <label class="field">
                <span>"Answer feedback"</span>
                <SelectNative
                    value=feedback.read_only()
                    on_change=Callback::new(move |ev: Event| feedback.set(event_target_value(&ev)))>
                    <option value="after_submit">"Show answers after submitting"</option>
                    <option value="on_the_go">"Show each answer as I go"</option>
                </SelectNative>
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
            <Button
                variant=ButtonVariant::Outline
                attr:disabled=move || draft.with(|q| q.questions.len() >= MAX_QUESTIONS)
                on:click=move |_| draft.update(|q| q.questions.push(Question::blank()))>
                "Add question"
            </Button>
            {move || {
                (draft.with(|q| q.questions.len()) >= MAX_QUESTIONS).then(|| {
                    view! { <p class="hint">{format!("A quiz can have at most {MAX_QUESTIONS} questions.")}</p> }
                })
            }}
        </section>

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

        <div class="save-bar">
            <Button variant=ButtonVariant::Outline attr:disabled=move || saving.get() on:click=move |_| start()>"Start quiz"</Button>
            <Button variant=ButtonVariant::Outline on:click=move |_| exit()>"Exit without saving"</Button>
            <Button variant=ButtonVariant::Default attr:disabled=move || saving.get() on:click=move |_| save()>
                {move || if saving.get() { "Saving…" } else { "Save quiz" }}
            </Button>
        </div>
    }
}

fn question_editor(draft: RwSignal<Quiz>, q: Question) -> impl IntoView {
    let qid = StoredValue::new(q.id.clone());
    let index = move || draft.with(|d| d.questions.iter().position(|x| x.id == qid.get_value()).unwrap_or(0));
    let count = move || draft.with(|d| d.questions.len());

    let prompt = synced(draft, q.prompt.clone(), move |quiz, v| {
        let id = qid.get_value();
        if let Some(question) = quiz.questions.iter_mut().find(|x| x.id == id) {
            question.prompt = v.to_string();
        }
    });
    let explanation = synced(draft, q.explanation.clone(), move |quiz, v| {
        let id = qid.get_value();
        if let Some(question) = quiz.questions.iter_mut().find(|x| x.id == id) {
            question.explanation = v.to_string();
        }
    });
    let kind = RwSignal::new(kind_name(draft, &q.id).to_string());

    view! {
        <div class="question">
            <div class="question-head">
                <h3 class="question-title">{move || format!("Question {}", index() + 1)}</h3>
                <div class="actions">
                    <Button variant=ButtonVariant::Outline attr:disabled=move || index() == 0
                        attr:aria-label=move || format!("Move question {} up", index() + 1)
                        on:click=move |_| move_question(draft, qid.get_value(), -1)>"Up"</Button>
                    <Button variant=ButtonVariant::Outline attr:disabled=move || index() + 1 >= count()
                        attr:aria-label=move || format!("Move question {} down", index() + 1)
                        on:click=move |_| move_question(draft, qid.get_value(), 1)>"Down"</Button>
                    <Button variant=ButtonVariant::Destructive
                        attr:aria-label=move || format!("Remove question {}", index() + 1)
                        on:click=move |_| draft.update(|d| d.questions.retain(|x| x.id != qid.get_value()))>
                        "Remove"
                    </Button>
                </div>
            </div>
            <label class="field">
                <span>"Prompt"</span>
                <Textarea rows=2u32 bind_value=prompt/>
            </label>
            <label class="field">
                <span>"Type"</span>
                <SelectNative
                    value=kind.read_only()
                    on_change=Callback::new(move |ev: Event| {
                        let value = event_target_value(&ev);
                        set_kind(draft, &qid.get_value(), &value);
                        kind.set(value);
                    })>
                    <option value="single">"Single answer"</option>
                    <option value="multi">"Multiple answers"</option>
                </SelectNative>
            </label>
            <div class="options">
                <For
                    each=move || question_options(draft, &qid.get_value())
                    key=|o| o.id.clone()
                    children=move |o: AnswerOption| option_row(draft, qid, o)
                />
            </div>
            <div class="actions">
                <Button variant=ButtonVariant::Outline
                    attr:disabled=move || question_option_count(draft, &qid.get_value()) >= MAX_OPTIONS
                    on:click=move |_| add_option(draft, &qid.get_value())>
                    "Add option"
                </Button>
            </div>
            <label class="field">
                <span>"Explanation (shown after submitting)"</span>
                <Textarea rows=2u32 bind_value=explanation/>
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
    let text = synced(draft, o.text.clone(), move |quiz, v| {
        let (question_id, option_id) = (qid.get_value(), oid.get_value());
        if let Some(question) = quiz.questions.iter_mut().find(|x| x.id == question_id) {
            if let Some(option) = question.options.iter_mut().find(|x| x.id == option_id) {
                option.text = v.to_string();
            }
        }
    });
    view! {
        <div class="option-row">
            <label class="correct">
                <Checkbox
                    aria_label="Correct answer"
                    checked=Signal::derive(move || option_is_correct(draft, &qid.get_value(), &oid.get_value()))
                    on_checked_change=Callback::new(move |v: bool| mark_correct(draft, &qid.get_value(), &oid.get_value(), v))/>
                <span>"Correct"</span>
            </label>
            <Input
                placeholder="Option text"
                bind_value=text
                aria_label=Signal::derive(move || format!("Option {} text", option_number(draft, &qid.get_value(), &oid.get_value())))/>
            <Button variant=ButtonVariant::Ghost size=ButtonSize::IconSm
                attr:disabled=move || question_option_count(draft, &qid.get_value()) <= MIN_OPTIONS
                attr:title="Remove option"
                attr:aria-label=move || format!("Remove option {}", option_number(draft, &qid.get_value(), &oid.get_value()))
                on:click=move |_| remove_option(draft, &qid.get_value(), &oid.get_value())>
                {icons::cross()}
            </Button>
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
        let finish = move || {
            draft.update(|q| {
                if replace {
                    q.questions = questions;
                } else {
                    q.questions.extend(questions);
                }
            });
            preview.set(None);
            ui.info(format!("Added {count} question(s). Check them below, then Save."));
        };
        if replace && current > 0 {
            ui.ask(
                "Replace questions",
                &format!("Replace the {current} existing question(s) with the {count} pasted? This cannot be undone."),
                "Replace",
                true,
                finish,
            );
        } else {
            finish();
        }
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
            <label class="field">
                <span>"Questions to import"</span>
                <Textarea rows=8u32 placeholder="Paste your questions here" bind_value=text/>
            </label>
            <div class="actions">
                <Button variant=ButtonVariant::Outline on:click=move |_| preview.set(Some(parse_quiz_text(&text.get_untracked())))>
                    "Check text"
                </Button>
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
                                    <Button variant=ButtonVariant::Default on:click=move |_| apply(true)>"Replace questions"</Button>
                                    <Button variant=ButtonVariant::Outline on:click=move |_| apply(false)>"Add to questions"</Button>
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
