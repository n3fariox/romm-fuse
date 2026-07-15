mod api;
mod config;
mod fs;
mod profiles;

use anyhow::Result;
use clap::Parser;
use log::info;

use crate::config::Args;

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let resolved = config::ResolvedConfig::resolve(&args)?;

    info!("romm-fuse starting");
    info!("RomM URL: {}", resolved.romm_url);
    info!("Profile: {}", resolved.profile);
    info!("Cache dir: {}", resolved.cache_dir.display());

    let mountpoint = resolved.mountpoint.clone();

    fs::mount(resolved)?;

    info!("romm-fuse unmounted from {}", mountpoint.display());
    Ok(())
}
