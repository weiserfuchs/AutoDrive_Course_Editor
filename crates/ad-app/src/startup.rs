//! Startup: OS directories, logging, panic hook and settings (AD-12).

use std::path::PathBuf;

use ad_io::settings::{self, Settings};
use directories::ProjectDirs;
use tracing_appender::non_blocking::WorkerGuard;

/// What `init` hands to the app. Keep `_log_guard` alive so buffered log lines are flushed.
pub struct Startup {
    pub settings: Settings,
    pub _log_guard: Option<WorkerGuard>,
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let text = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic without message".to_owned());
    match info.location() {
        Some(loc) => format!("{text} (at {}:{})", loc.file(), loc.line()),
        None => text,
    }
}

fn install_panic_hook(log_dir: Option<PathBuf>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = panic_message(info);
        tracing::error!("panic: {message}");
        let location = match &log_dir {
            Some(dir) => format!("Log directory: {}", dir.display()),
            None => "No log directory available.".to_owned(),
        };
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("AutoDrive Course Editor crashed")
            .set_description(format!("{message}\n\n{location}"))
            .show();
        previous(info);
    }));
}

fn start_logging(log_dir: &std::path::Path) -> Option<WorkerGuard> {
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("editor.log")
        .build(log_dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .try_init()
        .ok()?;
    Some(guard)
}

/// Resolves directories, installs the panic hook, starts logging and loads the settings.
pub fn init() -> Startup {
    let dirs = ProjectDirs::from("", "", "AutoDrive Course Editor");
    let log_dir = dirs.as_ref().map(|d| d.data_dir().join("logs"));
    let log_guard = log_dir.as_deref().and_then(start_logging);
    install_panic_hook(log_guard.as_ref().and(log_dir));

    if cfg!(debug_assertions) && std::env::var_os("AD_DEBUG_PANIC").is_some_and(|v| v == "1") {
        panic!("AD_DEBUG_PANIC requested a test panic");
    }

    let settings = match dirs {
        Some(d) => load_settings(&d.config_dir().join("settings.toml")),
        None => {
            tracing::warn!("no config directory available; using default settings");
            Settings::default()
        }
    };
    Startup {
        settings,
        _log_guard: log_guard,
    }
}

fn load_settings(path: &std::path::Path) -> Settings {
    tracing::info!("loading settings from {}", path.display());
    match settings::load(path) {
        Ok(settings) => {
            if !path.exists() {
                match settings::save(&settings, path) {
                    Ok(()) => tracing::info!("wrote default settings to {}", path.display()),
                    Err(e) => tracing::warn!("cannot write default settings: {e}"),
                }
            }
            settings
        }
        Err(e) => {
            tracing::warn!("{e}; using default settings and leaving the file untouched");
            Settings::default()
        }
    }
}
