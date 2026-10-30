mod api;
mod attempt;
mod editor;
mod kit;
mod icons;
mod ui;

use leptos::prelude::*;
use leptos::task::spawn_local;
use mocktest_domain::{
    AttemptSummary, AttemptStatus, BootstrapData, QuizSource, QuizSummary,
};
use ui::Ui;
use kit::ui::badge::{Badge, BadgeVariant};
use kit::ui::toast_custom::_context::ToasterContext;
use kit::ui::toast_custom::toaster::Toaster;
use kit::ui::dropdown_menu::{DropdownMenu, DropdownMenuAction, DropdownMenuActionVariant, DropdownMenuContent, DropdownMenuTrigger};
use kit::ui::button::{Button, ButtonVariant};
use kit::ui::card::Card;
use kit::ui::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow, TableWrapper};

#[derive(Clone, PartialEq)]
enum Page {
    Library,
    History,
    Data,
    Edit(String),
    Attempt(String),
    Review(String),
    NotFound,
}

fn current_page() -> Page {
    let hash = ui::hash();
    let parts: Vec<&str> = hash.split('/').filter(|part| !part.is_empty()).collect();
    match parts.as_slice() {
        [] => Page::Library,
        ["history"] => Page::History,
        ["data"] => Page::Data,
        ["edit", id] => Page::Edit(id.to_string()),
        ["attempt", id] => Page::Attempt(id.to_string()),
        ["review", id] => Page::Review(id.to_string()),
        _ => Page::NotFound,
    }
}

fn main() {
    console_error_panic_hook::set_once();
    kit::hooks::use_scroll_lock::init();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let toaster = ToasterContext::default();
    provide_context(toaster.clone());
    let ui = Ui::new(toaster);
    let page = RwSignal::new(current_page());
    let temporary = RwSignal::new(false);
    spawn_local(async move { temporary.set(api::storage_is_temporary().await) });

    let on_hash = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
        if ui::take_bypass() {
            page.set(current_page());
            return;
        }
        // A page with unsaved work can hold the navigation and ask first.
        if let Some(guard) = ui.leave_guard.get_untracked() {
            if guard(ui::hash()) {
                return;
            }
        }
        page.set(current_page());
    }) as Box<dyn FnMut()>);
    let _ = ui::win().add_event_listener_with_callback("hashchange", wasm_bindgen::JsCast::unchecked_ref(on_hash.as_ref()));
    on_hash.forget();

    // Keep the browser tab title in step with the page being shown.
    Effect::new(move |_| {
        let title = match page.get() {
            Page::Library => "Library",
            Page::History => "History",
            Page::Data => "Data",
            Page::Edit(_) => "Edit quiz",
            Page::Attempt(_) => "Quiz attempt",
            Page::Review(_) => "Review",
            Page::NotFound => "Page not found",
        };
        if let Some(document) = ui::win().document() {
            document.set_title(&format!("{title} · MockTest"));
        }
    });

    view! {
        <ui::ConfirmDialog ui=ui/>
        <Toaster/>
        <nav class="page-tabs" aria-label="Pages">
            <a href="#/" aria-current={move || (page.get() == Page::Library).then_some("page")}>"Library"</a>
            <a href="#/history" aria-current={move || (page.get() == Page::History).then_some("page")}>"History"</a>
            <a href="#/data" aria-current={move || (page.get() == Page::Data).then_some("page")}>"Data"</a>
        </nav>
        <main class="container">
            {move || {
                temporary.get().then(|| {
                    view! {
                        <section class="panel" role="status">
                            <h2>"Progress is not kept in this browser"</h2>
                            <p class="muted">
                                "This browser can't save data between visits. Quizzes, answers and history last only until you close this page. Use Chrome, Edge or another browser to keep them."
                            </p>
                        </section>
                    }
                })
            }}
            {move || match page.get() {
                Page::Library => view! { <Library ui=ui/> }.into_any(),
                Page::History => view! { <History ui=ui/> }.into_any(),
                Page::Data => view! { <Data ui=ui/> }.into_any(),
                Page::Edit(id) => view! { <editor::Editor ui=ui id=id/> }.into_any(),
                Page::Attempt(id) => view! { <attempt::Attempt ui=ui id=id/> }.into_any(),
                Page::Review(id) => view! { <attempt::Review ui=ui id=id/> }.into_any(),
                Page::NotFound => view! { <p class="muted">"Page not found."</p> }.into_any(),
            }}
        </main>
    }
}

// ---------- Library ----------

