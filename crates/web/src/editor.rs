// Quiz editor. Edits a local copy (`draft`) and saves it through the worker,
// which runs the same validation rules as the domain crate.
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
use crate::kit::ui::carousel::{Carousel, CarouselContent, CarouselFirst, CarouselItem, CarouselLast, CarouselNext, CarouselPrevious};
use crate::kit::hooks::use_stepper::StepperContext;
use crate::kit::ui::select::Select;
use crate::kit::ui::stepper::{Stepper, StepperIndicator, StepperItem, StepperTitle, StepperTrigger};
use crate::kit::ui::switch::Switch;
use crate::kit::ui::textarea::Textarea;
use crate::ui::{self, LeaveGuard, Ui};
use std::rc::Rc;
use wasm_bindgen::{JsCast, closure::Closure};

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
        None => view! { <p class="text-muted-foreground">"Loading…"</p> }.into_any(),
        Some(Err(error)) => view! { <p class="text-bad">{error}</p> }.into_any(),
        Some(Ok(quiz)) if quiz.source == QuizSource::Sample => {
            let id = quiz.id.clone();
            view! {
                <section class="mb-7 border-t-2 border-ink bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]">
                    <h1 class="m-0 mb-1.5 font-display font-stretch-[125%] font-black uppercase text-[40px] leading-[1.02] tracking-[-0.01em] max-[600px]:text-[30px]">{quiz.title.clone()}</h1>
                    <p class="text-muted-foreground">"Sample quizzes are read-only. Make an editable copy to change it."</p>
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

/// What to do once a quiz has been saved.
enum AfterSave {
    Library,
    Start,
    Go(String),
}

#[component]
fn EditForm(ui: Ui, quiz: Quiz) -> impl IntoView {
    let draft = RwSignal::new(quiz.clone());
    // The question shown in the carousel. Kept inside the list as questions are added or removed.
    let question_index = RwSignal::new(0usize);
    Effect::new(move |_| {
        let count = draft.with(|d| d.questions.len());
        if question_index.get_untracked() >= count {
            question_index.set(count.saturating_sub(1));
        }
    });
    let question_count = Signal::derive(move || draft.with(|d| d.questions.len()));
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

    // Validates and saves the quiz, then does what `after` says.
    let commit = move |after: AfterSave| {
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
                    match after {
                        AfterSave::Start => match api::start_attempt(&id, ui::new_seed()).await {
                            Ok(attempt) => ui::go(&format!("#/attempt/{}", attempt.id)),
                            Err(error) => ui.notify(error, true),
                        },
                        AfterSave::Library => {
                            ui.notify("Quiz saved.", false);
                            ui::go("#/");
                        }
                        AfterSave::Go(destination) => {
                            ui.notify("Quiz saved.", false);
                            ui::go(&destination);
                        }
                    }
                }
                Err(error) => ui.notify(error, true),
            }
            saving.set(false);
        });
    };
    let save = move || commit(AfterSave::Library);
    // Saves first, so Start works in one click even with unsaved changes.
    let start = move || commit(AfterSave::Start);

    // Leaves the editor for `destination`. A quiz made here and never saved is deleted first.
    let leave = move |destination: String| {
        let id = draft.with_untracked(|q| q.id.clone());
        let is_draft = ui.drafts.with_untracked(|d| d.contains(&id));
        if !is_draft {
            ui::go_unguarded(&destination);
            return;
        }
        spawn_local(async move {
            match api::delete_quiz(&id).await {
                Ok(()) => {
                    ui.drafts.update(|d| d.retain(|draft| draft != &id));
                    ui.reload();
                    ui::go_unguarded(&destination);
                }
                Err(error) => ui.notify(error, true),
            }
        });
    };

    let exit = move || {
        if is_dirty() {
            ui.ask(
                "Leave without saving",
                "Leave without saving? Your changes will be lost.",
                "Leave",
                true,
                move || leave("#/".to_string()),
            );
        } else {
            leave("#/".to_string());
        }
    };

    // Leaving with the back button, a link or a typed address asks first:
    // save, leave without saving, or stay.
    let guard: LeaveGuard = Rc::new(move |target: String| {
        let id = draft.with_untracked(|q| q.id.clone());
        if target == format!("/edit/{id}") {
            return false;
        }
        let is_draft = ui.drafts.with_untracked(|d| d.contains(&id));
        if !is_dirty() && !is_draft {
            return false;
        }
        let destination = if target.is_empty() { "#/".to_string() } else { format!("#{target}") };
        let stay = format!("#/edit/{id}");
        let message = if is_dirty() {
            "You have unsaved changes. Save them before you leave?"
        } else {
            "This quiz hasn't been saved yet. Save it before you leave?"
        };
        let (save_to, leave_to) = (destination.clone(), destination);
        ui.ask_three(
            "Save this quiz?",
            message,
            "Save and leave",
            move || commit(AfterSave::Go(save_to)),
            "Leave without saving",
            move || leave(leave_to),
            move || ui::replace_hash(&stay),
        );
        true
    });
    ui.leave_guard.set(Some(guard));
    on_cleanup(move || ui.leave_guard.set(None));

    // Reloading or closing the tab asks the browser to confirm while work is unsaved.
    let unload = Closure::<dyn FnMut(web_sys::BeforeUnloadEvent)>::new(move |event: web_sys::BeforeUnloadEvent| {
        let id = draft.with_untracked(|q| q.id.clone());
        let is_draft = ui.drafts.with_untracked(|d| d.contains(&id));
        if is_dirty() || is_draft {
            event.prevent_default();
            event.set_return_value("");
        }
    });
    ui::win().set_onbeforeunload(Some(unload.as_ref().unchecked_ref()));
    let _unload = StoredValue::new_local(unload);
    on_cleanup(move || {
        ui::win().set_onbeforeunload(None);
    });

    view! {
        <div class="mb-6 flex flex-wrap items-end justify-between gap-x-6 gap-y-3 border-t-[6px] border-ink pt-4">
            <h1 class="m-0 mb-1.5 font-display font-stretch-[125%] font-black uppercase text-[40px] leading-[1.02] tracking-[-0.01em] max-[600px]:text-[30px]">"Edit quiz"</h1>
        </div>
        <Stepper total_steps=3>
            <div class="mb-6 flex w-full gap-2">
                <StepperItem step=0>
                    <StepperTrigger class="w-full">
                        <StepperIndicator/>
                        <StepperTitle>"Details"</StepperTitle>
                    </StepperTrigger>
                </StepperItem>
                <StepperItem step=1>
                    <StepperTrigger class="w-full">
                        <StepperIndicator/>
                        <StepperTitle>"Settings"</StepperTitle>
                    </StepperTrigger>
                </StepperItem>
                <StepperItem step=2>
                    <StepperTrigger class="w-full">
                        <StepperIndicator/>
                        <StepperTitle>"Questions"</StepperTitle>
                    </StepperTrigger>
                </StepperItem>
            </div>
            <FormStep step=0>
        <section class="mb-7 border-t-2 border-ink bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]">
            <h2 class="m-0 mb-3 font-display font-stretch-[112%] font-extrabold uppercase text-[15px] leading-[1.3] tracking-[0.06em]">"Details"</h2>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Title"</span>
                <Input bind_value=title/>
            </label>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Description"</span>
                <Textarea rows=3u32 bind_value=description/>
            </label>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Category"</span>
                <Input bind_value=category/>
            </label>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Tags (comma separated)"</span>
                <Input bind_value=tags_text/>
            </label>
        </section>
            </FormStep>
            <FormStep step=1>

        <section class="mb-7 border-t-2 border-ink bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]">
            <h2 class="m-0 mb-3 font-display font-stretch-[112%] font-extrabold uppercase text-[15px] leading-[1.3] tracking-[0.06em]">"Settings"</h2>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Time limit in minutes (blank for none)"</span>
                <Input r#type=InputType::Number min="0" bind_value=minutes_text/>
            </label>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Pass mark in percent (blank for none)"</span>
                <Input r#type=InputType::Number min="0" max="100" bind_value=pass_text/>
            </label>
            <label class="mb-2.5 flex items-center gap-2.5">
                <Switch
                    aria_label="Shuffle questions"
                    checked=Signal::derive(move || draft.with(|q| q.settings.shuffle_questions))
                    on_checked_change=Callback::new(move |v: bool| draft.update(|q| q.settings.shuffle_questions = v))/>
                <span>"Shuffle questions"</span>
            </label>
            <label class="mb-2.5 flex items-center gap-2.5">
                <Switch
                    aria_label="Shuffle answer options"
                    checked=Signal::derive(move || draft.with(|q| q.settings.shuffle_options))
                    on_checked_change=Callback::new(move |v: bool| draft.update(|q| q.settings.shuffle_options = v))/>
                <span>"Shuffle answer options"</span>
            </label>
            <label class="mb-2.5 flex items-center gap-2.5">
                <Switch
                    aria_label="Show one question at a time"
                    checked=Signal::derive(move || draft.with(|q| q.settings.one_at_a_time))
                    on_checked_change=Callback::new(move |v: bool| draft.update(|q| q.settings.one_at_a_time = v))/>
                <span>"Show one question at a time"</span>
            </label>
            <div class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Answer feedback"</span>
                <Select
                    aria_label="Answer feedback"
                    value=feedback
                    options=vec![("after_submit", "Show answers after submitting"), ("on_the_go", "Show each answer as I go")]/>
            </div>
        </section>
            </FormStep>
            <FormStep step=2>

            {text_import(ui, draft)}

        <section class="mb-7 border-t-2 border-ink bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]">
            <h2 class="m-0 mb-3 font-display font-stretch-[112%] font-extrabold uppercase text-[15px] leading-[1.3] tracking-[0.06em]">"Questions"</h2>
            <Carousel index=question_index class="flex flex-col gap-4">
                <CarouselContent>
                    <For
                        each=move || draft.get().questions
                        key=|q| q.id.clone()
                        children=move |q: Question| {
                            let id = q.id.clone();
                            let position = Signal::derive(move || {
                                draft.with(|d| d.questions.iter().position(|x| x.id == id).unwrap_or(0))
                            });
                            view! { <CarouselItem position=position>{question_editor(draft, q, question_index)}</CarouselItem> }
                        }
                    />
                </CarouselContent>
                <div class="flex flex-wrap items-center gap-2">
                    <CarouselFirst index=question_index/>
                    <CarouselPrevious index=question_index/>
                    <CarouselNext index=question_index count=question_count/>
                    <CarouselLast index=question_index count=question_count/>
                </div>
            </Carousel>
            <Button
                variant=ButtonVariant::Outline
                attr:disabled={move || draft.with(|q| q.questions.len() >= MAX_QUESTIONS)}
                on:click=move |_| {
                    draft.update(|q| q.questions.push(Question::blank()));
                    question_index.set(draft.with(|q| q.questions.len()).saturating_sub(1));
                }>
                "Add question"
            </Button>
            {move || {
                (draft.with(|q| q.questions.len()) >= MAX_QUESTIONS).then(|| {
                    view! { <p class="m-0 text-[13px] text-muted-foreground">{format!("A quiz can have at most {MAX_QUESTIONS} questions.")}</p> }
                })
            }}
        </section>
            </FormStep>
            <StepNav/>
        </Stepper>

        {move || {
            (!issues.get().is_empty()).then(|| {
                view! {
                    <div class="mb-7 border-t-2 border-bad bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]" role="alert">
                        <strong>"Fix these before saving:"</strong>
                        <ul>{issues.get().into_iter().map(|issue| view! { <li>{issue}</li> }).collect_view()}</ul>
                    </div>
                }
            })
        }}

        <div class="sticky bottom-0 z-[3] mt-2 flex flex-wrap justify-end gap-2 border-t-2 border-ink bg-background py-3.5">
            <Button variant=ButtonVariant::Outline attr:disabled=move || saving.get() on:click=move |_| start()>"Start quiz"</Button>
            <Button variant=ButtonVariant::Outline on:click=move |_| exit()>"Exit without saving"</Button>
            <Button variant=ButtonVariant::Default attr:disabled=move || saving.get() on:click=move |_| save()>
                {move || if saving.get() { "Saving…" } else { "Save quiz" }}
            </Button>
        </div>
    }
}

