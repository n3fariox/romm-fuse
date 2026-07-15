use anyhow::{bail, Result};

const MISTER: &str = include_str!("mister.toml");
const RETROARCH: &str = include_str!("retroarch.toml");
const EMULATIONSTATION: &str = include_str!("emulationstation.toml");

pub fn get_builtin(name: &str) -> Result<&'static str> {
    match name {
        "mister" => Ok(MISTER),
        "retroarch" => Ok(RETROARCH),
        "emulationstation" => Ok(EMULATIONSTATION),
        _ => bail!("unknown built-in profile: {name}"),
    }
}
