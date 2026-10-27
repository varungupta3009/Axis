use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const USER_CONFIG_PATH: &str = "/home/root/.config/axis/config.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub launch_type: Option<String>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

fn default_color() -> String {
    "#7ca8ff".to_string()
}

impl AppItem {
    pub fn rgb(&self) -> [f32; 3] {
        parse_hex_rgb(&self.color)
    }

    pub fn is_internal_settings(&self) -> bool {
        self.id == "__settings__"
            || self.id == "internal.settings"
            || self.launch_type.as_deref() == Some("internal")
    }

    pub fn is_screen_off(&self) -> bool {
        self.id == "__screen_off__"
            || self.id == "internal.screenoff"
            || self.launch_type.as_deref() == Some("screenoff")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Categories {
    pub up: Vec<AppItem>,
    pub down: Vec<AppItem>,
    pub left: Vec<AppItem>,
    pub right: Vec<AppItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LauncherConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub clock24: bool,
    #[serde(default = "default_temp_unit")]
    pub temp_unit: String,
    #[serde(default = "default_location")]
    pub location_name: String,
    #[serde(default = "default_lat")]
    pub latitude: f64,
    #[serde(default = "default_lon")]
    pub longitude: f64,
    #[serde(default = "default_true")]
    pub bg_animated: bool,
    #[serde(default = "default_true")]
    pub card_animations: bool,
    #[serde(default = "default_theme")]
    pub bg_theme: String,
    #[serde(default = "default_density")]
    pub bg_density: String,
    #[serde(default = "default_speed")]
    pub bg_speed: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_false")]
    pub show_telemetry: bool,
    pub categories: Categories,
    #[serde(default)]
    pub hidden: Vec<String>,
}

fn default_version() -> u32 {
    1
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_temp_unit() -> String {
    "fahrenheit".to_string()
}
fn default_location() -> String {
    "San Francisco".to_string()
}
fn default_lat() -> f64 {
    37.7749
}
fn default_lon() -> f64 {
    -122.4194
}
fn default_theme() -> String {
    "classic".to_string()
}
fn default_density() -> String {
    "normal".to_string()
}
fn default_speed() -> String {
    "normal".to_string()
}
fn default_font_family() -> String {
    "lg".to_string()
}

pub fn parse_hex_rgb(hex: &str) -> [f32; 3] {
    let s = hex.trim_start_matches('#');
    if s.len() >= 6 {
        let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(124) as f32 / 255.0;
        let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(168) as f32 / 255.0;
        let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(255) as f32 / 255.0;
        [r, g, b]
    } else {
        [0.486, 0.659, 1.0]
    }
}

pub fn app_base_dir() -> PathBuf {
    let webos_dir = PathBuf::from("/media/developer/apps/usr/palm/applications/org.axis.native");
    if webos_dir.join("assets").exists() {
        return webos_dir;
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("assets").exists() {
            return cwd;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.to_path_buf();
        }
    }
    webos_dir
}

/// Executes a root command from inside the webOS jail via Homebrew Channel's public exec service,
/// falling back to direct `sh -c` if running outside the jail as root.
pub fn run_root_sh(cmd: &str) -> Option<String> {
    let payload = serde_json::json!({ "command": cmd }).to_string();
    if let Ok(out) = Command::new("/usr/bin/luna-send-pub")
        .args(["-n", "1", "luna://org.webosbrew.hbchannel.service/exec", &payload])
        .output()
    {
        if let Ok(text) = String::from_utf8(out.stdout) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(stdout_str) = val.get("stdoutString").and_then(|v| v.as_str()) {
                    return Some(stdout_str.to_string());
                }
            }
        }
    }
    let out = Command::new("sh").args(["-c", cmd]).output().ok()?;
    String::from_utf8(out.stdout).ok()
}

pub fn load_default_config(base_dir: &Path) -> LauncherConfig {
    let default_path = base_dir.join("assets").join("config.default.json");
    if let Ok(data) = fs::read_to_string(&default_path) {
        if let Ok(cfg) = serde_json::from_str::<LauncherConfig>(&data) {
            return cfg;
        }
    }
    LauncherConfig {
        version: 1,
        clock24: false,
        temp_unit: default_temp_unit(),
        location_name: default_location(),
        latitude: default_lat(),
        longitude: default_lon(),
        bg_animated: true,
        card_animations: true,
        bg_theme: "classic".to_string(),
        bg_density: "normal".to_string(),
        bg_speed: "normal".to_string(),
        font_family: default_font_family(),
        show_telemetry: false,
        categories: Categories {
            up: Vec::new(),
            down: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
        },
        hidden: Vec::new(),
    }
}

pub fn load_config(base_dir: &Path) -> LauncherConfig {
    if let Ok(data) = fs::read_to_string(USER_CONFIG_PATH) {
        if let Ok(cfg) = serde_json::from_str::<LauncherConfig>(&data) {
            if !cfg.categories.up.is_empty() || !cfg.categories.down.is_empty() {
                return cfg;
            }
        }
    }
    let cfg = load_default_config(base_dir);
    save_config(&cfg);
    cfg
}

pub fn save_config(cfg: &LauncherConfig) {
    if let Some(parent) = Path::new(USER_CONFIG_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let tmp_path = format!("{}.tmp", USER_CONFIG_PATH);
        if fs::write(&tmp_path, &json).is_ok() {
            let _ = fs::rename(&tmp_path, USER_CONFIG_PATH);
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppSyncResult {
    pub installed_ids: HashSet<String>,
    pub icon_map: HashMap<String, String>,
    pub extra_apps: Vec<AppItem>,
}

pub fn is_protected_builtin(app: &AppItem) -> bool {
    app.is_internal_settings()
        || app.is_screen_off()
        || app.id.starts_with("__")
        || app.id.starts_with("internal.")
        || app.id.starts_with("com.webos.app.hdmi")
        || app.launch_type.as_deref() == Some("input")
        || app.id == "com.palm.app.settings"
        || app.id == "com.webos.app.livetv"
        || app.id == "com.webos.app.connectionwizard"
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn pick_accent_color(bg: &str) -> String {
    let s = bg.trim().trim_start_matches('#');
    if s.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            if (r as u32 + g as u32 + b as u32) >= 120 {
                return format!("#{}", &s[0..6]);
            }
        }
    }
    "#8ea2ff".to_string()
}

pub fn sync_installed_apps(existing: &Categories) -> Option<AppSyncResult> {
    let mut known = HashSet::new();
    let mut needs_staged_icon = HashSet::new();
    for a in existing
        .up
        .iter()
        .chain(existing.down.iter())
        .chain(existing.left.iter())
        .chain(existing.right.iter())
    {
        known.insert(a.id.clone());
        if a.icon.is_empty() || a.icon.starts_with("/tmp/axis_icons/") {
            needs_staged_icon.insert(a.id.clone());
        }
    }
    known.insert("org.axis.native".to_string());
    known.insert("org.cardinal.native".to_string());
    known.insert("tld.my.customhome".to_string());
    known.insert("com.webos.app.home".to_string());

    let text =
        run_root_sh("luna-send -n 1 luna://com.webos.applicationManager/listLaunchPoints '{}'")?;
    let val = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let arr = val.get("launchPoints").and_then(|v| v.as_array())?;

    let mut installed_ids = HashSet::new();
    let mut icon_map = HashMap::new();
    let mut extra_apps = Vec::new();
    let mut copy_cmds: Vec<String> = Vec::new();

    for lp in arr {
        let id = lp.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
        if id.is_empty() {
            continue;
        }
        installed_ids.insert(id.to_string());

        let title = lp.get("title").and_then(|v| v.as_str()).unwrap_or("").trim();
        let large_icon = lp.get("largeIcon").and_then(|v| v.as_str()).unwrap_or("").trim();
        let small_icon = lp.get("icon").and_then(|v| v.as_str()).unwrap_or("").trim();
        let folder = lp.get("folderPath").and_then(|v| v.as_str()).unwrap_or("").trim();
        let bg_color = lp.get("bgColor").and_then(|v| v.as_str()).unwrap_or("").trim();

        let raw_icon = if !large_icon.is_empty() {
            large_icon
        } else {
            small_icon
        };
        let src_icon = if raw_icon.is_empty() {
            String::new()
        } else if Path::new(raw_icon).is_absolute() || folder.is_empty() {
            raw_icon.to_string()
        } else {
            format!("{}/{}", folder.trim_end_matches('/'), raw_icon.trim_start_matches('/'))
        };

        let is_new = !known.contains(id);
        if !src_icon.is_empty() && (is_new || needs_staged_icon.contains(id)) {
            let safe_id: String = id
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let lower = src_icon.to_ascii_lowercase();
            let ext = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
                "jpg"
            } else {
                "png"
            };
            let dst_icon = format!("/tmp/axis_icons/{}.{}", safe_id, ext);
            copy_cmds.push(format!(
                "[ -f {src} ] && cp -f {src} {dst} && chmod 644 {dst}",
                src = sh_quote(&src_icon),
                dst = sh_quote(&dst_icon)
            ));
            icon_map.insert(id.to_string(), dst_icon);
        }

        if is_new && !title.is_empty() {
            known.insert(id.to_string());
            extra_apps.push(AppItem {
                id: id.to_string(),
                title: title.to_string(),
                icon: icon_map.get(id).cloned().unwrap_or_default(),
                color: pick_accent_color(bg_color),
                launch_type: None,
                params: None,
            });
        }
    }

    if installed_ids.len() < 5 {
        return None;
    }

    if !copy_cmds.is_empty() {
        let script = format!(
            "mkdir -p /tmp/axis_icons && chmod 755 /tmp/axis_icons; {}; true",
            copy_cmds.join("; ")
        );
        let _ = run_root_sh(&script);
    }

    Some(AppSyncResult {
        installed_ids,
        icon_map,
        extra_apps,
    })
}

#[derive(Clone, Debug)]
pub struct HourForecast {
    pub hour: u32,
    pub temp: i32,
}

#[derive(Clone, Debug)]
pub struct DayForecast {
    pub label: String,
    pub hi: i32,
    pub lo: i32,
}

#[derive(Clone, Debug)]
pub struct WeatherData {
    pub temp: i32,
    pub unit_label: String,
    pub condition: String,
    pub hours: Vec<HourForecast>,
    pub days: Vec<DayForecast>,
}

impl Default for WeatherData {
    fn default() -> Self {
        Self {
            temp: 26,
            unit_label: "°C".to_string(),
            condition: "Partly cloudy".to_string(),
            hours: vec![
                HourForecast { hour: 10, temp: 26 },
                HourForecast { hour: 11, temp: 28 },
                HourForecast { hour: 12, temp: 29 },
                HourForecast { hour: 13, temp: 30 },
                HourForecast { hour: 14, temp: 29 },
            ],
            days: vec![
                DayForecast {
                    label: "Today".to_string(),
                    hi: 30,
                    lo: 20,
                },
                DayForecast {
                    label: "Sat".to_string(),
                    hi: 29,
                    lo: 19,
                },
                DayForecast {
                    label: "Sun".to_string(),
                    hi: 28,
                    lo: 19,
                },
            ],
        }
    }
}

pub fn wmo_label(code: i64) -> &'static str {
    match code {
        0 => "Clear",
        1 => "Mostly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51..=57 => "Drizzle",
        61..=67 | 80..=82 => "Rain",
        71..=77 | 85 | 86 => "Snow",
        95..=99 => "Thunderstorm",
        _ => "Clear",
    }
}

pub fn fetch_weather(lat: f64, lon: f64, temp_unit: &str) -> Option<WeatherData> {
    let is_f = temp_unit == "fahrenheit";
    let unit_param = if is_f { "fahrenheit" } else { "celsius" };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,weather_code&hourly=temperature_2m&daily=temperature_2m_max,temperature_2m_min&temperature_unit={}&timezone=auto&forecast_days=3",
        lat, lon, unit_param
    );