fn question_editor(draft: RwSignal<Quiz>, q: Question, question_index: RwSignal<usize>) -> impl IntoView {
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
        <div class="mb-0 border-t-[1.5px] border-ink py-[18px] [h2+&]:border-t-0 [h2+&]:pt-1">
            <div class="mb-3 flex flex-wrap items-center justify-between gap-2">
                <h3 class="m-0 font-display font-stretch-[112%] text-[15px] font-extrabold uppercase tracking-[0.06em]">{move || format!("Question {}", index() + 1)}</h3>
                <div class="flex flex-wrap items-center gap-2">
                    <Button variant=ButtonVariant::Outline attr:disabled=move || index() == 0
                        attr:aria-label=move || format!("Move question {} up", index() + 1)
                        on:click=move |_| {
                            move_question(draft, qid.get_value(), -1);
                            question_index.update(|i| *i = i.saturating_sub(1));
                        }>"Up"</Button>
                    <Button variant=ButtonVariant::Outline attr:disabled={move || index() + 1 >= count()}
                        attr:aria-label=move || format!("Move question {} down", index() + 1)
                        on:click=move |_| {
                            move_question(draft, qid.get_value(), 1);
                            question_index.update(|i| *i += 1);
                        }>"Down"</Button>
                    <Button variant=ButtonVariant::Destructive
                        attr:aria-label=move || format!("Remove question {}", index() + 1)
                        on:click=move |_| draft.update(|d| d.questions.retain(|x| x.id != qid.get_value()))>
                        "Remove"
                    </Button>
                </div>
            </div>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Prompt"</span>
                <Textarea rows=2u32 bind_value=prompt/>
            </label>
            <div class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Type"</span>
                <Select
                    aria_label="Question type"
                    value=kind
                    options=vec![("single", "Single answer"), ("multi", "Multiple answers")]
                    on_change=Callback::new(move |value: String| set_kind(draft, &qid.get_value(), &value))/>
            </div>
            <div >
                <For
                    each=move || question_options(draft, &qid.get_value())
                    key=|o| o.id.clone()
                    children=move |o: AnswerOption| option_row(draft, qid, o)
                />
            </div>
            <div class="flex flex-wrap items-center gap-2">
                <Button variant=ButtonVariant::Outline
                    attr:disabled={move || question_option_count(draft, &qid.get_value()) >= MAX_OPTIONS}
                    on:click=move |_| add_option(draft, &qid.get_value())>
                    "Add option"
                </Button>
            </div>
            <label class="mb-3.5 flex flex-col gap-1.5">
                <span class="font-display font-stretch-[112%] text-xs font-bold uppercase tracking-[0.06em] text-muted-foreground">"Explanation (shown after submitting)"</span>
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
        <div class="mb-2 grid grid-cols-[auto_1fr_auto] items-center gap-2 max-[600px]:grid-cols-[auto_1fr]">
            <label class="flex items-center gap-1.5 whitespace-nowrap text-[13px] text-muted-foreground">
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
                attr:disabled={move || question_option_count(draft, &qid.get_value()) <= MIN_OPTIONS}
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
    let show_text = RwSignal::new(false);
    let open_text = move |_: leptos::ev::MouseEvent| {
        crate::kit::hooks::use_scroll_lock::lock();
        show_text.set(true);
    };
    let close_text = move |_: leptos::ev::MouseEvent| {
        crate::kit::hooks::use_scroll_lock::unlock(0);
        show_text.set(false);
    };
    let close_now = move || {
        crate::kit::hooks::use_scroll_lock::unlock(0);
        show_text.set(false);
    };

    // Focus the paste area when the dialog opens.
    Effect::new(move |_| {
        if show_text.get() {
            let element = ui::win()
                .document()
                .and_then(|d| d.get_element_by_id("text-dialog-textarea"))
                .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok());
            if let Some(element) = element {
                let _ = element.focus();
            }
        }
    });

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
        <section class="mb-7 border-t-2 border-ink bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]">
            <h2 class="m-0 mb-3 font-display font-stretch-[112%] font-extrabold uppercase text-[15px] leading-[1.3] tracking-[0.06em]">"Import questions from text"</h2>
            <p class="text-muted-foreground">
                "Paste questions and answers. Put * after each correct option. The quiz title and settings are set in the fields above."
            </p>
            <details>
                <summary>"Show the format"</summary>
                <pre class="whitespace-pre overflow-x-auto border-[1.5px] border-ink bg-surface p-3 font-mono text-[13px] text-field-ink">{FORMAT_HELP}</pre>
            </details>
            <div class="flex flex-wrap items-center gap-2">
                <Button variant=ButtonVariant::Outline on:click=move |ev: leptos::ev::MouseEvent| open_text(ev)>
                    "Add questions from text"
                </Button>
            </div>
        </section>

        <Show when=move || show_text.get()>
            <div class="fixed inset-0 z-[60] bg-black/50" aria-hidden="true" on:click=move |ev: leptos::ev::MouseEvent| close_text(ev)></div>
            <div
                role="dialog"
                aria-modal="true"
                aria-labelledby="text-dialog-title"
                aria-describedby="text-dialog-description"
                class="fixed top-1/2 left-1/2 z-[70] flex w-[calc(100%-2rem)] max-w-2xl -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-md border bg-card p-6 text-card-foreground shadow-lg"
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "Escape" {
                        close_now();
                    }
                }
            >
                <h2 id="text-dialog-title" class="text-lg leading-none font-semibold">"Add questions from text"</h2>
                <p id="text-dialog-description" class="text-sm text-muted-foreground">
                    "Paste questions and answers. Put * after each correct option. The quiz title and settings are set in the fields above."
                </p>
                <Textarea
                    id="text-dialog-textarea"
                    rows=12u32
                    class="bg-[#ffffff] min-h-72"
                    placeholder="Paste your questions here"
                    bind_value=text
                />
                {move || {
                    preview.get().map(|result| match result {
                    Err(errors) => view! {
                        <div class="mb-7 border-t-2 border-bad bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]" role="alert">
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
                            <div class="mb-7 border-t-2 border-ink bg-surface px-5 pt-[22px] pb-1.5 [overflow-wrap:anywhere]">
                                <p class="mb-2.5">{format!("Found {count} question(s):")}</p>
                                <ol>{names.into_iter().map(|name| view! { <li>{name}</li> }).collect_view()}</ol>
                                <div class="flex flex-wrap items-center gap-2">
                                    <Button variant=ButtonVariant::Default on:click=move |_| apply(true)>"Replace questions"</Button>
                                    <Button variant=ButtonVariant::Outline on:click=move |_| apply(false)>"Add to questions"</Button>
                                </div>
                            </div>
                        }
                        .into_any()
                    }
                })
            }}
            <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
                <Button variant=ButtonVariant::Outline on:click=move |ev: leptos::ev::MouseEvent| close_text(ev)>"Close"</Button>
                <Button variant=ButtonVariant::Default on:click=move |_| preview.set(Some(parse_quiz_text(&text.get_untracked())))>"Check text"</Button>
            </div>
            </div>
        </Show>
    }
}

