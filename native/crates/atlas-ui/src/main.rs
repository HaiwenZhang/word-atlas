//! Starts Word Atlas on the machine's native desktop backend.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn main() -> impl std::process::Termination {
    env_logger::init();
    let asset_root = std::env::var_os("WORD_ATLAS_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../public")
        });
    platform::set_ui_sets(vec![nsh_base::registry(), word_atlas_native::registry()]);
    platform::run_app(
        platform::AppSource::Rust(
            platform::Root::of::<word_atlas_native::App>().app_id("dev.wordatlas.client"),
        ),
        platform::AppOptions {
            title: "词光 · Word Atlas".into(),
            app_id: "dev.wordatlas.client".into(),
            size: (1180, 820),
            asset_base: Some(platform::AssetBase::Directory(asset_root)),
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn main() {
    eprintln!("Mobile platforms use the Rust root in nsh.json: nsh run --target ios|android");
}