#[component]
fn Library(ui: Ui) -> impl IntoView {
    let data = RwSignal::new(None::<Result<BootstrapData, String>>);
    Effect::new(move |_| {
        ui.version.get();
        spawn_local(async move {
            data.set(Some(api::bootstrap().await));
        });
    });

    view! {
        <div class="page-head">
            <h1>"Your quizzes"</h1>
            <div class="actions">
                <Button variant=ButtonVariant::Default on:click=move |_| new_quiz(ui)>"New quiz"</Button>
                <Button variant=ButtonVariant::Outline on:click=move |_| import_quiz(ui)>"Import"</Button>
            </div>
        </div>
        {move || match data.get() {
            None => view! { <p class="muted">"Loading…"</p> }.into_any(),
            Some(Err(error)) => view! { <p class="error">{error}</p> }.into_any(),
            Some(Ok(bootstrap)) if bootstrap.quizzes.is_empty() => view! {
                <section class="panel empty">
                    <h2>"No quizzes yet"</h2>
                    <p class="muted">"Create a quiz, or import a quiz file you already have."</p>
                    <div class="actions">
                        <Button variant=ButtonVariant::Default on:click=move |_| new_quiz(ui)>"New quiz"</Button>
                        <Button variant=ButtonVariant::Outline on:click=move |_| import_quiz(ui)>"Import"</Button>
                    </div>
                </section>
            }
            .into_any(),
            Some(Ok(bootstrap)) => view! {
                <div class="grid">
                    {bootstrap.quizzes.into_iter().map(|q| quiz_card(ui, q)).collect_view()}
                </div>
            }
            .into_any(),
        }}
    }
}

