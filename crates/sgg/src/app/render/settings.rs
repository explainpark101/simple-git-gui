use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::color_picker::ColorSelect;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::busy::BusyKind;
use crate::app::editor::{EditorPreset, ZED_DOWNLOAD_URL};
use crate::app::i18n::{Locale, UiText};
use crate::app::prefs::{
    settings_path, StatusColors, TerminalShellKind, ThemePreference,
    COMFORTABLE_TERMINAL_LINE_HEIGHT, DEFAULT_TERMINAL_FONT_FAMILY, DEFAULT_TERMINAL_LINE_HEIGHT,
};
use crate::app::render::status_style::display_label;
use crate::app::view::AppView;
use crate::app::widgets::{action_btn, display_btn, icon_btn, list_row_btn, menu_item, with_busy};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let path = view.repo_root_display();
    let editor = &view.preferred_editor;
    let theme = view.theme_pref;
    let locale = view.locale;
    let settings_file = settings_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unavailable)".into());

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .items_center()
        .p_6()
        .overflow_y_scrollbar()
        .child(
            div()
                .w_full()
                .max_w(px(720.))
                .flex()
                .flex_col()
                .gap_6()
                .child(div().text_lg().font_semibold().text_left().child(t.settings))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(div().font_semibold().text_left().child(t.appearance))
                        .child(
                            div()
                                .text_xs()
                                .text_left()
                                .text_color(cx.theme().muted_foreground)
                                .child(t.appearance_help),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_start()
                                .gap_2()
                                .child(
                                    display_btn("theme-light")
                                        .outline()
                                        .icon(AssetIcon::Sun)
                                        .label(t.theme_light)
                                        .tooltip(t.tip_theme_light)
                                        .selected(theme == ThemePreference::Light)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_theme_preference(
                                                ThemePreference::Light,
                                                window,
                                                cx,
                                            )
                                        })),
                                )
                                .child(
                                    display_btn("theme-dark")
                                        .outline()
                                        .icon(AssetIcon::Moon)
                                        .label(t.theme_dark)
                                        .tooltip(t.tip_theme_dark)
                                        .selected(theme == ThemePreference::Dark)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_theme_preference(
                                                ThemePreference::Dark,
                                                window,
                                                cx,
                                            )
                                        })),
                                )
                                .child(
                                    display_btn("theme-system")
                                        .outline()
                                        .icon(AssetIcon::Monitor)
                                        .label(t.theme_system)
                                        .tooltip(t.tip_theme_system)
                                        .selected(theme == ThemePreference::System)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.set_theme_preference(
                                                ThemePreference::System,
                                                window,
                                                cx,
                                            )
                                        })),
                                ),
                        ),
                )
                .child(status_colors_section(view, t, cx))
                .child(ui_font_section(view, t, cx))
                .child(inline_blame_section(view, t, cx))
                .child(diff_line_background_section(view, t, cx))
                .child(terminal_shell_section(view, t, cx))
                .child(terminal_font_section(view, t, cx))
                .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .child(div().font_semibold().text_left().child(t.language))
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.language_help),
                )
                .child(
                    div()
                        .flex()
                        .justify_start()
                        .gap_2()
                        .children(Locale::ALL.into_iter().map(|item| {
                            let tip = match item {
                                Locale::English => t.tip_locale_en,
                                Locale::Korean => t.tip_locale_ko,
                            };
                            display_btn(SharedString::from(format!("locale-{}", item.label())))
                                .outline()
                                .label(item.label())
                                .tooltip(tip)
                                .selected(locale == item)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.set_locale(item, window, cx)
                                }))
                                .into_any_element()
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .child(div().font_semibold().text_left().child(t.changes_view))
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.changes_view_help),
                )
                .child(
                    div()
                        .flex()
                        .justify_start()
                        .gap_2()
                        .child(
                            display_btn("changes-flat")
                                .outline()
                                .label(t.changes_flat)
                                .tooltip(t.tip_changes_flat)
                                .selected(!view.changes_as_tree)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_changes_as_tree(false, cx)
                                })),
                        )
                        .child(
                            display_btn("changes-tree")
                                .outline()
                                .label(t.changes_tree)
                                .tooltip(t.tip_changes_tree)
                                .selected(view.changes_as_tree)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_changes_as_tree(true, cx)
                                })),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .child(div().font_semibold().text_left().child(t.auto_refresh))
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.auto_refresh_help),
                )
                .child(
                    div()
                        .flex()
                        .justify_start()
                        .gap_2()
                        .child(
                            display_btn("auto-refresh-on")
                                .outline()
                                .label(t.auto_refresh_on)
                                .tooltip(t.tip_auto_refresh_on)
                                .selected(view.auto_refresh)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_auto_refresh(true, cx)
                                })),
                        )
                        .child(
                            display_btn("auto-refresh-off")
                                .outline()
                                .label(t.auto_refresh_off)
                                .tooltip(t.tip_auto_refresh_off)
                                .selected(!view.auto_refresh)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_auto_refresh(false, cx)
                                })),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .child(div().font_semibold().text_left().child(t.branches))
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.tip_show_remote_branches),
                )
                .child(
                    Switch::new("settings-show-remote-branches")
                        .label(t.show_remote_branches)
                        .checked(view.show_remote_branches)
                        .tooltip(t.tip_show_remote_branches)
                        .on_click(cx.listener(|this, checked, _, cx| {
                            this.set_show_remote_branches(*checked, cx);
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(div().font_semibold().text_left().child(t.external_editor))
                        .child(
                            display_btn("zed-install")
                                .ghost()
                                .small()
                                .icon(AssetIcon::Download)
                                .label(t.zed_install)
                                .tooltip(t.tip_zed_install)
                                .on_click(|_, _, cx| {
                                    cx.open_url(ZED_DOWNLOAD_URL);
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.external_editor_help),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_start()
                        .gap_2()
                        .child(editor_select(view, t.tip_editor_select, cx))
                        .child(
                            with_busy(
                                crate::app::widgets::confirm_btn(
                                    "open-project-editor",
                                    AssetIcon::ExternalLink,
                                    cx,
                                )
                                .small()
                                .label(format!("{} {}", t.open_project_in, editor.label()))
                                .tooltip(t.tip_open_project)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_project_in_editor_busy(window, cx)
                                })),
                                view.busy,
                                BusyKind::OpenProject,
                            ),
                        ),
                )
                .when(editor.preset == EditorPreset::Custom, |this| {
                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(Input::new(&view.custom_editor_input)),
                            )
                            .child(
                                with_busy(
                                    action_btn("save-custom-editor", AssetIcon::Save)
                                        .outline()
                                        .small()
                                        .label(t.save_command)
                                        .tooltip(t.tip_save_command)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.save_custom_editor_command(window, cx)
                                        })),
                                    view.busy,
                                    BusyKind::SaveEditor,
                                ),
                            ),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .child(div().font_semibold().text_left().child(t.fetch_on_open))
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.fetch_on_open_help),
                )
                .child(
                    div()
                        .flex()
                        .justify_start()
                        .gap_2()
                        .child(
                            display_btn("ask-fetch-on")
                                .outline()
                                .label(t.ask_fetch_on)
                                .tooltip(t.tip_ask_fetch_on)
                                .selected(view.ask_fetch_on_open)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_ask_fetch_on_open(true, cx)
                                })),
                        )
                        .child(
                            display_btn("ask-fetch-off")
                                .outline()
                                .label(t.ask_fetch_off)
                                .tooltip(t.tip_ask_fetch_off)
                                .selected(!view.ask_fetch_on_open)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_ask_fetch_on_open(false, cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_left()
                                .font_semibold()
                                .child(t.fetch_prompt_muted_list),
                        )
                        .child(
                            action_btn("mute-current-fetch", AssetIcon::Ban)
                                .outline()
                                .small()
                                .label(t.mute_current_repo)
                                .tooltip(t.tip_mute_current_repo)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.mute_current_fetch_prompt(cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.fetch_prompt_muted_help),
                )
                .when(view.fetch_prompt_muted.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_left()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.fetch_prompt_muted_empty),
                    )
                })
                .children(view.fetch_prompt_muted.iter().cloned().map(|root| {
                    let root_unmute = root.clone();
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .text_left()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(root),
                        )
                        .child(
                            action_btn(
                                SharedString::from(format!("unmute-fetch:{root_unmute}")),
                                AssetIcon::RotateCcw,
                            )
                            .outline()
                            .small()
                            .label(t.unmute_fetch_prompt)
                            .tooltip(t.tip_unmute_fetch_prompt)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.unmute_fetch_prompt(root_unmute.clone(), cx);
                            })),
                        )
                        .into_any_element()
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .min_w_0()
                                .flex_1()
                                .child(div().font_semibold().text_left().child(t.repository))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_left()
                                        .text_color(cx.theme().muted_foreground)
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(format!("{}: {path}", t.current_path)),
                                ),
                        )
                        .child(
                            with_busy(
                                action_btn("settings-open-folder", AssetIcon::FolderOpen)
                                    .primary()
                                    .small()
                                    .label(t.open_folder)
                                    .tooltip(t.tip_open_folder)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_folder(window, cx)
                                    })),
                                view.busy,
                                BusyKind::OpenFolder,
                            ),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_left()
                        .child(t.recent_projects),
                )
                .child(
                    div()
                        .max_h(px(280.))
                        .min_h(px(72.))
                        .w_full()
                        .min_w_0()
                        .overflow_y_scrollbar()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when(view.recent_repos.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_left()
                                    .child(t.recent_projects_empty),
                            )
                        })
                        .children(view.recent_repos.iter().enumerate().map(|(i, repo)| {
                            let open_path = repo.clone();
                            let tip = t.tip_open_recent;
                            list_row_btn(
                                SharedString::from(format!("settings-recent:{i}")),
                                repo.clone(),
                            )
                            .tooltip(tip)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_repo_path(
                                    std::path::PathBuf::from(open_path.clone()),
                                    cx,
                                );
                            }))
                            .into_any_element()
                        })),
                ),
        )
                .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{}: {settings_file}", t.settings_file)),
        ),
        )
}

