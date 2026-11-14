use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::editor::EditorPreset;
use crate::app::i18n::Locale;
use crate::app::prefs::{ThemePreference, settings_path};
use crate::app::view::AppView;
use crate::app::widgets::btn;

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
        .p_6()
        .gap_6()
        .overflow_y_scrollbar()
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
                        .gap_2()
                        .child(
                            btn("theme-light")
                                .outline()
                                .label(t.theme_light)
                                .tooltip(t.tip_theme_light)
                                .selected(theme == ThemePreference::Light)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_theme_preference(ThemePreference::Light, cx)
                                })),
                        )
                        .child(
                            btn("theme-dark")
                                .outline()
                                .label(t.theme_dark)
                                .tooltip(t.tip_theme_dark)
                                .selected(theme == ThemePreference::Dark)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_theme_preference(ThemePreference::Dark, cx)
                                })),
                        )
                        .child(
                            btn("theme-system")
                                .outline()
                                .label(t.theme_system)
                                .tooltip(t.tip_theme_system)
                                .selected(theme == ThemePreference::System)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_theme_preference(ThemePreference::System, cx)
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
                        .gap_2()
                        .children(Locale::ALL.into_iter().map(|item| {
                            let tip = match item {
                                Locale::English => t.tip_locale_en,
                                Locale::Korean => t.tip_locale_ko,
                            };
                            btn(SharedString::from(format!("locale-{}", item.label())))
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
                .child(div().font_semibold().text_left().child(t.external_editor))
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.external_editor_help),
                )
                .child(editor_select(view, t.tip_editor_select, cx))
                .when(editor.preset == EditorPreset::Custom, |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_left()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.custom_command),
                            )
                            .child(Input::new(&view.custom_editor_input))
                            .child(
                                btn("save-custom-editor")
                                    .outline()
                                    .small()
                                    .label(t.save_command)
                                    .tooltip(t.tip_save_command)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_custom_editor_command(cx)
                                    })),
                            ),
                    )
                })
                .child(
                    btn("open-project-editor")
                        .primary()
                        .label(format!("{} {}", t.open_project_in, editor.label()))
                        .tooltip(t.tip_open_project)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_project_in_editor(cx)
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
                .child(div().font_semibold().text_left().child(t.repository))
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{}: {path}", t.current_path)),
                )
                .child(
                    btn("settings-open-folder")
                        .primary()
                        .label(t.open_folder)
                        .tooltip(t.tip_open_folder)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_folder(window, cx)
                        })),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{}: {settings_file}", t.settings_file)),
        )
}

fn editor_select(
    view: &AppView,
    tip: &'static str,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let current = view.preferred_editor.preset.label();
    let open = view.editor_select_open;

    div()
        .flex()
        .flex_col()
        .w(px(280.))
        .gap_1()
        .child(
            btn("editor-select-trigger")
                .outline()
                .w_full()
                .label(format!("{current}  ▾"))
                .tooltip(tip)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_editor_select(cx))),
        )
        .when(open, |this| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .bg(cx.theme().background)
                    .overflow_hidden()
                    .children(EditorPreset::ALL.into_iter().map(|preset| {
                        let selected = view.preferred_editor.preset == preset;
                        btn(SharedString::from(format!("editor-opt-{}", preset.label())))
                            .ghost()
                            .w_full()
                            .label(preset.label())
                            .selected(selected)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_editor_preset(preset, cx)
                            }))
                            .into_any_element()
                    })),
            )
        })
}