use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::{Disableable as _, StyledExt as _};
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
use crate::app::ai_provider::{AiAuthKind, AiProviderKind, AiProviderPreset, ACTIVE_AUTO};
use crate::app::prefs::{
    settings_path, CommitAiStyle, StatusColors, TerminalShellKind, ThemePreference,
    COMFORTABLE_TERMINAL_LINE_HEIGHT, DEFAULT_TERMINAL_LINE_HEIGHT,
};
use crate::app::render::status_style::display_label;
use crate::app::view::AppView;
use crate::app::widgets::{
    action_btn, display_btn, icon_btn, icon_danger_btn, list_row_btn, menu_item, with_busy,
};

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
                .child(commit_ai_section(view, t, cx))
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
                .child(updates_section(view, t, cx))
                .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{}: {settings_file}", t.settings_file)),
        ),
        )
}

fn updates_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let version = crate::app::update::current_version();
    let latest = view
        .update_available
        .as_ref()
        .map(|info| info.version.clone());
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(div().font_semibold().text_left().child(t.updates))
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.updates_help),
        )
        .child(
            div()
                .text_sm()
                .text_left()
                .line_height(relative(1.35))
                .child(format!("{}: {version}", t.current_version)),
        )
        .when_some(latest, |this, ver| {
            this.child(
                div()
                    .text_sm()
                    .text_left()
                    .line_height(relative(1.35))
                    .child(format!("{}: {ver}", t.update_available)),
            )
        })
        .child(
            div()
                .flex()
                .justify_start()
                .items_center()
                .gap_3()
                .child(
                    Switch::new("auto-check-updates")
                        .checked(view.auto_check_updates)
                        .label(t.auto_check_updates)
                        .on_click(cx.listener(|this, checked, _, cx| {
                            this.set_auto_check_updates(*checked, cx);
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(if view.auto_check_updates {
                            t.tip_auto_check_updates_on
                        } else {
                            t.tip_auto_check_updates_off
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(with_busy(
                    action_btn("check-for-updates", AssetIcon::RefreshCw)
                        .outline()
                        .label(t.check_for_updates)
                        .tooltip(t.tip_check_for_updates)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.check_for_updates(false, window, cx);
                        })),
                    view.busy,
                    BusyKind::CheckUpdate,
                ))
                .when(view.update_available.is_some(), |this| {
                    this.child(with_busy(
                        action_btn("download-update", AssetIcon::CloudDownload)
                            .primary()
                            .label(t.download_update)
                            .tooltip(t.tip_download_update)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.download_available_update(cx);
                            })),
                        view.busy,
                        BusyKind::DownloadUpdate,
                    ))
                }),
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
    let query = view.ui_font_input.read(cx).value().to_string();
    let fonts = view.system_font_names.as_deref().unwrap_or(&[]);

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
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.tip_font_search),
        )
        .child(
            div()
                .text_xs()
                .text_left()
                .line_height(relative(1.35))
                .child(format!(
                    "{}: {}",
                    t.font_current,
                    if view.ui_font_family.trim().is_empty() {
                        t.font_theme_default.to_string()
                    } else {
                        view.ui_font_family.clone()
                    }
                )),
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
                        .child(Input::new(&view.ui_font_input).cleanable(true)),
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
        .child(system_font_picker_list(
            "ui-font",
            fonts,
            &query,
            &view.ui_font_family,
            true,
            t,
            cx,
        ))
}

