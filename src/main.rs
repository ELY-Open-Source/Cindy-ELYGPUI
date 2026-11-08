mod region;

use anyhow::{Context as _, Result};
use ely_gpui_component::{
    Assets,
    theme::{ActiveTheme, Mode, Theme},
};
use gpui::{
    App, AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, WindowOptions,
    div,
};

struct Root;

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.colors.bg)
            .text_color(theme.colors.fg)
            .child("Cindy")
    }
}

fn open(cx: &mut App) -> Result<()> {
    ely_gpui_component::init(cx).context("Ely failed to start")?;
    Theme::set_mode(Mode::Dark, cx);
    cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Root))
        .context("window failed to open")?;
    cx.activate(true);
    log::info!("main window opened");
    Ok(())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,cindy=info"))
        .init();
    log::info!("cindy {} starting", env!("CARGO_PKG_VERSION"));
    let user_data = region::user_data_dir().unwrap_or_else(|error| panic!("{error:#}"));
    log::info!("region {} userData {}", region::REGION, user_data.display());
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            open(cx).unwrap_or_else(|error| panic!("startup failed: {error:#}"));
        });
}
