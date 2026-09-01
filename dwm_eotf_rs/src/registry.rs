use anyhow::Result;
use windows_registry::LOCAL_MACHINE;

const MPO_REG_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\Dwm";
const MPO_REG_NAME: &str = "OverlayTestMode";

pub fn is_mpo_enabled() -> Result<bool> {
    let value = LOCAL_MACHINE
        .create(MPO_REG_KEY)?
        .get_u32(MPO_REG_NAME)
        .unwrap_or(0);

    Ok(value & 5 == 0)
}

pub fn set_mpo_state(enabled: bool) -> Result<()> {
    let value = if enabled { 0 } else { 5 };

    Ok(LOCAL_MACHINE
        .create(MPO_REG_KEY)?
        .set_u32(MPO_REG_NAME, value)?)
}
