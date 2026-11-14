use std::path::PathBuf;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _};
use gpui_kit::*;
use sgg_core::{ChangeKind, FileChange, RepoSnapshot, RepoState};

pub fn run(start: PathBuf) {
    application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            init(cx);

            let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("simple-git-gui".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            open_window(options, cx, move |window, cx| {
                cx.new(|cx| AppView::new(start.clone(), window, cx))
            })
            .expect("open window");
        });
}

struct AppView {
    start_path: PathBuf,
    snapshot: Option<RepoSnapshot>,
    selected: Vec<String>,
    status_line: SharedString,
    commit_input: Entity<TextareaState>,
    branch_input: Entity<InputState>,
    error: Option<SharedString>,
}

impl AppView {
    fn new(start_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let commit_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("Commit message")
        });
        let branch_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("New branch name")
        });

        let mut view = Self {
            start_path,
            snapshot: None,
            selected: Vec::new(),
            status_line: "Loading…".into(),
            commit_input,
            branch_input,
            error: None,
        };
        view.reload(cx);
        view
    }

    fn with_repo<F, T>(&self, f: F) -> Result<T, SharedString>
    where
        F: FnOnce(&RepoState) -> Result<T, sgg_core::RepoError>,
    {
        let start = self
            .snapshot
            .as_ref()
            .map(|s| s.root.clone())
            .unwrap_or_else(|| self.start_path.clone());
        let repo = RepoState::open_nearest(&start).map_err(|e| SharedString::from(e.to_string()))?;
        f(&repo).map_err(|e| SharedString::from(e.to_string()))
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        match RepoState::open_nearest(&self.start_path) {
            Ok(repo) => match repo.snapshot() {
                Ok(snap) => {
                    let branch = snap
                        .head_branch
                        .clone()
                        .unwrap_or_else(|| "(detached)".into());
                    self.status_line = format!(
                        "{}  ·  {}  ·  {} changes",
                        snap.root.display(),
                        branch,
                        snap.changes.len()
                    )
                    .into();
                    self.snapshot = Some(snap);
                    self.error = None;
                    self.selected.clear();
                }
                Err(err) => {
                    self.error = Some(err.to_string().into());
                    self.status_line = "Failed to read repository".into();
                }
            },
            Err(err) => {
                self.snapshot = None;
                self.error = Some(err.to_string().into());
                self.status_line = format!("No git repo near {}", self.start_path.display()).into();
            }
        }
        cx.notify();
    }

    fn toggle_select(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(idx) = self.selected.iter().position(|p| p == &path) {
            self.selected.remove(idx);
        } else {
            self.selected.push(path);
        }
        cx.notify();
    }

    fn stage_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected.clone();
        if paths.is_empty() {
            self.error = Some("Select one or more files first".into());
            cx.notify();
            return;
        }
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        match self.with_repo(|repo| repo.stage_paths(&refs)) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn unstage_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected.clone();
        if paths.is_empty() {
            self.error = Some("Select one or more files first".into());
            cx.notify();
            return;
        }
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        match self.with_repo(|repo| repo.unstage_paths(&refs)) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn stage_all(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.stage_all()) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.commit_input.read(cx).value().to_string();
        match self.with_repo(|repo| repo.commit(&message)) {
            Ok(oid) => {
                self.commit_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
                self.status_line = format!("Committed {}", &oid[..8.min(oid.len())]).into();
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn push(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.push(None)) {
            Ok(out) => {
                self.status_line = if out.is_empty() {
                    "Push completed".into()
                } else {
                    out.into()
                };
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn pull(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.pull(None)) {
            Ok(out) => {
                self.status_line = if out.is_empty() {
                    "Pull completed".into()
                } else {
                    out.into()
                };
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.fetch(None)) {
            Ok(out) => {
                self.status_line = if out.is_empty() {
                    "Fetch completed".into()
                } else {
                    out.into()
                };
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn create_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.branch_input.read(cx).value().to_string();
        match self.with_repo(|repo| {
            repo.create_branch(&name)?;
            repo.switch_branch(name.trim())?;
            Ok(())
        }) {
            Ok(()) => {
                self.branch_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn switch_branch(&mut self, name: String, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.switch_branch(&name)) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => {
                self.error = Some(err);
                cx.notify();
            }
        }
    }

    fn render_change_row(
        &self,
        change: &FileChange,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = change.path.clone();
        let selected = self.selected.iter().any(|p| p == &change.path);
        let kind = match change.kind {
            ChangeKind::Staged => "S",
            ChangeKind::Unstaged => "U",
            ChangeKind::Untracked => "?",
            ChangeKind::Conflicted => "!",
        };
        let label = format!(
            "[{kind}] {status}  {path}",
            status = change.status_label,
            path = change.path
        );

        Button::new(("change", change.path.clone()))
            .ghost()
            .label(label)
            .selected(selected)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_select(path.clone(), cx);
            }))
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changes = self
            .snapshot
            .as_ref()
            .map(|s| s.changes.clone())
            .unwrap_or_default();
        let branches = self
            .snapshot
            .as_ref()
            .map(|s| s.branches.clone())
            .unwrap_or_default();
        let remotes = self
            .snapshot
            .as_ref()
            .map(|s| s.remotes.clone())
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child("sgg"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.status_line.clone()),
                    )
                    .child(
                        Button::new("refresh")
                            .outline()
                            .small()
                            .label("Refresh")
                            .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                    )
                    .child(
                        Button::new("fetch")
                            .outline()
                            .small()
                            .label("Fetch")
                            .on_click(cx.listener(|this, _, _, cx| this.fetch(cx))),
                    )
                    .child(
                        Button::new("pull")
                            .outline()
                            .small()
                            .label("Pull")
                            .on_click(cx.listener(|this, _, _, cx| this.pull(cx))),
                    )
                    .child(
                        Button::new("push")
                            .primary()
                            .small()
                            .label("Push")
                            .on_click(cx.listener(|this, _, _, cx| this.push(cx))),
                    ),
            )
                    .when_some(self.error.clone(), |this, err| {
                this.child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(cx.theme().danger)
                        .text_color(cx.theme().background)
                        .text_sm()
                        .child(err),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        // Changes column
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .py_2()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(div().font_semibold().child("Changes"))
                                    .child(
                                        Button::new("stage")
                                            .small()
                                            .outline()
                                            .label("Stage")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.stage_selected(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("unstage")
                                            .small()
                                            .outline()
                                            .label("Unstage")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.unstage_selected(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("stage-all")
                                            .small()
                                            .outline()
                                            .label("Stage all")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.stage_all(cx)
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .id("changes-list")
                                    .flex_1()
                                    .min_h_0()
                                    .p_2()
                                    .gap_1()
                                    .flex()
                                    .flex_col()
                                    .overflow_y_scroll()
                                    .children(changes.into_iter().map(|c| {
                                        self.render_change_row(&c, cx).into_any_element()
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .p_3()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .child(Textarea::new(&self.commit_input))
                                    .child(
                                        Button::new("commit")
                                            .primary()
                                            .label("Commit")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.commit(window, cx)
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        // Branches / remotes column
                        div()
                            .flex()
                            .flex_col()
                            .w(px(320.))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .font_semibold()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child("Branches"),
                            )
                            .child(
                                div()
                                    .id("branch-list")
                                    .flex_1()
                                    .min_h_0()
                                    .p_2()
                                    .gap_1()
                                    .flex()
                                    .flex_col()
                                    .overflow_y_scroll()
                                    .children(branches.into_iter().map(|b| {
                                        let name = b.name.clone();
                                        let label = if b.is_head {
                                            format!("* {}", b.name)
                                        } else if b.is_remote {
                                            format!("  remotes/{}", b.name)
                                        } else {
                                            format!("  {}", b.name)
                                        };
                                        let disabled = b.is_remote || b.is_head;
                                        Button::new(("branch", b.name.clone()))
                                            .ghost()
                                            .label(label)
                                            .disabled(disabled)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.switch_branch(name.clone(), cx);
                                            }))
                                            .into_any_element()
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .p_3()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .child(Input::new(&self.branch_input))
                                    .child(
                                        Button::new("create-branch")
                                            .outline()
                                            .label("Create & switch")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.create_branch(window, cx)
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .font_semibold()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .child("Remotes"),
                            )
                            .child(
                                div()
                                    .p_3()
                                    .gap_1()
                                    .flex()
                                    .flex_col()
                                    .children(remotes.into_iter().map(|r| {
                                        let text = match r.url {
                                            Some(url) => format!("{} — {}", r.name, url),
                                            None => r.name,
                                        };
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(text)
                                            .into_any_element()
                                    })),
                            ),
                    ),
            )
    }
}