/// One step of the edit form. Steps that are not current are hidden, not removed, so typing is kept.
#[component]
fn FormStep(step: usize, children: Children) -> impl IntoView {
    let ctx = expect_context::<StepperContext>();
    view! {
        <div class=move || if ctx.current_index.get() == step { "w-full basis-full" } else { "hidden" }>
            {children()}
        </div>
    }
}

/// Back and Next under the steps.
#[component]
fn StepNav() -> impl IntoView {
    let ctx = expect_context::<StepperContext>();
    let names = ["Details", "Settings", "Questions"];
    view! {
        <div class="mb-7 flex w-full flex-wrap items-center justify-between gap-2">
            <Button variant=ButtonVariant::Outline attr:aria-disabled=move || (!ctx.can_go_prev.get()).then_some("true")
                on:click=move |_| ctx.go_prev.run(())>"Back"</Button>
            <span class="text-[13px] text-muted-foreground">
                {move || format!("Step {} of {}: {}", ctx.current_index.get() + 1, names.len(), names[ctx.current_index.get().min(names.len() - 1)])}
            </span>
            <Button variant=ButtonVariant::Outline attr:aria-disabled=move || (!ctx.can_go_next.get()).then_some("true")
                on:click=move |_| ctx.go_next.run(())>"Next"</Button>
        </div>
    }
}