    let cmd = format!("curl -s --max-time 4 '{}'", url);
    let body = run_root_sh(&cmd)?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;

    let cur = json.get("current")?;
    let temp = cur.get("temperature_2m")?.as_f64()?.round() as i32;
    let code = cur.get("weather_code")?.as_i64().unwrap_or(0);
    let condition = wmo_label(code).to_string();

    let mut hours = Vec::new();
    let cur_hour = crate::local_time().2 as usize;
    if let Some(h_temps) = json
        .get("hourly")
        .and_then(|h| h.get("temperature_2m"))
        .and_then(|v| v.as_array())
    {
        for offset in 1..=5 {
            let idx = cur_hour + offset;
            if let Some(t) = h_temps.get(idx).and_then(|v| v.as_f64()) {
                hours.push(HourForecast {
                    hour: ((idx) % 24) as u32,
                    temp: t.round() as i32,
                });
            }
        }
    }

    let mut days = Vec::new();
    let day_names = ["Today", "Tom", "Day 3"];
    if let (Some(maxs), Some(mins)) = (
        json.get("daily")
            .and_then(|d| d.get("temperature_2m_max"))
            .and_then(|v| v.as_array()),
        json.get("daily")
            .and_then(|d| d.get("temperature_2m_min"))
            .and_then(|v| v.as_array()),
    ) {
        for i in 0..3 {
            if let (Some(hi), Some(lo)) = (
                maxs.get(i).and_then(|v| v.as_f64()),
                mins.get(i).and_then(|v| v.as_f64()),
            ) {
                days.push(DayForecast {
                    label: day_names[i].to_string(),
                    hi: hi.round() as i32,
                    lo: lo.round() as i32,
                });
            }
        }
    }

    Some(WeatherData {
        temp,
        unit_label: if is_f { "°F" } else { "°C" }.to_string(),
        condition,
        hours,
        days,
    })
}

pub fn read_rss_mb() -> f32 {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("RssAnon:") {
                if let Some(kb_str) = rest.split_whitespace().next() {
                    if let Ok(kb) = kb_str.parse::<f32>() {
                        return kb / 1024.0;
                    }
                }
            }
        }
    }
    if let Ok(statm) = fs::read_to_string("/proc/self/statm") {
        let mut parts = statm.split_whitespace();
        let _total = parts.next();
        if let Some(rss_pages_str) = parts.next() {
            if let Ok(pages) = rss_pages_str.parse::<u64>() {
                return (pages as f32 * 4096.0) / (1024.0 * 1024.0);
            }
        }
    }
    10.0
}



