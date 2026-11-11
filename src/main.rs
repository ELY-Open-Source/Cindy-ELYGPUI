mod activation_click;
mod app_shortcuts;
mod appearance_settings;
mod atomic_file;
mod attachment_safety;
mod bare_space_images;
mod bounded_read;
mod browse_history;
mod browser_file;
mod chat_message;
mod chat_quotes;
mod chat_store;
mod chrome;
mod cindy_json;
mod command_display;
mod command_intent;
mod command_output;
mod confirm;
mod confirm_dialog;
mod copy_media;
mod cross_process_lock;
mod deep_link;
mod dev_profile;
mod dock_icon;
mod file_location;
mod file_path_policy;
mod file_presentation;
mod generated_files;
mod goal_verdict;
mod handoff_card;
mod history_render;
mod history_view;
mod hook_card;
mod i18n;
mod js_date;
mod js_re;
mod js_sort;
mod layout_root;
mod layout_tree;
mod link_open;
mod linkify;
mod local_path;
mod localdb;
mod locale;
mod logger;
mod lucide;
mod main_window;
mod markdown;
mod markdown_media;
mod markdown_target;
mod markdown_text;
mod media_store;
mod message_rows;
mod motion;
mod node_path;
mod open_external;
mod open_path;
mod os_appearance;
mod path_resolver;
mod path_tokens;
mod payload;
mod plan_todos;
mod preferences;
mod press;
mod region;
mod remote_media_url;
mod render_items;
mod rich_answer;
mod router;
mod session_owner;
mod session_view;
mod sessions;
mod show_item;
mod sidebar;
mod smooth_scroll;
mod source_label;
mod theme;
mod theme_import;
mod tip;
mod toast;
mod tool_media;
mod tool_row;
mod tool_use;
mod transition;
mod units;
mod url_boundary;
mod user_message;
mod v8_classes;
mod wallpaper;
mod window_material;
mod window_state;
mod work_groups;
mod work_runs;
mod workspace_media;

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use ely_gpui_component::{Assets, i18n::I18n, theme::Theme};
use gpui::App;

use crate::{i18n::Translator, theme::Appearance};

/// Everything read before the app starts.
struct Startup {
    user_data: PathBuf,
    database: Option<PathBuf>,
    owner_dir: Option<PathBuf>,
    translator: Translator,
    theme_mode: theme::Mode,
    family: Option<String>,
    appearance: appearance_settings::Settings,
    activation: main_window::Activation,
    preferences: preferences::Preferences,
}

/// Cindy's `applyFontSettings` on Ely's theme: UI scale and families.
fn apply_fonts(fonts: &appearance_settings::Settings, cx: &mut App) {
    let ui = appearance_settings::first_family(&fonts.ui_family);
    let code = appearance_settings::first_family(&fonts.code_family);
    log::info!(target: "appearance-settings-store", "fonts: ui {ui:?} code {code:?} size {} code size {}", fonts.ui_size, fonts.code_size);
    cx.set_global(appearance_settings::CodeSize(fonts.code_size as f32));
    Theme::update(cx, |theme| {
        theme.font_scale = (fonts.ui_size / appearance_settings::DEFAULT_UI_SIZE) as f32;
        if let Some(family) = ui {
            theme.font_family = family.into();
        }
        if let Some(family) = code {
            theme.mono_family = family.into();
        }
    });
}

fn open(startup: Startup, cx: &mut App) -> Result<()> {
    ely_gpui_component::init(cx).context("Ely failed to start")?;
    cx.set_global(motion::ReducedMotion::default());
    motion::refresh_reduced_motion(cx);
    apply_fonts(&startup.appearance, cx);
    cx.set_global(units::Zoom(startup.appearance.window_zoom as f32));
    cx.set_global(I18n::new(startup.translator.ely_tag()));
    cx.set_global(startup.translator);
    cx.set_global(startup.preferences);
    cx.set_global(toast::Toasts::default());
    chat_store::install(cx);
    sessions::install(startup.user_data.clone(), startup.owner_dir.clone(), cx);
    localdb::install(startup.database.clone(), cx);
    router::install(cx);
    session_view::install(cx);
    browse_history::install(cx);
    link_open::install(cx);
    wallpaper::Wallpaper::install(startup.appearance.wallpaper.clone(), &startup.user_data, cx);
    cx.set_global(window_material::Material::load());
    let home = dirs::home_dir().context("this user has no home folder")?;
    let sources = theme::Sources {
        local_dir: theme::local::dir(&home),
        omarchy_dir: theme::omarchy::state_dir(&home),
    };
    let omarchy_dir = sources.omarchy_dir.clone();
    let initial = os_appearance::initial(cx);
    let appearance = Appearance::new(
        startup.theme_mode,
        startup.family.as_deref(),
        initial,
        Some(sources),
    )?;
    theme::shell::install(appearance, cx);
    dock_icon::install(startup.appearance.app_icon, cx);
    theme::shell::follow_omarchy(&omarchy_dir, cx);
    cx.set_global(layout_tree::LayoutStore::open(&startup.user_data));
    cx.set_global(app_shortcuts::AppShortcuts::load(&startup.user_data));
    main_window::open(startup.user_data, startup.activation, cx)?;
    cx.activate(true);
    Ok(())
}

fn startup() -> Result<Startup> {
    let user_data = dev_profile::user_data_dir(region::user_data_dir()?)?;
    std::fs::create_dir_all(&user_data)
        .with_context(|| format!("create {}", user_data.display()))?;
    logger::init(&user_data).context("logger failed to start")?;
    log::info!("region {} userData {}", region::REGION, user_data.display());
    let preferences =
        preferences::Preferences::load(&user_data).context("preferences failed to load")?;
    let mode = session_owner::read_mode(&user_data);
    let owner = session_owner::owner(mode, &user_data);
    if let Some(owner) = &owner {
        log::info!(target: "appSessionState", "data owner {} in {}", owner.id, owner.dir.display());
    }
    let database = owner
        .as_ref()
        .map(|owner| localdb::path(&user_data, owner.id));
    let owner_dir = owner.map(|owner| owner.dir);
    let tag = locale::effective(preferences.text("language")?);
    let translator = Translator::new(tag)?;
    log::info!(target: "locale", "locale {tag}, Ely components in {}", translator.ely_tag());
    Ok(Startup {
        database,
        owner_dir,
        theme_mode: theme::Mode::parse(preferences.text("theme")?),
        family: preferences.text("theme.familyId")?.map(str::to_owned),
        appearance: appearance_settings::read(&user_data),
        activation: main_window::Activation {
            mac_setting: main_window::mac_swallow_setting(&user_data),
            windows_setting: preferences.text("input.swallowActivationClick")? == Some("true"),
        },
        translator,
        user_data,
        preferences,
    })
}

fn main() {
    cross_process_lock::note_process_start();
    let startup = startup().unwrap_or_else(|error| panic!("startup failed: {error:#}"));
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            open(startup, cx).unwrap_or_else(|error| panic!("startup failed: {error:#}"));
        });
}
