mod i18n;
mod locale;
mod logger;
mod preferences;
mod region;

use anyhow::{Context as _, Result};
use ely_gpui_component::{
    Assets,
    i18n::I18n,
    theme::{ActiveTheme, Mode, Theme},
};

use crate::i18n::Translator;
use gpui::{
    App, AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, WindowOptions,
    div,
};

struct Root;

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let waking = cx.global::<Translator>().t("splash.tips.waking", &[]);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.colors.bg)
            .text_color(theme.colors.fg)
            .child(waking)
    }
}

fn open(cx: &mut App, translator: Translator) -> Result<()> {
    ely_gpui_component::init(cx).context("Ely failed to start")?;
    cx.set_global(I18n::new(translator.ely_tag()));
    cx.set_global(translator);
    Theme::set_mode(Mode::Dark, cx);
    cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Root))
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
    let tag = locale::effective(language);
    let translator = Translator::new(tag).unwrap_or_else(|error| panic!("{error:#}"));
    log::info!(target: "locale", "locale {tag}, Ely components in {}", translator.ely_tag());
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            open(cx, translator).unwrap_or_else(|error| panic!("startup failed: {error:#}"));
        });
}
