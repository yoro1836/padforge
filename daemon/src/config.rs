use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct Config {
    pub vid: u16,
    pub pid: u16,
    pub plugin_dir: String,
    pub hide_device: bool,
    pub values: HashMap<String, String>,
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let mut cfg = Config {
            vid: 0x045e,
            pid: 0x028e,
            plugin_dir: "/sdcard/.keyforge/plugins".into(),
            hide_device: false,
            values: HashMap::new(),
        };
        if let Ok(file) = fs::File::open(path) {
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    let key = k.trim().to_lowercase();
                    let val = v.trim().to_string();
                    match key.as_str() {
                        "vid" => cfg.vid = parse_hex16(&val),
                        "pid" => cfg.pid = parse_hex16(&val),
                        "plugin_dir" => cfg.plugin_dir = val,
                        "hide_device" => cfg.hide_device = parse_bool(&val),
                        _ => {
                            cfg.values.insert(key, val);
                        }
                    }
                }
            }
        }
        cfg
    }
}

fn parse_hex16(s: &str) -> u16 {
    let s = s.trim().strip_prefix("0x").unwrap_or(s);
    u16::from_str_radix(s, 16).unwrap_or(0)
}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_hiding_without_exposing_it_to_plugins() {
        let path =
            std::env::temp_dir().join(format!("keyforge-config-{}.conf", std::process::id()));
        fs::write(
            &path,
            "VID=0x054c\nPID=0x0ce6\nHIDE_DEVICE=on\nplugin.deadzone=1\n",
        )
        .unwrap();

        let config = Config::load(&path);
        assert!(config.hide_device);
        assert_eq!(config.vid, 0x054c);
        assert_eq!(config.pid, 0x0ce6);
        assert_eq!(
            config.values.get("plugin.deadzone").map(String::as_str),
            Some("1")
        );
        assert!(!config.values.contains_key("hide_device"));
        fs::remove_file(path).unwrap();
    }
}