fn inline_blame_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(div().font_semibold().text_left().child(t.inline_blame))
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.tip_inline_blame),
        )
        .child(
            Switch::new("settings-inline-blame")
                .label(t.inline_blame)
                .checked(view.show_inline_blame)
                .tooltip(t.tip_inline_blame)
                .on_click(cx.listener(|this, checked, _, cx| {
                    this.set_show_inline_blame(*checked, cx);
                })),
        )
}

fn diff_line_background_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(div().font_semibold().text_left().child(t.diff_line_background))
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.tip_diff_line_background),
        )
        .child(
            Switch::new("settings-diff-line-bg")
                .label(t.diff_line_background)
                .checked(view.show_diff_line_background)
                .tooltip(t.tip_diff_line_background)
                .on_click(cx.listener(|this, checked, _, cx| {
                    this.set_show_diff_line_background(*checked, cx);
                })),
        )
}

fn ui_font_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let presets = [
        ("Default", ""),
        ("Segoe UI", "Segoe UI"),
        ("Malgun Gothic", "Malgun Gothic"),
        ("Noto Sans", "Noto Sans"),
        ("Arial", "Arial"),
    ];

    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().font_semibold().text_left().child(t.ui_font))
                .child(
                    display_btn("ui-font-reset")
                        .ghost()
                        .small()
                        .icon(AssetIcon::RotateCcw)
                        .label(t.reset_status_colors)
                        .tooltip(t.tip_reset_ui_font)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.reset_ui_font(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.ui_font_help),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(presets.into_iter().map(|(label, family)| {
                    let family = family.to_string();
                    let selected = view.ui_font_family == family
                        || (label == "Default" && view.ui_font_family.is_empty());
                    display_btn(SharedString::from(format!("ui-font-preset:{label}")))
                        .outline()
                        .small()
                        .label(label.to_string())
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_ui_font_family(family.clone(), window, cx);
                        }))
                        .into_any_element()
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(&view.ui_font_input)),
                )
                .child(
                    action_btn("apply-ui-font", AssetIcon::Save)
                        .outline()
                        .small()
                        .label(t.save)
                        .tooltip(t.tip_apply_ui_font)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.apply_ui_font_from_input(window, cx);
                        })),
                ),
        )
}

