use crate::state::Config;
use anyhow::Result;
use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::*;

pub(crate) fn config_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE")
                .unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("agent-monitor")
}

pub(crate) fn agents_dir() -> PathBuf {
    config_dir().join("agents")
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let cfg: Config = serde_json::from_str(&content)?;
    Ok(cfg)
}

pub fn save_config(config: &Config) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(config_path(), content)?;
    Ok(())
}

pub fn set_auto_start(enabled: bool) -> Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        let key_path: Vec<u16> = OsStr::new(
            r"Software\Microsoft\Windows\CurrentVersion\Run"
        )
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );

        if result.is_err() {
            return Err(anyhow::anyhow!("Failed to open registry key"));
        }

        if enabled {
            let exe_path: Vec<u16> = std::env::current_exe()
                .unwrap_or_default()
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let value_name: Vec<u16> = OsStr::new("AgentMonitor")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let bytes = std::slice::from_raw_parts(
                exe_path.as_ptr() as *const u8,
                exe_path.len() * 2,
            );

            let _ = RegSetValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                0,
                REG_SZ,
                Some(bytes),
            );
        } else {
            let value_name: Vec<u16> = OsStr::new("AgentMonitor")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let _ = RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr()));
        }

        let _ = RegCloseKey(hkey);
    }

    Ok(())
}