fn quiz_card(ui: Ui, q: QuizSummary) -> impl IntoView {
    let is_sample = q.source == QuizSource::Sample;
    let details = [
        q.category.clone(),
        Some(format!("{} question(s)", q.question_count)),
        Some(match q.time_limit_secs {
            Some(secs) => format!("{} min", secs / 60),
            None => "No time limit".to_string(),
        }),
        q.pass_percent.map(|p| format!("Pass {p}%")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let attempts = format!(
        "{} attempt(s){}",
        q.attempts_count,
        match q.best_percent {
            Some(best) => format!(" · Best {}", ui::pct(Some(best))),
            None => String::new(),
        }
    );
    let resume_href = q.active_attempt_id.clone().map(|a| format!("#/attempt/{a}"));
    let edit_href = format!("#/edit/{}", q.id);
    let (start_id, dup_id, export_id, export_title, delete_id, delete_title) = (
        q.id.clone(),
        q.id.clone(),
        q.id.clone(),
        q.title.clone(),
        q.id.clone(),
        q.title.clone(),
    );
    let ready = q.ready;
    let hint = if ready { None } else { Some("Open Edit to finish this quiz before starting it.") };

    view! {
        <article class="card-article">
          <Card class="h-full">
            <div class="card-head">
                <h2 class="card-title">{q.title.clone()}</h2>
                <div class="badges">
                    {is_sample.then(|| view! { <Badge variant=BadgeVariant::Secondary>"Sample"</Badge> })}
                    {(!ready).then(|| view! { <Badge variant=BadgeVariant::Outline class="text-[var(--warn)] border-[var(--warn)]">"Incomplete"</Badge> })}
                </div>
            </div>
            {(!q.description.is_empty()).then(|| view! { <p class="muted">{q.description.clone()}</p> })}
            <p class="meta">{details}</p>
            <p class="meta">{attempts}</p>
            {hint.map(|text| view! { <p class="hint">{text}</p> })}
            <div class="actions">
                {match resume_href {
                    Some(href) => view! { <Button variant=ButtonVariant::Default href=href>"Resume"</Button> }.into_any(),
                    None => view! {
                        <Button variant=ButtonVariant::Default attr:disabled={!ready}
                            on:click=move |_| start_attempt(ui, start_id.clone())>"Start"</Button>
                    }
                    .into_any(),
                }}
                <DropdownMenu>
                    <DropdownMenuTrigger>"More"</DropdownMenuTrigger>
                    <DropdownMenuContent>
                        {(!is_sample).then(|| view! { <DropdownMenuAction href=edit_href.clone()>"Edit"</DropdownMenuAction> })}
                        <div class="contents" on:click=move |_| duplicate(ui, dup_id.clone())>
                            <DropdownMenuAction>{if is_sample { "Duplicate to edit" } else { "Duplicate" }}</DropdownMenuAction>
                        </div>
                        <div class="contents" on:click=move |_| export_quiz(ui, export_id.clone(), export_title.clone())>
                            <DropdownMenuAction>"Export"</DropdownMenuAction>
                        </div>
                        {(!is_sample).then(|| view! {
                            <div class="contents" on:click=move |_| delete_quiz(ui, delete_id.clone(), delete_title.clone())>
                                <DropdownMenuAction variant=DropdownMenuActionVariant::Destructive>"Delete"</DropdownMenuAction>
                            </div>
                        })}
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>
          </Card>
        </article>
    }
}

fn new_quiz(ui: Ui) {
    spawn_local(async move {
        match api::create_quiz().await {
            Ok(quiz) => {
                ui.drafts.update(|d| d.push(quiz.id.clone()));
                ui::go(&format!("#/edit/{}", quiz.id));
            }
            Err(error) => ui.notify(error, true),
        }
    });
}

fn import_quiz(ui: Ui) {
    spawn_local(async move {
        match ui::pick_text(".json,application/json").await {
            Ok(Some(text)) => match api::import_quiz(text).await {
                Ok(quiz) => {
                    ui.notify(format!("Imported “{}”.", quiz.title), false);
                    ui.reload();
                }
                Err(error) => ui.notify(error, true),
            },
            Ok(None) => {}
            Err(error) => ui.notify(ui::js_error(error), true),
        }
    });
}

fn start_attempt(ui: Ui, quiz_id: String) {
    spawn_local(async move {
        match api::start_attempt(&quiz_id, ui::new_seed()).await {
            Ok(attempt) => ui::go(&format!("#/attempt/{}", attempt.id)),
            Err(error) => ui.notify(error, true),
        }
    });
}

fn duplicate(ui: Ui, id: String) {
    spawn_local(async move {
        match api::duplicate_quiz(&id).await {
            Ok(copy) => {
                ui.notify("Copied. You can edit the copy now.", false);
                ui::go(&format!("#/edit/{}", copy.id));
            }
            Err(error) => ui.notify(error, true),
        }
    });
}

fn export_quiz(ui: Ui, id: String, title: String) {
    spawn_local(async move {
        match api::export_quiz(&id).await {
            Ok(json) => {
                let name = format!("{}.quiz.json", ui::slug(&title));
                if let Err(error) = ui::download(&name, &json, "application/json") {
                    ui.notify(ui::js_error(error), true);
                }
            }
            Err(error) => ui.notify(error, true),
        }
    });
}

fn delete_quiz(ui: Ui, id: String, title: String) {
    ui.ask(
        "Delete quiz",
        &format!("Delete “{title}”? Its past attempts stay in History."),
        "Delete",
        true,
        move || {
            spawn_local(async move {
                match api::delete_quiz(&id).await {
                    Ok(()) => {
                        ui.notify("Quiz deleted.", false);
                        ui.reload();
                    }
                    Err(error) => ui.notify(error, true),
                }
            });
        },
    );
}

// ---------- History ----------

#[component]
fn History(ui: Ui) -> impl IntoView {
    let data = RwSignal::new(None::<Result<BootstrapData, String>>);
    spawn_local(async move {
        data.set(Some(api::bootstrap().await));
    });

    view! {
        <div class="page-head"><h1>"History"</h1></div>
        {move || match data.get() {
            None => view! { <p class="muted">"Loading…"</p> }.into_any(),
            Some(Err(error)) => view! { <p class="error">{error}</p> }.into_any(),
            Some(Ok(bootstrap)) if bootstrap.attempts.is_empty() => {
                view! { <p class="muted">"No attempts yet."</p> }.into_any()
            }
            Some(Ok(bootstrap)) => {
                let mut rows: Vec<AttemptSummary> = bootstrap.attempts;
                rows.sort_by(|a, b| b.started_at.cmp(&a.started_at));
                view! {
                    <div class="actions">
                        <Button variant=ButtonVariant::Destructive on:click=move |_| clear_history(ui)>"Clear history"</Button>
                    </div>
                    <TableWrapper>
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>"Quiz"</TableHead><TableHead>"Started"</TableHead><TableHead>"Status"</TableHead><TableHead>"Score"</TableHead><TableHead>""</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>{rows.into_iter().map(history_row).collect_view()}</TableBody>
                        </Table>
                    </TableWrapper>
                }
                .into_any()
            }
        }}
    }
}

fn clear_history(ui: Ui) {
    ui.ask(
        "Clear history",
        "Delete all attempts and their answers? Quizzes are kept. This cannot be undone.",
        "Clear history",
        true,
        move || {
            spawn_local(async move {
                match api::clear_history().await {
                    Ok(()) => {
                        ui.notify("History was cleared.", false);
                        ui.reload();
                    }
                    Err(error) => ui.notify(error, true),
                }
            });
        },
    );
}

fn history_row(a: AttemptSummary) -> impl IntoView {
    let in_progress = a.status == AttemptStatus::InProgress;
    let score = if in_progress {
        "—".to_string()
    } else {
        let verdict = match a.passed {
            Some(true) => " · Passed",
            Some(false) => " · Not passed",
            None => "",
        };
        format!(
            "{}/{} ({}){}",
            a.correct_count.unwrap_or(0),
            a.total_count,
            ui::pct(a.score_percent),
            verdict
        )
    };
    let action = if in_progress {
        view! { <Button variant=ButtonVariant::Default href=format!("#/attempt/{}", a.id)>"Resume"</Button> }.into_any()
    } else {
        view! { <Button variant=ButtonVariant::Outline href=format!("#/review/{}", a.id)>"Review"</Button> }.into_any()
    };
    view! {
        <TableRow>
            <TableCell>{a.quiz_title.clone()}</TableCell>
            <TableCell>{ui::fmt_date(a.started_at)}</TableCell>
            <TableCell>{ui::status_label(a.status)}</TableCell>
            <TableCell>{score}</TableCell>
            <TableCell>{action}</TableCell>
        </TableRow>
    }
}

// ---------- Data ----------

#[component]
fn Data(ui: Ui) -> impl IntoView {
    view! {
        <div class="page-head"><h1>"Data"</h1></div>
        <section class="panel">
            <h2>"Backup"</h2>
            <p class="muted">"Save all quizzes and attempt history to a file you can restore later."</p>
            <Button variant=ButtonVariant::Default on:click=move |_| backup(ui)>"Download backup"</Button>
        </section>
        <section class="panel">
            <h2>"Restore"</h2>
            <p class="muted">"Replaces all current quizzes and history with the contents of a backup file."</p>
            <Button variant=ButtonVariant::Outline on:click=move |_| restore(ui)>"Restore from backup…"</Button>
        </section>
        <section class="panel danger-zone">
            <h2>"Reset"</h2>
            <p class="muted">"Deletes all quizzes and history."</p>
            <Button variant=ButtonVariant::Destructive on:click=move |_| reset(ui)>"Reset everything"</Button>
        </section>
    }
}

fn backup(ui: Ui) {
    spawn_local(async move {
        match api::backup().await {
            Ok(data) => {
                let payload = serde_json::json!({
                    "format": "mocktest.backup",
                    "version": 1,
                    "createdAt": ui::now_ms(),
                    "data": data,
                });
                let name = format!("mocktest-backup-{}.json", ui::today());
                match ui::download(&name, &payload.to_string(), "application/json") {
                    Ok(()) => ui.notify("Backup downloaded.", false),
                    Err(error) => ui.notify(ui::js_error(error), true),
                }
            }
            Err(error) => ui.notify(error, true),
        }
    });
}

fn restore(ui: Ui) {
    spawn_local(async move {
        let text = match ui::pick_text(".json,application/json").await {
            Ok(Some(text)) => text,
            Ok(None) => return,
            Err(error) => {
                ui.notify(ui::js_error(error), true);
                return;
            }
        };
        let payload: serde_json::Value = match serde_json::from_str(&text) {
            Ok(payload) => payload,
            Err(error) => {
                ui.notify(error.to_string(), true);
                return;
            }
        };
        let Some(data) = payload.get("data").and_then(|d| d.as_str()).map(str::to_string) else {
            ui.notify("This is not a MockTest backup file.", true);
            return;
        };
        if payload.get("format").and_then(|f| f.as_str()) != Some("mocktest.backup") {
            ui.notify("This is not a MockTest backup file.", true);
            return;
        }
        ui.ask(
            "Restore backup",
            "Restoring replaces all current quizzes and history. Continue?",
            "Restore",
            true,
            move || {
                spawn_local(async move {
                    match api::restore(data).await {
                        Ok(result) => {
                            let quizzes = result["quizzes"].as_u64().unwrap_or(0);
                            let attempts = result["attempts"].as_u64().unwrap_or(0);
                            ui.notify(format!("Restored {quizzes} quiz(zes) and {attempts} attempt(s)."), false);
                            ui.reload();
                        }
                        Err(error) => ui.notify(error, true),
                    }
                });
            },
        );
    });
}

fn reset(ui: Ui) {
    ui.ask(
        "Reset everything",
        "Delete all quizzes and history? This cannot be undone.",
        "Reset everything",
        true,
        move || {
            spawn_local(async move {
                match api::reset().await {
                    Ok(()) => {
                        ui.notify("Everything was reset.", false);
                        ui.reload();
                    }
                    Err(error) => ui.notify(error, true),
                }
            });
        },
    );
}
