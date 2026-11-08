use std::path::PathBuf;

use anyhow::{Context as _, Result};

pub const REGION: &str = env!("CINDY_REGION");

/// Cindy's own folder for this region, shared with it.
pub fn user_data_dir() -> Result<PathBuf> {
    let name = match REGION {
        "cn" => "Cindy",
        "global" => "CindyGlobal",
        "dev" => "CindyDev",
        other => unreachable!("build.rs rejects region {other}"),
    };
    let base = dirs::config_dir().context("this user has no app-data folder")?;
    Ok(base.join(name))
}

#[cfg(test)]
mod tests {
    #[test]
    fn shares_cindy_folder_for_built_region() {
        let cindy = match super::REGION {
            "cn" => "Cindy",
            "global" => "CindyGlobal",
            _ => "CindyDev",
        };
        let expected = dirs::config_dir().unwrap().join(cindy);
        assert_eq!(super::user_data_dir().unwrap(), expected);
    }
}
