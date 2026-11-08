mod i18n;
mod locale;
mod logger;
mod os_appearance;
mod preferences;
mod region;
mod theme;
mod theme_import;

use anyhow::{Context as _, Result};
use ely_gpui_component::{
    Assets,
    i18n::I18n,
    theme::{Mode as ElyMode, Theme},
};

use crate::{
    i18n::Translator,
    theme::{Appearance, Kind},
};
use gpui::{
    App, AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, WindowOptions,
    div,
};

struct Root;

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = cx.global::<Appearance>();
        let waking = cx.global::<Translator>().t("splash.tips.waking", &[]);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(appearance.token("surface"))
            .text_color(appearance.token("text-primary"))
            .child(waking)
    }
}

fn ely_mode(kind: Kind) -> ElyMode {
    match kind {
        Kind::Light => ElyMode::Light,
        Kind::Dark => ElyMode::Dark,
    }
}

fn log_theme(appearance: &Appearance) {
    let active = &appearance.active;
    log::info!(
        target: "theme",
        "theme {} (family {}, mode {:?}{})",
        active.theme.id,
        active.family,
        appearance.mode,
        if active.fallback { ", other type stands in" } else { "" }
    );
}

/// Re-resolves the theme when the OS turns light or dark.
fn follow_os(_: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let os = window.appearance();
    let appearance = cx.global_mut::<Appearance>();
    let changed = appearance
        .follow(os)
        .unwrap_or_else(|error| panic!("theme failed to follow the OS: {error:#}"));
    if !changed {
        return;
    }
    log::info!(target: "theme", "OS appearance {os:?}");
    apply_theme(cx);
}

/// Hands the active theme to Ely and repaints.
fn apply_theme(cx: &mut App) {
    let appearance = cx.global::<Appearance>();
    log_theme(appearance);
    let (kind, palette) = (appearance.kind(), theme::ely::palette(appearance));
    Theme::set_palette(ely_mode(kind), Some(palette), cx);
    Theme::set_mode(ely_mode(kind), cx);
    cx.refresh_windows();
}

/// Reloads themes after each Omarchy switch, as Cindy's `local-themes:changed` does.
fn follow_omarchy(dir: &std::path::Path, cx: &mut App) {
    let (sender, mut switches) = futures::channel::mpsc::unbounded();
    let watcher = match theme::omarchy::watch(dir, sender) {
        Ok(Some(watcher)) => watcher,
        Ok(None) => return,
        Err(error) => {
            return log::error!(target: "theme", "Omarchy theme watcher not started: {error:#}");
        }
    };
    cx.spawn(async move |cx| {
        let _watcher = watcher;
        while futures::StreamExt::next(&mut switches).await.is_some() {
            cx.update(|cx| {
                log::info!(target: "theme", "Omarchy theme switched; reloading themes");
                if let Err(error) = cx.global_mut::<Appearance>().reload() {
                    panic!("themes failed to reload: {error:#}");
                }
                apply_theme(cx);
            });
        }
    })
    .detach();
}

fn open(cx: &mut App, translator: Translator, theme: (theme::Mode, Option<String>)) -> Result<()> {
    ely_gpui_component::init(cx).context("Ely failed to start")?;
    cx.set_global(I18n::new(translator.ely_tag()));
    cx.set_global(translator);
    let home = dirs::home_dir().context("this user has no home folder")?;
    let sources = theme::Sources {
        local_dir: theme::local::dir(&home),
        omarchy_dir: theme::omarchy::state_dir(&home),
    };
    let omarchy_dir = sources.omarchy_dir.clone();
    let appearance = Appearance::new(
        theme.0,
        theme.1.as_deref(),
        os_appearance::initial(cx),
        Some(sources),
    )?;
    log_theme(&appearance);
    let mode = ely_mode(appearance.kind());
    Theme::set_palette(mode, Some(theme::ely::palette(&appearance)), cx);
    Theme::set_mode_now(mode, cx);
    cx.set_global(appearance);
    follow_omarchy(&omarchy_dir, cx);
    cx.open_window(WindowOptions::default(), |window, cx| {
        cx.new(|cx| {
            cx.observe_window_appearance(window, follow_os).detach();
            Root
        })
    })
    .context("window failed to open")?;
    cx.activate(true);
    log::info!("main window opened");
    Ok(())
}

fn main() {
    let user_data = region::user_data_dir().unwrap_or_else(|error| panic!("{error:#}"));
    logger::init(&user_data).unwrap_or_else(|error| panic!("logger failed to start: {error:#}"));
    log::info!("region {} userData {}", region::REGION, user_data.display());
    let preferences = preferences::Preferences::load(&user_data)
        .unwrap_or_else(|error| panic!("preferences failed to load: {error:#}"));
    let language = preferences
        .text("language")
        .unwrap_or_else(|error| panic!("{error:#}"));
    let theme_mode = theme::Mode::parse(
        preferences
            .text("theme")
            .unwrap_or_else(|error| panic!("{error:#}")),
    );
    let family = preferences
        .text("theme.familyId")
        .unwrap_or_else(|error| panic!("{error:#}"))
        .map(str::to_owned);
    let tag = locale::effective(language);
    let translator = Translator::new(tag).unwrap_or_else(|error| panic!("{error:#}"));
    log::info!(target: "locale", "locale {tag}, Ely components in {}", translator.ely_tag());
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            open(cx, translator, (theme_mode, family))
                .unwrap_or_else(|error| panic!("startup failed: {error:#}"));
        });
}