fn terminal_shell_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let kind = view.terminal_shell.kind;
    let tips = [
        (
            TerminalShellKind::PowerShell,
            t.tip_terminal_shell_powershell,
        ),
        (TerminalShellKind::Cmd, t.tip_terminal_shell_cmd),
        (TerminalShellKind::GitBash, t.tip_terminal_shell_git_bash),
        (TerminalShellKind::Custom, t.tip_terminal_shell_custom),
    ];

    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(div().font_semibold().text_left().child(t.terminal_shell))
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.terminal_shell_help),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(tips.into_iter().map(|(shell_kind, tip)| {
                    display_btn(SharedString::from(format!(
                        "term-shell:{}",
                        shell_kind.label()
                    )))
                    .outline()
                    .small()
                    .label(shell_kind.label().to_string())
                    .tooltip(tip)
                    .selected(kind == shell_kind)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_terminal_shell_kind(shell_kind, window, cx);
                    }))
                    .into_any_element()
                })),
        )
        .when(kind == TerminalShellKind::Custom, |this| {
            this.child(
                div()
                    .text_sm()
                    .text_left()
                    .child(t.terminal_shell_custom),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&view.terminal_shell_input)),
                    )
                    .child(
                        action_btn("apply-terminal-shell", AssetIcon::Save)
                            .outline()
                            .small()
                            .label(t.save)
                            .tooltip(t.tip_apply_terminal_shell)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.apply_terminal_shell_custom(window, cx);
                            })),
                    ),
            )
        })
}