fn commit_ai_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let style = view.commit_ai.style;
    let active = view.commit_ai.active_provider_id.clone();
    let edit_id = view.commit_ai_edit_id.clone();
    let providers = view.commit_ai.providers.clone();
    let editing = providers.iter().find(|p| p.id == edit_id).cloned();
    let is_http = editing
        .as_ref()
        .is_some_and(|p| p.kind == AiProviderKind::OpenAiCompatible);
    let auth = editing.as_ref().map(|p| p.auth).unwrap_or(AiAuthKind::Bearer);
    let docs_url = editing
        .as_ref()
        .map(|p| p.docs_url.clone())
        .unwrap_or_default();
    let style_tips = [
        (CommitAiStyle::Conventional, t.tip_commit_ai_style_conventional),
        (CommitAiStyle::Free, t.tip_commit_ai_style_free),
    ];

    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(div().font_semibold().text_left().child(t.commit_ai))
        .child(
            div()
                .text_xs()
                .text_left()
                .line_height(relative(1.35))
                .text_color(cx.theme().muted_foreground)
                .child(t.commit_ai_help),
        )
        .child(div().text_sm().text_left().child(t.commit_ai_active))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    display_btn("commit-ai-active:auto")
                        .outline()
                        .small()
                        .label("Auto")
                        .tooltip(t.tip_commit_ai_active_auto)
                        .selected(active == ACTIVE_AUTO || active.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_commit_ai_active_provider(ACTIVE_AUTO, cx);
                        })),
                )
                .children(providers.iter().map(|p| {
                    let id = p.id.clone();
                    let id_sel = id.clone();
                    let label = p.title.clone();
                    let selected = active == id;
                    display_btn(SharedString::from(format!("commit-ai-active:{id}")))
                        .outline()
                        .small()
                        .label(label)
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_commit_ai_active_provider(id_sel.clone(), cx);
                        }))
                        .into_any_element()
                })),
        )
        .child(div().text_sm().text_left().child(t.commit_ai_style))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(style_tips.into_iter().map(|(kind, tip)| {
                    display_btn(SharedString::from(format!("commit-ai-style:{}", kind.label())))
                        .outline()
                        .small()
                        .label(kind.label().to_string())
                        .tooltip(tip)
                        .selected(style == kind)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_commit_ai_style(kind, cx);
                        }))
                        .into_any_element()
                })),
        )
        .child(div().text_sm().text_left().child(t.commit_ai_providers))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(providers.iter().enumerate().map(|(idx, p)| {
                    let id = p.id.clone();
                    let id_sel = id.clone();
                    let id_up = id.clone();
                    let id_down = id.clone();
                    let id_rm = id.clone();
                    let id_en = id.clone();
                    let enabled = p.enabled;
                    let selected = edit_id == id;
                    let last = idx + 1 >= providers.len();
                    let kind_label = p.kind.label();
                    let title = p.title.clone();
                    let meta = if p.kind == AiProviderKind::OpenAiCompatible {
                        format!("{kind_label} · {}", p.model)
                    } else {
                        kind_label.to_string()
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .py_1()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    list_row_btn(
                                        SharedString::from(format!("commit-ai-row:{id}")),
                                        title,
                                    )
                                    .w_full()
                                    .selected(selected)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select_commit_ai_provider(
                                            id_sel.clone(),
                                            window,
                                            cx,
                                        );
                                    })),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .text_xs()
                                        .line_height(relative(1.35))
                                        .text_color(cx.theme().muted_foreground)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .child(meta),
                                ),
                        )
                        .child(
                            Switch::new(SharedString::from(format!("commit-ai-en:{id}")))
                                .checked(enabled)
                                .tooltip(t.commit_ai_enabled)
                                .on_click(cx.listener(move |this, checked, _, cx| {
                                    this.set_commit_ai_provider_enabled(
                                        id_en.clone(),
                                        *checked,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            icon_btn(
                                SharedString::from(format!("commit-ai-up:{id}")),
                                AssetIcon::ArrowUp,
                            )
                            .tooltip(t.tip_move_commit_ai_provider_up)
                            .disabled(idx == 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_commit_ai_provider(id_up.clone(), -1, cx);
                            })),
                        )
                        .child(
                            icon_btn(
                                SharedString::from(format!("commit-ai-down:{id}")),
                                AssetIcon::ArrowDown,
                            )
                            .tooltip(t.tip_move_commit_ai_provider_down)
                            .disabled(last)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_commit_ai_provider(id_down.clone(), 1, cx);
                            })),
                        )
                        .child(
                            icon_danger_btn(
                                SharedString::from(format!("commit-ai-rm:{id}")),
                                AssetIcon::Trash,
                            )
                            .tooltip(t.tip_remove_commit_ai_provider)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_commit_ai_provider(id_rm.clone(), window, cx);
                            })),
                        )
                        .into_any_element()
                })),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.commit_ai_add_preset),
                )
                .children(AiProviderPreset::HTTP_PRESETS.into_iter().map(|preset| {
                    display_btn(SharedString::from(format!("commit-ai-preset:{}", preset.slug())))
                        .outline()
                        .small()
                        .label(preset.title().to_string())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.add_commit_ai_preset(preset, window, cx);
                        }))
                        .into_any_element()
                }))
                .child(
                    display_btn("commit-ai-preset:cursor_cli")
                        .outline()
                        .small()
                        .label(AiProviderPreset::CursorCli.title().to_string())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_commit_ai_preset(AiProviderPreset::CursorCli, window, cx);
                        })),
                )
                .child(
                    display_btn("commit-ai-preset:codex_cli")
                        .outline()
                        .small()
                        .label(AiProviderPreset::CodexCli.title().to_string())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_commit_ai_preset(AiProviderPreset::CodexCli, window, cx);
                        })),
                )
                .child(
                    action_btn("commit-ai-add-custom", AssetIcon::Plus)
                        .outline()
                        .small()
                        .label(t.commit_ai_add_custom)
                        .tooltip(t.tip_commit_ai_add_custom)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_custom_commit_ai_provider(window, cx);
                        })),
                ),
        )
        .when(editing.is_some(), |this| {
            this.child(div().text_sm().text_left().child(t.commit_ai_provider_title))
                .child(Input::new(&view.commit_ai_title_input))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().text_left().child(t.commit_ai_provider_docs))
                                .child(Input::new(&view.commit_ai_docs_input)),
                        )
                        .when(!docs_url.trim().is_empty(), |row| {
                            let url = docs_url.clone();
                            row.child(
                                icon_btn("commit-ai-open-docs", AssetIcon::ExternalLink)
                                    .tooltip(t.tip_commit_ai_open_docs)
                                    .on_click(move |_, _, cx| {
                                        cx.open_url(&url);
                                    }),
                            )
                        }),
                )
                .when(is_http, |this| {
                    this.child(div().text_sm().text_left().child(t.commit_ai_provider_base))
                        .child(Input::new(&view.commit_ai_base_input))
                        .child(div().text_sm().text_left().child(t.commit_ai_provider_path))
                        .child(Input::new(&view.commit_ai_path_input))
                        .child(div().text_sm().text_left().child(t.commit_ai_provider_model))
                        .child(Input::new(&view.commit_ai_model_input))
                        .child(div().text_sm().text_left().child(t.commit_ai_provider_auth))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .children(AiAuthKind::ALL.into_iter().map(|kind| {
                                    display_btn(SharedString::from(format!(
                                        "commit-ai-auth:{}",
                                        kind.label()
                                    )))
                                    .outline()
                                    .small()
                                    .label(kind.label().to_string())
                                    .selected(auth == kind)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_commit_ai_auth(kind, cx);
                                    }))
                                    .into_any_element()
                                })),
                        )
                        .when(auth == AiAuthKind::Header, |this| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_left()
                                    .child(t.commit_ai_provider_auth_header),
                            )
                            .child(Input::new(&view.commit_ai_auth_header_input))
                        })
                        .when(auth == AiAuthKind::Query, |this| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_left()
                                    .child(t.commit_ai_provider_auth_query),
                            )
                            .child(Input::new(&view.commit_ai_auth_query_input))
                        })
                        .when(auth != AiAuthKind::None, |this| {
                            this.child(
                                div().text_sm().text_left().child(t.commit_ai_provider_key),
                            )
                            .child(Input::new(&view.commit_ai_key_input))
                            .child(
                                div()
                                    .text_sm()
                                    .text_left()
                                    .child(t.commit_ai_provider_key_env),
                            )
                            .child(Input::new(&view.commit_ai_key_env_input))
                        })
                })
        })
        .child(div().text_sm().text_left().child(t.commit_ai_cursor_bin))
        .child(Input::new(&view.commit_ai_cursor_bin_input))
        .child(div().text_sm().text_left().child(t.commit_ai_codex_bin))
        .child(Input::new(&view.commit_ai_codex_bin_input))
        .child(
            div()
                .flex()
                .justify_end()
                .child(
                    action_btn("apply-commit-ai-provider", AssetIcon::Save)
                        .outline()
                        .small()
                        .label(t.save)
                        .tooltip(t.tip_apply_commit_ai_provider)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.apply_commit_ai_provider(window, cx);
                        })),
                ),
        )
}