fn terminal_font_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let size_label = format!("{:.0}px", view.terminal_font_size);
    let standard = (view.terminal_line_height - DEFAULT_TERMINAL_LINE_HEIGHT).abs() < 0.01;
    let comfortable =
        (view.terminal_line_height - COMFORTABLE_TERMINAL_LINE_HEIGHT).abs() < 0.01;
    let presets = [
        ("MesloLGS NF", "MesloLGS NF"),
        ("JetBrainsMono NF", "JetBrainsMono NF"),
        ("FiraCode NF", "FiraCode Nerd Font"),
        ("Cascadia Mono", "Cascadia Mono"),
        ("Consolas", "Consolas"),
        ("Default", DEFAULT_TERMINAL_FONT_FAMILY),
    ];

    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().font_semibold().text_left().child(t.terminal_font))
                .child(
                    display_btn("terminal-font-reset")
                        .ghost()
                        .small()
                        .icon(AssetIcon::RotateCcw)
                        .label(t.reset_status_colors)
                        .tooltip(t.tip_reset_terminal_font)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.reset_terminal_font(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.terminal_font_help),
        )
        .child(
            div()
                .text_sm()
                .text_left()
                .child(t.terminal_font_family),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(presets.into_iter().map(|(label, family)| {
                    let family = family.to_string();
                    let selected = view.terminal_font_family == family
                        || (label == "Default"
                            && view.terminal_font_family == DEFAULT_TERMINAL_FONT_FAMILY);
                    display_btn(SharedString::from(format!("term-font-preset:{label}")))
                        .outline()
                        .small()
                        .label(label.to_string())
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_terminal_font_family(family.clone(), window, cx);
                        }))
                        .into_any_element()
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(&view.terminal_font_input)),
                )
                .child(
                    action_btn("apply-terminal-font", AssetIcon::Save)
                        .outline()
                        .small()
                        .label(t.save)
                        .tooltip(t.tip_apply_terminal_font)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.apply_terminal_font_from_input(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_left()
                        .child(format!("{}: {size_label}", t.terminal_font_size)),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            display_btn("term-font-smaller")
                                .outline()
                                .small()
                                .label("A−")
                                .tooltip(t.tip_terminal_font_smaller)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.bump_terminal_font_size(-1.0, window, cx);
                                })),
                        )
                        .child(
                            display_btn("term-font-larger")
                                .outline()
                                .small()
                                .label("A+")
                                .tooltip(t.tip_terminal_font_larger)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.bump_terminal_font_size(1.0, window, cx);
                                })),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_left()
                        .child(t.terminal_line_height),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            display_btn("term-line-standard")
                                .outline()
                                .small()
                                .label(t.terminal_line_standard)
                                .tooltip(t.tip_terminal_line_standard)
                                .selected(standard)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_terminal_line_height(
                                        DEFAULT_TERMINAL_LINE_HEIGHT,
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            display_btn("term-line-comfortable")
                                .outline()
                                .small()
                                .label(t.terminal_line_comfortable)
                                .tooltip(t.tip_terminal_line_comfortable)
                                .selected(comfortable)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_terminal_line_height(
                                        COMFORTABLE_TERMINAL_LINE_HEIGHT,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                ),
        )
}

fn status_colors_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().font_semibold().text_left().child(t.status_colors))
                .child(
                    display_btn("status-colors-reset-all")
                        .ghost()
                        .small()
                        .icon(AssetIcon::RotateCcw)
                        .label(t.reset_status_colors)
                        .tooltip(t.tip_reset_status_colors)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.reset_status_colors(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.status_colors_help),
        )
        .children(StatusColors::KEYS.into_iter().map(|key| {
            let label = display_label(key);
            let picker = view
                .status_color_pickers
                .get(key)
                .expect("status picker")
                .clone();
            let status_key = key.to_string();
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .w(px(120.))
                        .text_sm()
                        .text_left()
                        .child(label),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .flex_1()
                        .min_w_0()
                        .child(ColorSelect::new(&picker).w_full().flex_1())
                        .child(
                            icon_btn(
                                SharedString::from(format!("status-color-reset:{key}")),
                                AssetIcon::RotateCcw,
                            )
                            .accessibility_label(t.tip_reset_status_color)
                            .tooltip(t.tip_reset_status_color)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.reset_status_color(&status_key, window, cx);
                            })),
                        ),
                )
                .into_any_element()
        }))
}

fn editor_select(
    view: &AppView,
    tip: &'static str,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let current = view.preferred_editor.preset;
    let entity = cx.entity();

    div().w(px(200.)).child(
        display_btn("editor-select-trigger")
            .outline()
            .small()
            .w_full()
            .label(format!("{}  ▾", current.label()))
            .tooltip(tip)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |menu, _, _| {
                EditorPreset::ALL.into_iter().fold(menu, |menu, preset| {
                    let entity = entity.clone();
                    let checked = current == preset;
                    menu.item(
                        menu_item(preset.label(), move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.set_editor_preset(preset, cx);
                            });
                        })
                        .checked(checked),
                    )
                })
            }),
    )
}