fn terminal_shell_tip(kind: TerminalShellKind, t: UiText) -> &'static str {
    match kind {
        TerminalShellKind::PowerShell => t.tip_terminal_shell_powershell,
        TerminalShellKind::Cmd => t.tip_terminal_shell_cmd,
        TerminalShellKind::GitBash => t.tip_terminal_shell_git_bash,
        TerminalShellKind::Bash => t.tip_terminal_shell_bash,
        TerminalShellKind::Zsh => t.tip_terminal_shell_zsh,
        TerminalShellKind::Custom => t.tip_terminal_shell_custom,
    }
}

fn terminal_shell_section(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let kind = view.terminal_shell.kind;
    let options: Vec<(TerminalShellKind, &'static str)> = TerminalShellKind::available_kinds()
        .into_iter()
        .map(|shell_kind| (shell_kind, terminal_shell_tip(shell_kind, t)))
        .collect();

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
                .children(options.into_iter().map(|(shell_kind, tip)| {
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

const FONT_PICKER_MATCH_LIMIT: usize = 120;

fn filtered_system_fonts<'a>(fonts: &'a [String], query: &str) -> Vec<&'a str> {
    let q = query.trim().to_lowercase();
    fonts
        .iter()
        .map(String::as_str)
        .filter(|name| q.is_empty() || name.to_lowercase().contains(&q))
        .take(FONT_PICKER_MATCH_LIMIT)
        .collect()
}

/// Searchable installed-font list. `allow_empty_default` adds a theme-default row for UI fonts.
fn system_font_picker_list(
    id_prefix: &'static str,
    fonts: &[String],
    query: &str,
    selected: &str,
    allow_empty_default: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let matches = filtered_system_fonts(fonts, query);
    let q = query.trim().to_lowercase();
    let show_default = allow_empty_default
        && (q.is_empty()
            || t.font_theme_default.to_lowercase().contains(&q)
            || "default".contains(&q)
            || ".systemuifont".contains(&q));
    let selected_empty = selected.trim().is_empty();
    let is_ui = id_prefix == "ui-font";

    div()
        .max_h(px(220.))
        .min_h(px(96.))
        .w_full()
        .min_w_0()
        .overflow_y_scrollbar()
        .flex()
        .flex_col()
        .gap_1()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_md()
        .p_2()
        .when(show_default, |this| {
            this.child(
                list_row_btn(
                    SharedString::from(format!("{id_prefix}:theme-default")),
                    t.font_theme_default,
                )
                .selected(selected_empty)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if is_ui {
                        this.set_ui_font_family(String::new(), window, cx);
                    }
                })),
            )
        })
        .when(matches.is_empty() && !show_default, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .text_left()
                    .py_1()
                    .child(t.font_list_empty),
            )
        })
        .children(matches.into_iter().map(|family| {
            let family_owned = family.to_string();
            let selected_row = selected == family;
            let family_for_click = family_owned.clone();
            // Preview the face in the row label when the platform can resolve it.
            display_btn(SharedString::from(format!("{id_prefix}:{family}")))
                .ghost()
                .w_full()
                .selected(selected_row)
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .py_1()
                        .text_left()
                        .text_sm()
                        .line_height(relative(1.35))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .font_family(family_owned.clone())
                        .child(family_owned),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    if is_ui {
                        this.set_ui_font_family(family_for_click.clone(), window, cx);
                    } else {
                        this.set_terminal_font_family(family_for_click.clone(), window, cx);
                    }
                }))
                .into_any_element()
        }))
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
    let query = view.terminal_font_input.read(cx).value().to_string();
    let fonts = view.system_font_names.as_deref().unwrap_or(&[]);

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
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.tip_font_search),
        )
        .child(
            div()
                .text_xs()
                .text_left()
                .line_height(relative(1.35))
                .child(format!("{}: {}", t.font_current, view.terminal_font_family)),
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
                        .child(Input::new(&view.terminal_font_input).cleanable(true)),
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
        .child(system_font_picker_list(
            "term-font",
            fonts,
            &query,
            &view.terminal_font_family,
            false,
            t,
            cx,
        ))
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