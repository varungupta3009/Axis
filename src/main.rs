mod config;
mod ffi;
mod gl_renderer;

use config::{
    app_base_dir, discover_extra_apps, fetch_weather, load_config,
    load_default_config, parse_hex_rgb, read_rss_mb, run_root_sh, save_config, AppItem,
    LauncherConfig, WeatherData,
};
use ffi::*;
use gl_renderer::{ConstellationsState, GlRenderer};
use libc::c_char;
use std::collections::HashSet;
use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[allow(dead_code)]
pub fn log_line(msg: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/axis.log")
    {
        let _ = writeln!(f, "{}", msg);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenId {
    Home,
    Up,    // Gaming
    Down,  // Streaming
    Left,  // Media
    Right, // Misc
}

impl ScreenId {
    pub fn label(&self) -> &'static str {
        match self {
            ScreenId::Home => "Home",
            ScreenId::Up => "Gaming",
            ScreenId::Down => "Streaming",
            ScreenId::Left => "Media",
            ScreenId::Right => "Misc",
        }
    }

    pub fn accent_hex(&self) -> &'static str {
        match self {
            ScreenId::Home => "#7ca8ff",
            ScreenId::Up => "#b39cff",
            ScreenId::Down => "#7ca8ff",
            ScreenId::Left => "#5eead4",
            ScreenId::Right => "#7ee7a6",
        }
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self, ScreenId::Up | ScreenId::Down)
    }

    pub fn near_end(&self) -> bool {
        matches!(self, ScreenId::Up | ScreenId::Left)
    }

    pub fn anchor(&self) -> f32 {
        match self {
            ScreenId::Home => 0.5,
            ScreenId::Up => 0.58,
            ScreenId::Down => 0.44,
            ScreenId::Left => 0.60,
            ScreenId::Right => 0.40,
        }
    }

    pub fn panel_offset(&self) -> (f32, f32) {
        match self {
            ScreenId::Home => (0.0, 0.0),
            ScreenId::Up => (0.0, -1080.0),
            ScreenId::Down => (0.0, 1080.0),
            ScreenId::Left => (-1920.0, 0.0),
            ScreenId::Right => (1920.0, 0.0),
        }
    }

    pub fn world_target(&self) -> (f32, f32) {
        let (px, py) = self.panel_offset();
        (-px, -py)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpringF32 {
    pub val: f32,
    pub target: f32,
    pub vel: f32,
}

impl SpringF32 {
    pub fn new(initial: f32) -> Self {
        Self {
            val: initial,
            target: initial,
            vel: 0.0,
        }
    }

    pub fn set(&mut self, target: f32, snap: bool) {
        self.target = target;
        if snap {
            self.val = target;
            self.vel = 0.0;
        }
    }

    pub fn step(&mut self, dt_sec: f32, stiffness: f32, damping: f32) -> bool {
        let diff = self.target - self.val;
        if diff.abs() < 0.08 && self.vel.abs() < 0.5 {
            self.val = self.target;
            self.vel = 0.0;
            return false;
        }
        // Sub-step for numerical stability
        let steps = 2;
        let h = (dt_sec / steps as f32).min(0.016);
        for _ in 0..steps {
            let force = (self.target - self.val) * stiffness - self.vel * damping;
            self.vel += force * h;
            self.val += self.vel * h;
        }
        true
    }

    pub fn is_moving(&self) -> bool {
        (self.target - self.val).abs() >= 0.08 || self.vel.abs() >= 0.5
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPane {
    Main,
    ManageApps,
}

const SETTINGS_ROWS: usize = 11;

pub fn local_time() -> (u32, u32, u32, u32, u32) {
    // Returns (month 1..12, mday 1..31, hour 0..23, min 0..59, sec 0..59, wday 0..6)
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now);
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        (
            (tm.tm_mon + 1) as u32,
            tm.tm_mday as u32,
            tm.tm_hour as u32,
            tm.tm_min as u32,
            tm.tm_sec as u32,
        )
    }
}

pub fn local_weekday_and_date() -> (String, String) {
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now);
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        let wdays = [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ];
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let wday = wdays.get(tm.tm_wday as usize).unwrap_or(&"Friday");
        let mon = months.get(tm.tm_mon as usize).unwrap_or(&"October");
        (wday.to_string(), format!("{} {}", mon, tm.tm_mday))
    }
}

fn greeting_for(hour: u32) -> &'static str {
    if hour < 5 {
        "Good Night!"
    } else if hour < 12 {
        "Good Morning!"
    } else if hour < 17 {
        "Good Afternoon!"
    } else if hour < 22 {
        "Good Evening!"
    } else {
        "Good Night!"
    }
}

fn set_env(key: &str, val: &str) {
    let k = CString::new(key).unwrap();
    let v = CString::new(val).unwrap();
    unsafe {
        libc::setenv(k.as_ptr(), v.as_ptr(), 0);
    }
}

fn set_hint(name: &str, val: &str) {
    let n = CString::new(name).unwrap();
    let v = CString::new(val).unwrap();
    unsafe {
        SDL_SetHint(n.as_ptr(), v.as_ptr());
    }
}

fn spawn_luna_launch(app: &AppItem) {
    if app.is_screen_off() {
        thread::spawn(|| {
            let _ = run_root_sh(
                "luna-send -n 1 luna://com.webos.service.tvpower/power/turnOffScreen '{\"standbyMode\":\"active\"}'",
            );
        });
        return;
    }

    let id = app.id.clone();
    let params = app.params.clone();
    thread::spawn(move || {
        let wake_hook = if id == "com.beb.plxnative" || id == "cdp-30" {
            "curl -s -m 2 'http://192.168.50.100:8099/wake?service=plex' >/dev/null 2>&1 & "
        } else if id == "org.litefin.app" {
            "curl -s -m 2 'http://192.168.50.100:8099/wake?service=jellyfin' >/dev/null 2>&1 & "
        } else {
            ""
        };
        let payload = if let Some(p) = params {
            serde_json::json!({ "id": id, "params": p }).to_string()
        } else {
            serde_json::json!({ "id": id }).to_string()
        };
        let cmd = format!(
            "{}luna-send -n 1 luna://com.webos.applicationManager/launch '{}'",
            wake_hook, payload
        );
        let _ = run_root_sh(&cmd);
    });
}

fn target_scroll_for(screen: ScreenId, idx: usize) -> f32 {
    if screen.is_vertical() {
        (idx as f32) * 128.0 + 52.0
    } else {
        (idx as f32) * 276.0 + 124.0
    }
}

struct AppState {
    base_dir: PathBuf,
    cfg: LauncherConfig,
    hidden_set: HashSet<String>,
    screen: ScreenId,
    selected: usize,
    moving: bool,
    move_snapshot: Option<config::Categories>,
    settings_open: bool,
    settings_pane: SettingsPane,
    settings_row: usize,
    apps_row: usize,
    reset_done_ms: u32,

    world_x: SpringF32,
    world_y: SpringF32,
    scroll_up: SpringF32,
    scroll_down: SpringF32,
    scroll_left: SpringF32,
    scroll_right: SpringF32,
    bg_fade: SpringF32,
    ambient_fade: SpringF32,

    ok_down_ms: u32,
    ok_held_fired: bool,
    launch_fx_id: Option<String>,
    launch_fx_ms: u32,
    toast_msg: Option<String>,
    toast_until_ms: u32,

    last_input_ms: u32,
    ambient: bool,
    blackout_entered_ms: u32,
    jitter_x: f32,
    jitter_y: f32,
    last_jitter_ms: u32,

    weather: Arc<Mutex<WeatherData>>,
    discovered_rx: Arc<Mutex<Option<Vec<AppItem>>>>,
    rss_mb: f32,
    last_rss_ms: u32,
    fps_display: u32,
    frame_counter: u32,
    fps_timer_ms: u32,
}

impl AppState {
    fn new(base_dir: PathBuf, cfg: LauncherConfig, now_ms: u32) -> Self {
        let hidden_set: HashSet<String> = cfg.hidden.iter().cloned().collect();
        let up_vis = cfg
            .categories
            .up
            .iter()
            .filter(|a| !hidden_set.contains(&a.id))
            .count();
        let left_vis = cfg
            .categories
            .left
            .iter()
            .filter(|a| !hidden_set.contains(&a.id))
            .count();

        let s_up = target_scroll_for(ScreenId::Up, up_vis.saturating_sub(1));
        let s_down = target_scroll_for(ScreenId::Down, 0);
        let s_left = target_scroll_for(ScreenId::Left, left_vis.saturating_sub(1));
        let s_right = target_scroll_for(ScreenId::Right, 0);

        let weather = Arc::new(Mutex::new(WeatherData::default()));
        let discovered_rx = Arc::new(Mutex::new(None));

        let st = Self {
            base_dir,
            cfg,
            hidden_set,
            screen: ScreenId::Home,
            selected: 0,
            moving: false,
            move_snapshot: None,
            settings_open: false,
            settings_pane: SettingsPane::Main,
            settings_row: 0,
            apps_row: 0,
            reset_done_ms: 0,
            world_x: SpringF32::new(0.0),
            world_y: SpringF32::new(0.0),
            scroll_up: SpringF32::new(s_up),
            scroll_down: SpringF32::new(s_down),
            scroll_left: SpringF32::new(s_left),
            scroll_right: SpringF32::new(s_right),
            bg_fade: SpringF32::new(1.0),
            ambient_fade: SpringF32::new(0.0),
            ok_down_ms: 0,
            ok_held_fired: false,
            launch_fx_id: None,
            launch_fx_ms: 0,
            toast_msg: None,
            toast_until_ms: 0,
            last_input_ms: now_ms,
            ambient: false,
            blackout_entered_ms: 0,
            jitter_x: 0.0,
            jitter_y: 0.0,
            last_jitter_ms: now_ms,
            weather,
            discovered_rx,
            rss_mb: read_rss_mb(),
            last_rss_ms: now_ms,
            fps_display: 60,
            frame_counter: 0,
            fps_timer_ms: now_ms,
        };
        st.spawn_weather_refresh();
        st.spawn_app_discovery();
        st
    }

    fn spawn_weather_refresh(&self) {
        let w_arc = Arc::clone(&self.weather);
        let lat = self.cfg.latitude;
        let lon = self.cfg.longitude;
        let unit = self.cfg.temp_unit.clone();
        thread::spawn(move || {
            if let Some(data) = fetch_weather(lat, lon, &unit) {
                if let Ok(mut lock) = w_arc.lock() {
                    *lock = data;
                }
            }
        });
    }

    fn spawn_app_discovery(&self) {
        let d_arc = Arc::clone(&self.discovered_rx);
        let cats = self.cfg.categories.clone();
        thread::spawn(move || {
            let extra = discover_extra_apps(&cats);
            if !extra.is_empty() {
                if let Ok(mut lock) = d_arc.lock() {
                    *lock = Some(extra);
                }
            }
        });
    }

    fn visible_for(&self, screen: ScreenId) -> Vec<AppItem> {
        let list = match screen {
            ScreenId::Home => return Vec::new(),
            ScreenId::Up => &self.cfg.categories.up,
            ScreenId::Down => &self.cfg.categories.down,
            ScreenId::Left => &self.cfg.categories.left,
            ScreenId::Right => &self.cfg.categories.right,
        };
        list.iter()
            .filter(|a| !self.hidden_set.contains(&a.id))
            .cloned()
            .collect()
    }

    fn category_mut(&mut self, screen: ScreenId) -> &mut Vec<AppItem> {
        match screen {
            ScreenId::Up => &mut self.cfg.categories.up,
            ScreenId::Down => &mut self.cfg.categories.down,
            ScreenId::Left => &mut self.cfg.categories.left,
            ScreenId::Right | ScreenId::Home => &mut self.cfg.categories.right,
        }
    }

    fn update_scroll_target(&mut self, screen: ScreenId, idx: usize) {
        let snap = !self.cfg.card_animations;
        let target = target_scroll_for(screen, idx);
        match screen {
            ScreenId::Up => self.scroll_up.set(target, snap),
            ScreenId::Down => self.scroll_down.set(target, snap),
            ScreenId::Left => self.scroll_left.set(target, snap),
            ScreenId::Right => self.scroll_right.set(target, snap),
            ScreenId::Home => {}
        }
    }

    fn set_screen(&mut self, next: ScreenId) {
        let snap = !self.cfg.card_animations;
        self.moving = false;
        self.move_snapshot = None;
        self.screen = next;
        let (wx, wy) = next.world_target();
        self.world_x.set(wx, snap);
        self.world_y.set(wy, snap);
        self.bg_fade
            .set(if next == ScreenId::Home { 1.0 } else { 0.0 }, snap);

        if next == ScreenId::Home {
            self.selected = 0;
            // Reset inactive screens to their entry edge
            let up_len = self.visible_for(ScreenId::Up).len();
            let left_len = self.visible_for(ScreenId::Left).len();
            self.update_scroll_target(ScreenId::Up, up_len.saturating_sub(1));
            self.update_scroll_target(ScreenId::Down, 0);
            self.update_scroll_target(ScreenId::Left, left_len.saturating_sub(1));
            self.update_scroll_target(ScreenId::Right, 0);
        } else {
            let vis = self.visible_for(next);
            let idx = if next.near_end() {
                vis.len().saturating_sub(1)
            } else {
                0
            };
            self.selected = idx;
            self.update_scroll_target(next, idx);
        }
    }

    fn enter_blackout(&mut self, now_ms: u32) {
        if self.ambient {
            return;
        }
        self.ambient = true;
        self.blackout_entered_ms = now_ms;
        self.settings_open = false;
        self.settings_pane = SettingsPane::Main;
        if self.moving {
            if let Some(snap) = self.move_snapshot.take() {
                self.cfg.categories = snap;
            }
            self.moving = false;
        }
        self.ok_down_ms = 0;
        self.ok_held_fired = false;
        self.ambient_fade.set(1.0, true);
        thread::spawn(|| {
            let _ = run_root_sh(
                "echo 1 > /tmp/oled_screen_off; chmod 644 /tmp/oled_screen_off; luna-send -n 1 luna://com.webos.service.tvpower/power/turnOffScreen '{\"standbyMode\":\"active\"}'",
            );
        });
    }

    fn wake_from_blackout(&mut self, now_ms: u32) {
        self.last_input_ms = now_ms;
        if self.ambient {
            self.ambient = false;
            self.ambient_fade.set(0.0, true);
            thread::spawn(|| {
                let _ = run_root_sh(
                    "echo 0 > /tmp/oled_screen_off; chmod 644 /tmp/oled_screen_off; luna-send -n 1 luna://com.webos.service.tvpower/power/turnOnScreen '{}'",
                );
            });
        }
    }

    fn reset_to_home(&mut self, now_ms: u32) {
        self.wake_from_blackout(now_ms);
        self.settings_open = false;
        self.settings_pane = SettingsPane::Main;
        if self.moving {
            if let Some(snap) = self.move_snapshot.take() {
                self.cfg.categories = snap;
            }
            self.moving = false;
        }
        self.ok_down_ms = 0;
        self.ok_held_fired = false;
        self.set_screen(ScreenId::Home);
    }

    fn show_toast(&mut self, msg: String, now_ms: u32) {
        self.toast_msg = Some(msg);
        self.toast_until_ms = now_ms + 2600;
    }

    fn trigger_launch(&mut self, now_ms: u32) {
        if self.screen == ScreenId::Home {
            return;
        }
        let vis = self.visible_for(self.screen);
        if let Some(app) = vis.get(self.selected) {
            if app.is_internal_settings() {
                self.settings_open = true;
                self.settings_pane = SettingsPane::Main;
                self.settings_row = 0;
                return;
            }
            self.launch_fx_id = Some(app.id.clone());
            self.launch_fx_ms = now_ms;
            spawn_luna_launch(app);
        }
    }

    fn on_move_step(&mut self, delta: isize) {
        if self.screen == ScreenId::Home {
            return;
        }
        let vis = self.visible_for(self.screen);
        let new_sel = (self.selected as isize) + delta;
        if new_sel < 0 || new_sel >= (vis.len() as isize) {
            return;
        }
        let target_idx = new_sel as usize;
        let id_a = vis[self.selected].id.clone();
        let id_b = vis[target_idx].id.clone();

        let list = self.category_mut(self.screen);
        if let (Some(ia), Some(ib)) = (
            list.iter().position(|x| x.id == id_a),
            list.iter().position(|x| x.id == id_b),
        ) {
            list.swap(ia, ib);
        }
        self.selected = target_idx;
        self.update_scroll_target(self.screen, target_idx);
    }

    fn on_move_cross(&mut self, target_screen: ScreenId, now_ms: u32) {
        if self.screen == ScreenId::Home || target_screen == self.screen {
            return;
        }
        let vis = self.visible_for(self.screen);
        let Some(app) = vis.get(self.selected).cloned() else {
            return;
        };
        if app.is_internal_settings() {
            return;
        }
        let cur_screen = self.screen;
        self.category_mut(cur_screen).retain(|x| x.id != app.id);
        self.category_mut(target_screen).push(app.clone());
        save_config(&self.cfg);
        self.moving = false;
        self.move_snapshot = None;

        let new_vis = self.visible_for(cur_screen);
        if new_vis.is_empty() {
            self.set_screen(ScreenId::Home);
        } else {
            self.selected = self.selected.min(new_vis.len() - 1);
            self.update_scroll_target(cur_screen, self.selected);
        }
        self.show_toast(
            format!("Moved {} to {}", app.title, target_screen.label()),
            now_ms,
        );
    }

    fn handle_key_down(&mut self, scancode: i32, sym: i32, repeat: bool, now_ms: u32) {
        // Ignore LG webOS synthetic Magic Remote cursor hide scancode (485=CURSOR_HIDE)
        // which fires automatically 5s after the pointer stops moving!
        if scancode == SDL_WEBOS_SCANCODE_GREEN {
            return;
        }

        if self.ambient {
            self.wake_from_blackout(now_ms);
            return;
        }
        self.last_input_ms = now_ms;

        // Ignore synthetic Magic Remote cursor show scancode (484=CURSOR_SHOW) for navigation
        if scancode == SDL_WEBOS_SCANCODE_RED {
            return;
        }

        let is_back = scancode == SDL_WEBOS_SCANCODE_BACK
            || scancode == SDL_SCANCODE_AC_BACK
            || scancode == SDL_SCANCODE_ESCAPE
            || scancode == SDL_SCANCODE_BACKSPACE
            || sym == SDLK_ESCAPE
            || sym == SDLK_BACKSPACE
            || sym == 461;

        let is_ok = scancode == SDL_SCANCODE_RETURN
            || scancode == SDL_SCANCODE_KP_ENTER
            || scancode == SDL_SCANCODE_SPACE
            || sym == SDLK_RETURN;

        if scancode == SDL_SCANCODE_AC_HOME {
            self.reset_to_home(now_ms);
            return;
        }

        if self.settings_open {
            self.handle_settings_key(scancode, is_ok, is_back, now_ms);
            return;
        }

        if is_ok {
            if self.screen == ScreenId::Home || repeat {
                return;
            }
            if self.moving {
                self.ok_held_fired = true;
                save_config(&self.cfg);
                self.moving = false;
                self.move_snapshot = None;
                self.show_toast("Saved app order".to_string(), now_ms);
                return;
            }
            self.ok_down_ms = now_ms;
            self.ok_held_fired = false;
            return;
        }

        if is_back {
            if self.moving {
                if let Some(snap) = self.move_snapshot.take() {
                    self.cfg.categories = snap;
                    let vis_len = self.visible_for(self.screen).len();
                    self.selected = self.selected.min(vis_len.saturating_sub(1));
                    self.update_scroll_target(self.screen, self.selected);
                }
                self.moving = false;
                return;
            }
            if self.screen != ScreenId::Home {
                self.set_screen(ScreenId::Home);
            }
            return;
        }

        let dir = match scancode {
            SDL_SCANCODE_UP => Some(ScreenId::Up),
            SDL_SCANCODE_DOWN => Some(ScreenId::Down),
            SDL_SCANCODE_LEFT => Some(ScreenId::Left),
            SDL_SCANCODE_RIGHT => Some(ScreenId::Right),
            _ => None,
        };
        let Some(dir_screen) = dir else {
            return;
        };

        if self.screen == ScreenId::Home {
            self.set_screen(dir_screen);
            return;
        }

        let is_vert = self.screen.is_vertical();
        let (prev_dir, next_dir) = if is_vert {
            (ScreenId::Up, ScreenId::Down)
        } else {
            (ScreenId::Left, ScreenId::Right)
        };

        if self.moving {
            if dir_screen == next_dir {
                self.on_move_step(1);
            } else if dir_screen == prev_dir {
                self.on_move_step(-1);
            } else {
                self.on_move_cross(dir_screen, now_ms);
            }
            return;
        }

        let count = self.visible_for(self.screen).len();
        if dir_screen == next_dir {
            if self.selected + 1 >= count {
                self.set_screen(ScreenId::Home);
            } else {
                self.selected += 1;
                self.update_scroll_target(self.screen, self.selected);
            }
        } else if dir_screen == prev_dir {
            if self.selected == 0 {
                self.set_screen(ScreenId::Home);
            } else {
                self.selected -= 1;
                self.update_scroll_target(self.screen, self.selected);
            }
        } else {
            self.set_screen(ScreenId::Home);
        }
    }

    fn handle_key_up(&mut self, scancode: i32, sym: i32, now_ms: u32) {
        let is_ok = scancode == SDL_SCANCODE_RETURN
            || scancode == SDL_SCANCODE_KP_ENTER
            || scancode == SDL_SCANCODE_SPACE
            || sym == SDLK_RETURN;
        if !is_ok {
            return;
        }
        let was_down = self.ok_down_ms > 0;
        self.ok_down_ms = 0;
        if was_down && !self.ok_held_fired && !self.moving && !self.settings_open {
            self.trigger_launch(now_ms);
        }
        self.ok_held_fired = false;
    }

    fn all_manageable_apps(&self) -> Vec<(ScreenId, AppItem)> {
        let mut out = Vec::new();
        for (scr, list) in [
            (ScreenId::Up, &self.cfg.categories.up),
            (ScreenId::Down, &self.cfg.categories.down),
            (ScreenId::Left, &self.cfg.categories.left),
            (ScreenId::Right, &self.cfg.categories.right),
        ] {
            for a in list {
                if !a.is_internal_settings() {
                    out.push((scr, a.clone()));
                }
            }
        }
        out
    }

    fn handle_settings_key(&mut self, scancode: i32, is_ok: bool, is_back: bool, now_ms: u32) {
        if self.settings_pane == SettingsPane::ManageApps {
            let flat = self.all_manageable_apps();
            if is_back {
                self.settings_pane = SettingsPane::Main;
                return;
            }
            if scancode == SDL_SCANCODE_UP {
                self.apps_row = self.apps_row.saturating_sub(1);
            } else if scancode == SDL_SCANCODE_DOWN {
                if !flat.is_empty() {
                    self.apps_row = (self.apps_row + 1).min(flat.len() - 1);
                }
            } else if is_ok {
                if let Some((_, app)) = flat.get(self.apps_row) {
                    if self.hidden_set.contains(&app.id) {
                        self.hidden_set.remove(&app.id);
                    } else {
                        self.hidden_set.insert(app.id.clone());
                    }
                    self.cfg.hidden = self.hidden_set.iter().cloned().collect();
                    save_config(&self.cfg);
                }
            }
            return;
        }

        if is_back {
            self.settings_open = false;
            return;
        }
        if scancode == SDL_SCANCODE_UP {
            self.settings_row = self.settings_row.saturating_sub(1);
            return;
        }
        if scancode == SDL_SCANCODE_DOWN {
            self.settings_row = (self.settings_row + 1).min(SETTINGS_ROWS - 1);
            return;
        }

        let is_left = scancode == SDL_SCANCODE_LEFT;
        let is_right = scancode == SDL_SCANCODE_RIGHT;
        if !is_left && !is_right && !is_ok {
            return;
        }

        let step: isize = if is_left { -1 } else { 1 };
        match self.settings_row {
            0 => {
                // Clock format
                self.cfg.clock24 = !self.cfg.clock24;
                save_config(&self.cfg);
            }
            1 => {
                // Temperature unit
                self.cfg.temp_unit = if self.cfg.temp_unit == "celsius" {
                    "fahrenheit".to_string()
                } else {
                    "celsius".to_string()
                };
                save_config(&self.cfg);
                self.spawn_weather_refresh();
            }
            2 => {
                // Background animation (Constellations)
                self.cfg.bg_animated = !self.cfg.bg_animated;
                save_config(&self.cfg);
            }
            3 => {
                // Card & Nudge animations
                self.cfg.card_animations = !self.cfg.card_animations;
                save_config(&self.cfg);
            }
            4 => {
                // Color theme
                let themes = ["classic", "mono", "ember", "aurora"];
                let cur = themes
                    .iter()
                    .position(|&t| t == self.cfg.bg_theme)
                    .unwrap_or(0) as isize;
                let next = (cur + step).rem_euclid(themes.len() as isize) as usize;
                self.cfg.bg_theme = themes[next].to_string();
                save_config(&self.cfg);
            }
            5 => {
                // Star density
                let densities = ["low", "normal", "high"];
                let cur = densities
                    .iter()
                    .position(|&d| d == self.cfg.bg_density)
                    .unwrap_or(1) as isize;
                let next = (cur + step).rem_euclid(densities.len() as isize) as usize;
                self.cfg.bg_density = densities[next].to_string();
                save_config(&self.cfg);
            }
            6 => {
                // Motion speed
                let speeds = ["slow", "normal", "fast"];
                let cur = speeds
                    .iter()
                    .position(|&s| s == self.cfg.bg_speed)
                    .unwrap_or(1) as isize;
                let next = (cur + step).rem_euclid(speeds.len() as isize) as usize;
                self.cfg.bg_speed = speeds[next].to_string();
                save_config(&self.cfg);
            }
            7 => {
                // Typography (LG Smart UI vs Titillium Web)
                self.cfg.font_family = if self.cfg.font_family == "titillium" {
                    "lg".to_string()
                } else {
                    "titillium".to_string()
                };
                save_config(&self.cfg);
            }
            8 => {
                // System Telemetry HUD
                self.cfg.show_telemetry = !self.cfg.show_telemetry;
                save_config(&self.cfg);
            }
            9 => {
                // Manage apps
                if is_ok {
                    self.settings_pane = SettingsPane::ManageApps;
                    self.apps_row = 0;
                }
            }
            10 => {
                // Reset app order
                if is_ok {
                    let def_cfg = load_default_config(&self.base_dir);
                    self.cfg.categories = def_cfg.categories;
                    self.cfg.hidden.clear();
                    self.hidden_set.clear();
                    save_config(&self.cfg);
                    self.reset_done_ms = now_ms + 2200;
                }
            }
            _ => {}
        }
    }

    fn handle_mouse_click(&mut self, mx: f32, my: f32, now_ms: u32) {
        if self.ambient {
            self.wake_from_blackout(now_ms);
            return;
        }
        self.last_input_ms = now_ms;
        if self.settings_open {
            return;
        }
        if self.screen == ScreenId::Home {
            // Check clicks on the 4 directional assist chips
            if (mx - 960.0).abs() < 130.0 && (my - 115.0).abs() < 48.0 {
                self.set_screen(ScreenId::Up);
            } else if (mx - 960.0).abs() < 130.0 && (my - 965.0).abs() < 48.0 {
                self.set_screen(ScreenId::Down);
            } else if (mx - 170.0).abs() < 110.0 && (my - 540.0).abs() < 48.0 {
                self.set_screen(ScreenId::Left);
            } else if (mx - 1750.0).abs() < 110.0 && (my - 540.0).abs() < 48.0 {
                self.set_screen(ScreenId::Right);
            }
        } else {
            self.trigger_launch(now_ms);
        }
    }

    fn springs_moving(&self) -> bool {
        self.world_x.is_moving()
            || self.world_y.is_moving()
            || self.scroll_up.is_moving()
            || self.scroll_down.is_moving()
            || self.scroll_left.is_moving()
            || self.scroll_right.is_moving()
            || self.bg_fade.is_moving()
            || self.ambient_fade.is_moving()
    }

    fn needs_continuous_redraw(&self, now_ms: u32, constellations: &ConstellationsState) -> bool {
        if self.ambient {
            return false;
        }
        if self.springs_moving() {
            return true;
        }
        if self.ok_down_ms > 0 {
            return true;
        }
        if self.launch_fx_id.is_some() && now_ms.saturating_sub(self.launch_fx_ms) < 700 {
            return true;
        }
        if self.toast_msg.is_some() && now_ms < self.toast_until_ms {
            return true;
        }
        if !constellations.trail.is_empty() {
            return true;
        }
        // On Home screen, animate only if bg_animated or card_animations is enabled!
        if self.screen == ScreenId::Home && !self.settings_open {
            if self.cfg.bg_animated || self.cfg.card_animations {
                return true;
            }
        }
        false
    }
}

fn draw_chevron(
    r: &mut GlRenderer,
    cx: f32,
    cy: f32,
    dir: ScreenId,
    col: [f32; 4],
) {
    let s = 8.5;
    let t = 2.6;
    match dir {
        ScreenId::Up => {
            r.push_thick_line(cx - s, cy + s * 0.45, cx, cy - s * 0.45, t, col);
            r.push_thick_line(cx, cy - s * 0.45, cx + s, cy + s * 0.45, t, col);
        }
        ScreenId::Down => {
            r.push_thick_line(cx - s, cy - s * 0.45, cx, cy + s * 0.45, t, col);
            r.push_thick_line(cx, cy + s * 0.45, cx + s, cy - s * 0.45, t, col);
        }
        ScreenId::Left => {
            r.push_thick_line(cx + s * 0.45, cy - s, cx - s * 0.45, cy, t, col);
            r.push_thick_line(cx - s * 0.45, cy, cx + s * 0.45, cy + s, t, col);
        }
        ScreenId::Right | ScreenId::Home => {
            r.push_thick_line(cx - s * 0.45, cy - s, cx + s * 0.45, cy, t, col);
            r.push_thick_line(cx + s * 0.45, cy, cx - s * 0.45, cy + s, t, col);
        }
    }
    r.flush_primitives(2.0);
}

fn draw_analog_clock(
    r: &mut GlRenderer,
    cx: f32,
    cy: f32,
    radius: f32,
    hour: u32,
    min: u32,
    sec: u32,
    alpha: f32,
) {
    // Subtle clock dial surface & ring
    r.draw_rounded_rect(
        cx - radius,
        cy - radius,
        radius * 2.0,
        radius * 2.0,
        radius,
        [1.0, 1.0, 1.0, 0.025 * alpha],
        [1.0, 1.0, 1.0, 0.015 * alpha],
        1.5,
        [1.0, 1.0, 1.0, 0.12 * alpha],
        0.0,
        [0.0; 4],
        false,
    );

    // 12 hour tick marks
    for i in 0..12 {
        let ang = ((i as f32) * 30.0 - 90.0).to_radians();
        let r1 = radius - 8.0;
        let r2 = radius - 3.5;
        r.push_line(
            cx + ang.cos() * r1,
            cy + ang.sin() * r1,
            cx + ang.cos() * r2,
            cy + ang.sin() * r2,
            [1.0, 1.0, 1.0, 0.22 * alpha],
        );
    }

    // Hands
    let h_ang = (((hour % 12) as f32) * 30.0 + (min as f32) * 0.5 - 90.0).to_radians();
    let m_ang = ((min as f32) * 6.0 + (sec as f32) * 0.1 - 90.0).to_radians();
    let s_ang = ((sec as f32) * 6.0 - 90.0).to_radians();

    r.push_thick_line(
        cx,
        cy,
        cx + h_ang.cos() * (radius * 0.50),
        cy + h_ang.sin() * (radius * 0.50),
        3.0,
        [1.0, 1.0, 1.0, 0.88 * alpha],
    );
    r.push_thick_line(
        cx,
        cy,
        cx + m_ang.cos() * (radius * 0.72),
        cy + m_ang.sin() * (radius * 0.72),
        2.2,
        [1.0, 1.0, 1.0, 0.88 * alpha],
    );
    r.push_thick_line(
        cx,
        cy,
        cx + s_ang.cos() * (radius * 0.80),
        cy + s_ang.sin() * (radius * 0.80),
        1.4,
        [0.486, 0.659, 1.0, 0.95 * alpha],
    );
    r.push_octagon(cx, cy, 3.2, [0.486, 0.659, 1.0, alpha]);
    r.flush_primitives(1.5);
}

fn draw_home_panel(
    r: &mut GlRenderer,
    st: &AppState,
    ox: f32,
    oy: f32,
    now_ms: u32,
    ui_alpha: f32,
) {
    if ui_alpha <= 0.01 {
        return;
    }
    let (_, _, hour, min, sec) = local_time();
    let (weekday, month_day) = local_weekday_and_date();

    // 1. Centered Greeting + Location Subtitle
    let greeting = greeting_for(hour);
    r.draw_text(
        greeting,
        ox + 960.0,
        oy + 520.0,
        64,
        false,
        [1.0, 1.0, 1.0, 0.95 * ui_alpha],
        0.5,
        0.5,
    );
    r.draw_text(
        &st.cfg.location_name,
        ox + 960.0,
        oy + 575.0,
        20,
        false,
        [1.0, 1.0, 1.0, 0.48 * ui_alpha],
        0.5,
        0.5,
    );

    // 2. Top-Right Material 3 Elevated Widget Cluster (Date · Weather · Clock)
    let wx = ox + 1920.0 - 96.0 - 348.0;
    let wy = oy + 54.0;
    let ww = 348.0;
    let wh = 376.0;

    r.draw_rounded_rect(
        wx,
        wy,
        ww,
        wh,
        28.0,
        [0.09, 0.10, 0.15, 0.86 * ui_alpha],
        [0.05, 0.06, 0.09, 0.78 * ui_alpha],
        1.0,
        [0.70, 0.74, 0.92, 0.13 * ui_alpha],
        18.0,
        [0.0, 0.0, 0.0, 0.55 * ui_alpha],
        false,
    );

    let right_edge = wx + ww - 30.0;
    let left_edge = wx + 30.0;

    // DateLine
    r.draw_text(
        &weekday,
        right_edge,
        wy + 24.0,
        26,
        true,
        [1.0, 1.0, 1.0, 0.92 * ui_alpha],
        1.0,
        0.0,
    );
    r.draw_text(
        &month_day,
        right_edge,
        wy + 56.0,
        16,
        false,
        [1.0, 1.0, 1.0, 0.55 * ui_alpha],
        1.0,
        0.0,
    );

    // Divider 1
    r.push_line(
        left_edge,
        wy + 88.0,
        right_edge,
        wy + 88.0,
        [0.75, 0.78, 0.95, 0.12 * ui_alpha],
    );
    r.flush_primitives(1.0);

    // WeatherBlock
    if let Ok(w) = st.weather.lock() {
        let temp_str = format!("{}{}", w.temp, w.unit_label);
        r.draw_text(
            &temp_str,
            right_edge,
            wy + 100.0,
            26,
            true,
            [1.0, 1.0, 1.0, 0.92 * ui_alpha],
            1.0,
            0.0,
        );
        r.draw_text(
            &w.condition,
            right_edge,
            wy + 130.0,
            15,
            false,
            [0.486, 0.659, 1.0, 0.80 * ui_alpha],
            1.0,
            0.0,
        );

        // Next 5 hours strip
        let col_w = (ww - 60.0) / 5.0;
        for (i, hf) in w.hours.iter().take(5).enumerate() {
            let cx = left_edge + col_w * (i as f32 + 0.5);
            let h_lbl = if st.cfg.clock24 {
                format!("{:02}:00", hf.hour)
            } else {
                let h12 = ((hf.hour + 11) % 12) + 1;
                format!("{}{}", h12, if hf.hour < 12 { "a" } else { "p" })
            };
            r.draw_text(
                &h_lbl,
                cx,
                wy + 162.0,
                13,
                false,
                [1.0, 1.0, 1.0, 0.45 * ui_alpha],
                0.5,
                0.0,
            );
            r.draw_text(
                &format!("{}°", hf.temp),
                cx,
                wy + 182.0,
                15,
                true,
                [1.0, 1.0, 1.0, 0.85 * ui_alpha],
                0.5,
                0.0,
            );
        }

        // Divider 2
        r.push_line(
            left_edge,
            wy + 212.0,
            right_edge,
            wy + 212.0,
            [0.75, 0.78, 0.95, 0.12 * ui_alpha],
        );
        r.flush_primitives(1.0);

        // 3-day outlook
        let d_col_w = (ww - 60.0) / 3.0;
        for (i, df) in w.days.iter().take(3).enumerate() {
            let cx = left_edge + d_col_w * (i as f32 + 0.5);
            r.draw_text(
                &df.label,
                cx,
                wy + 222.0,
                13,
                false,
                [1.0, 1.0, 1.0, 0.50 * ui_alpha],
                0.5,
                0.0,
            );
            r.draw_text(
                &format!("{}° / {}°", df.hi, df.lo),
                cx,
                wy + 242.0,
                14,
                true,
                [1.0, 1.0, 1.0, 0.82 * ui_alpha],
                0.5,
                0.0,
            );
        }
    }

    // Divider 3
    r.push_line(
        left_edge,
        wy + 272.0,
        right_edge,
        wy + 272.0,
        [0.75, 0.78, 0.95, 0.12 * ui_alpha],
    );
    r.flush_primitives(1.0);

    // Compact Analog + Digital Clock
    draw_analog_clock(r, left_edge + 42.0, wy + 324.0, 40.0, hour, min, sec, ui_alpha);

    let (digital_str, ampm_str) = if st.cfg.clock24 {
        (format!("{:02}:{:02}", hour, min), "")
    } else {
        let h12 = ((hour + 11) % 12) + 1;
        (
            format!("{}:{:02}", h12, min),
            if hour < 12 { "AM" } else { "PM" },
        )
    };

    if !ampm_str.is_empty() {
        let (aw, _) = r.draw_text(
            ampm_str,
            right_edge,
            wy + 336.0,
            18,
            true,
            [1.0, 1.0, 1.0, 0.50 * ui_alpha],
            1.0,
            0.5,
        );
        r.draw_text(
            &digital_str,
            right_edge - aw - 8.0,
            wy + 324.0,
            46,
            true,
            [1.0, 1.0, 1.0, 0.95 * ui_alpha],
            1.0,
            0.5,
        );
    } else {
        r.draw_text(
            &digital_str,
            right_edge,
            wy + 324.0,
            48,
            true,
            [1.0, 1.0, 1.0, 0.95 * ui_alpha],
            1.0,
            0.5,
        );
    }

    // System Telemetry HUD (top-left, mirroring the top-right Weather card)
    if st.cfg.show_telemetry {
        let sx = ox + 96.0;
        let sy = oy + 54.0;
        let sw = 340.0;
        let sh = 204.0;

        r.draw_rounded_rect(
            sx,
            sy,
            sw,
            sh,
            26.0,
            [0.09, 0.10, 0.15, 0.86 * ui_alpha],
            [0.05, 0.06, 0.09, 0.78 * ui_alpha],
            1.0,
            [0.70, 0.74, 0.92, 0.14 * ui_alpha],
            18.0,
            [0.0, 0.0, 0.0, 0.55 * ui_alpha],
            false,
        );

        // Vector Axis Crosshair Emblem
        let lcx = sx + 38.0;
        let lcy = sy + 34.0;
        let col = [0.486, 0.659, 1.0, 0.96 * ui_alpha]; // #7ca8ff

        // Center dot
        r.draw_rounded_rect(
            lcx - 3.5,
            lcy - 3.5,
            7.0,
            7.0,
            3.5,
            col,
            col,
            0.0,
            [0.0; 4],
            0.0,
            [0.0; 4],
            false,
        );

        // Crosshair ticks
        r.push_line(lcx - 14.0, lcy, lcx - 6.0, lcy, col);
        r.push_line(lcx + 6.0, lcy, lcx + 14.0, lcy, col);
        r.push_line(lcx, lcy - 14.0, lcx, lcy - 6.0, col);
        r.push_line(lcx, lcy + 6.0, lcx, lcy + 14.0, col);

        // Outer ring
        r.draw_rounded_rect(
            lcx - 14.0,
            lcy - 14.0,
            28.0,
            28.0,
            14.0,
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            1.5,
            [0.486, 0.659, 1.0, 0.45 * ui_alpha],
            0.0,
            [0.0; 4],
            false,
        );

        r.draw_text(
            "Axis Telemetry",
            sx + 64.0,
            sy + 24.0,
            18,
            true,
            [1.0, 1.0, 1.0, 0.94 * ui_alpha],
            0.0,
            0.5,
        );
        r.draw_text(
            "webOS Native • OpenGL ES 2.0",
            sx + 64.0,
            sy + 44.0,
            13,
            false,
            [1.0, 1.0, 1.0, 0.52 * ui_alpha],
            0.0,
            0.5,
        );

        let status_txt = "ACTIVE";
        let status_col = [0.494, 0.906, 0.651, 0.95 * ui_alpha];
        r.draw_text(
            status_txt,
            sx + sw - 24.0,
            sy + 34.0,
            14,
            true,
            status_col,
            1.0,
            0.5,
        );

        // Divider 1
        r.push_line(
            sx + 22.0,
            sy + 64.0,
            sx + sw - 22.0,
            sy + 64.0,
            [0.75, 0.78, 0.95, 0.12 * ui_alpha],
        );
        r.flush_primitives(1.0);

        // 2x2 Telemetry Grid: Memory (RSS), Display Mode, Graphics API, Refresh Sync
        let res_str = "1920×1080";
        let rss_str = format!("{:.1} MB", st.rss_mb);
        let fps_str = format!("{} FPS", st.fps_display);
        let items: [(&str, &str); 4] = [
            ("Memory (RSS)", &rss_str),
            ("Display Mode", res_str),
            ("Graphics API", "OpenGL ES 2.0"),
            ("Refresh Sync", &fps_str),
        ];
        for (idx, (label, val)) in items.iter().enumerate() {
            let col_idx = idx % 2;
            let row_idx = idx / 2;
            let cell_x = sx + 22.0 + (col_idx as f32) * 152.0;
            let cell_y = sy + 84.0 + (row_idx as f32) * 38.0;

            let dot_col = [0.486, 0.659, 1.0, 0.85 * ui_alpha];
            r.draw_rounded_rect(
                cell_x,
                cell_y - 4.0,
                8.0,
                8.0,
                4.0,
                dot_col,
                dot_col,
                0.0,
                [0.0; 4],
                6.0,
                [dot_col[0], dot_col[1], dot_col[2], 0.35 * ui_alpha],
                false,
            );
            r.draw_text(
                label,
                cell_x + 14.0,
                cell_y - 7.0,
                14,
                true,
                [1.0, 1.0, 1.0, 0.88 * ui_alpha],
                0.0,
                0.5,
            );
            r.draw_text(
                val,
                cell_x + 14.0,
                cell_y + 10.0,
                12,
                false,
                [0.78, 0.84, 0.96, 0.85 * ui_alpha],
                0.0,
                0.5,
            );
        }

        // Divider 2
        r.push_line(
            sx + 22.0,
            sy + 158.0,
            sx + sw - 22.0,
            sy + 158.0,
            [0.75, 0.78, 0.95, 0.12 * ui_alpha],
        );
        r.flush_primitives(1.0);

        let mode_str = if st.cfg.bg_animated || st.cfg.card_animations {
            format!("AXIS ENGINE · {:.1} MB RSS · {} FPS", st.rss_mb, st.fps_display)
        } else {
            format!("AXIS ENGINE · {:.1} MB RSS · IDLE 0% GPU", st.rss_mb)
        };
        r.draw_text(
            &mode_str,
            sx + sw * 0.5,
            sy + 181.0,
            13,
            true,
            [0.68, 0.80, 1.0, 0.82 * ui_alpha],
            0.5,
            0.5,
        );
    }

    // 3. 4 Directional Material You Assist Chips (Gaming ↑, Streaming ↓, Media ←, Misc →)
    let nudge = if st.cfg.card_animations && st.screen == ScreenId::Home {
        ((now_ms as f32 * 0.0024).sin()) * 5.0
    } else {
        0.0
    };
    let chip_rgb = [0.486, 0.659, 1.0]; // #7ca8ff

    // UP: Gaming (top-[7vh] left-1/2, flex-col)
    {
        let cw = 152.0;
        let ch = 78.0;
        let cx = ox + 960.0 - cw * 0.5;
        let cy = oy + 75.0;
        r.draw_rounded_rect(
            cx,
            cy,
            cw,
            ch,
            39.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.13 * ui_alpha],
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.08 * ui_alpha],
            1.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.34 * ui_alpha],
            14.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.25 * ui_alpha],
            false,
        );
        draw_chevron(
            r,
            ox + 960.0,
            cy + 24.0 - nudge.abs(),
            ScreenId::Up,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.95 * ui_alpha],
        );
        r.draw_text(
            "Gaming",
            ox + 960.0,
            cy + 54.0,
            19,
            true,
            [0.933, 0.945, 1.0, 0.95 * ui_alpha],
            0.5,
            0.5,
        );
    }

    // DOWN: Streaming (bottom-[7vh] left-1/2, flex-col-reverse)
    {
        let cw = 176.0;
        let ch = 78.0;
        let cx = ox + 960.0 - cw * 0.5;
        let cy = oy + 1080.0 - 75.0 - ch;
        r.draw_rounded_rect(
            cx,
            cy,
            cw,
            ch,
            39.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.13 * ui_alpha],
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.08 * ui_alpha],
            1.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.34 * ui_alpha],
            14.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.25 * ui_alpha],
            false,
        );
        r.draw_text(
            "Streaming",
            ox + 960.0,
            cy + 26.0,
            19,
            true,
            [0.933, 0.945, 1.0, 0.95 * ui_alpha],
            0.5,
            0.5,
        );
        draw_chevron(
            r,
            ox + 960.0,
            cy + 56.0 + nudge.abs(),
            ScreenId::Down,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.95 * ui_alpha],
        );
    }

    // LEFT: Media (left-[5vw] top-1/2, flex-row)
    {
        let cw = 148.0;
        let ch = 56.0;
        let cx = ox + 96.0;
        let cy = oy + 540.0 - ch * 0.5;
        r.draw_rounded_rect(
            cx,
            cy,
            cw,
            ch,
            28.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.13 * ui_alpha],
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.08 * ui_alpha],
            1.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.34 * ui_alpha],
            14.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.25 * ui_alpha],
            false,
        );
        draw_chevron(
            r,
            cx + 30.0 - nudge.abs(),
            oy + 540.0,
            ScreenId::Left,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.95 * ui_alpha],
        );
        r.draw_text(
            "Media",
            cx + 88.0,
            oy + 540.0,
            19,
            true,
            [0.933, 0.945, 1.0, 0.95 * ui_alpha],
            0.5,
            0.5,
        );
    }

    // RIGHT: Misc (right-[5vw] top-1/2, flex-row-reverse)
    {
        let cw = 140.0;
        let ch = 56.0;
        let cx = ox + 1920.0 - 96.0 - cw;
        let cy = oy + 540.0 - ch * 0.5;
        r.draw_rounded_rect(
            cx,
            cy,
            cw,
            ch,
            28.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.13 * ui_alpha],
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.08 * ui_alpha],
            1.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.34 * ui_alpha],
            14.0,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.25 * ui_alpha],
            false,
        );
        r.draw_text(
            "Misc",
            cx + 54.0,
            oy + 540.0,
            19,
            true,
            [0.933, 0.945, 1.0, 0.95 * ui_alpha],
            0.5,
            0.5,
        );
        draw_chevron(
            r,
            cx + cw - 30.0 + nudge.abs(),
            oy + 540.0,
            ScreenId::Right,
            [chip_rgb[0], chip_rgb[1], chip_rgb[2], 0.95 * ui_alpha],
        );
    }
}

fn truncate_title(title: &str, max_chars: usize) -> String {
    if title.chars().count() <= max_chars {
        title.to_string()
    } else {
        let mut s: String = title.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_alt_panel(
    r: &mut GlRenderer,
    st: &AppState,
    scr: ScreenId,
    ox: f32,
    oy: f32,
    now_ms: u32,
    ui_alpha: f32,
) {
    // Frustum cull entire panel if offscreen
    if ox <= -1920.0 || ox >= 1920.0 || oy <= -1080.0 || oy >= 1080.0 || ui_alpha <= 0.01 {
        return;
    }

    let items = st.visible_for(scr);
    let active = st.screen == scr;
    let selected_idx = if active {
        st.selected
    } else if scr.near_end() {
        items.len().saturating_sub(1)
    } else {
        0
    };

    let scroll_px = match scr {
        ScreenId::Up => st.scroll_up.val,
        ScreenId::Down => st.scroll_down.val,
        ScreenId::Left => st.scroll_left.val,
        ScreenId::Right | ScreenId::Home => st.scroll_right.val,
    };

    let accent = parse_hex_rgb(scr.accent_hex());
    let is_vert = scr.is_vertical();

    // Category header pill & centered title at top-left (left-[5vw] top-[8vh])
    let lbl_w = r
        .get_text_texture(scr.label(), 36, true)
        .map(|t| t.w as f32)
        .unwrap_or(160.0);
    let pill_w = lbl_w + 56.0;
    let pill_h = 66.0;
    let pill_x = ox + 88.0;
    let pill_y = oy + 76.0;
    r.draw_rounded_rect(
        pill_x,
        pill_y,
        pill_w,
        pill_h,
        33.0,
        [accent[0], accent[1], accent[2], 0.10 * ui_alpha],
        [accent[0], accent[1], accent[2], 0.04 * ui_alpha],
        1.0,
        [accent[0], accent[1], accent[2], 0.28 * ui_alpha],
        24.0,
        [accent[0], accent[1], accent[2], 0.16 * ui_alpha],
        false,
    );
    r.draw_text(
        scr.label(),
        pill_x + pill_w * 0.5,
        pill_y + pill_h * 0.5,
        36,
        true,
        [accent[0], accent[1], accent[2], 0.96 * ui_alpha],
        0.5,
        0.5,
    );

    // Draw Tiles along track
    let anchor_px = if is_vert {
        1080.0 * scr.anchor()
    } else {
        1920.0 * scr.anchor()
    };

    for (i, app) in items.iter().enumerate() {
        let focused = active && i == selected_idx;
        let held = focused && st.moving;
        let launching = st.launch_fx_id.as_deref() == Some(app.id.as_str())
            && now_ms.saturating_sub(st.launch_fx_ms) < 600;

        let mut scale = if held {
            1.09
        } else if focused {
            1.05
        } else {
            1.0
        };
        if launching {
            let p = (now_ms.saturating_sub(st.launch_fx_ms) as f32 / 600.0).clamp(0.0, 1.0);
            let pulse = (p * std::f32::consts::PI).sin() * 0.09;
            scale -= pulse;
        }

        let tile_alpha = if focused {
            1.0
        } else if active && st.moving {
            0.40
        } else {
            0.68
        } * ui_alpha;

        let brand = app.rgb();

        if is_vert {
            // Vertical row: w=560, h=104, step=128
            let base_w = 560.0;
            let base_h = 104.0;
            let cx = ox + 960.0;
            let cy = oy + anchor_px - scroll_px + (i as f32) * 128.0 + 52.0;
            if cy < -140.0 || cy > 1220.0 {
                continue;
            }
            let w = base_w * scale;
            let h = base_h * scale;
            let x = cx - w * 0.5;
            let y = cy - h * 0.5;

            let fill_top = if focused {
                [brand[0], brand[1], brand[2], 0.32 * tile_alpha]
            } else {
                [brand[0], brand[1], brand[2], 0.11 * tile_alpha]
            };
            let fill_bot = if focused {
                [brand[0], brand[1], brand[2], 0.09 * tile_alpha]
            } else {
                [1.0, 1.0, 1.0, 0.02 * tile_alpha]
            };
            let border_col = if focused {
                [brand[0], brand[1], brand[2], 0.95 * tile_alpha]
            } else {
                [brand[0], brand[1], brand[2], 0.18 * tile_alpha]
            };
            let glow_r = if focused { 20.0 } else { 0.0 };
            let glow_col = [brand[0], brand[1], brand[2], 0.48 * tile_alpha];

            r.draw_rounded_rect(
                x,
                y,
                w,
                h,
                28.0 * scale,
                fill_top,
                fill_bot,
                if held { 2.2 } else { 1.2 },
                border_col,
                glow_r,
                glow_col,
                held,
            );

            // AppArt icon (56x56)
            let icon_s = 56.0 * scale;
            let ix = x + 24.0 * scale;
            let iy = cy - icon_s * 0.5;
            let icon_rad = icon_s * 0.28;

            r.draw_rounded_rect(
                ix,
                iy,
                icon_s,
                icon_s,
                icon_rad,
                [brand[0], brand[1], brand[2], 0.22 * tile_alpha],
                [brand[0], brand[1], brand[2], 0.16 * tile_alpha],
                0.0,
                [0.0; 4],
                0.0,
                [0.0; 4],
                false,
            );

            if let Some(tex) = r.preload_icon(&st.base_dir, &app.icon) {
                r.draw_texture(tex, ix, iy, icon_s, icon_s, icon_rad, [1.0, 1.0, 1.0, tile_alpha]);
            } else {
                let ch = app.title.chars().next().unwrap_or('A').to_string();
                r.draw_text(
                    &ch,
                    ix + icon_s * 0.5,
                    iy + icon_s * 0.5,
                    24,
                    true,
                    [1.0, 1.0, 1.0, 0.95 * tile_alpha],
                    0.5,
                    0.5,
                );
            }

            let lbl = truncate_title(&app.title, 28);
            r.draw_text(
                &lbl,
                ix + icon_s + 20.0 * scale,
                cy,
                24,
                true,
                [1.0, 1.0, 1.0, 0.95 * tile_alpha],
                0.0,
                0.5,
            );
        } else {
            // Horizontal card: w=248, h=320, step=276
            let base_w = 248.0;
            let base_h = 320.0;
            let cx = ox + anchor_px - scroll_px + (i as f32) * 276.0 + 124.0;
            let cy = oy + 540.0;
            if cx < -200.0 || cx > 2120.0 {
                continue;
            }
            let w = base_w * scale;
            let h = base_h * scale;
            let x = cx - w * 0.5;
            let y = cy - h * 0.5;

            let fill_top = if focused {
                [brand[0], brand[1], brand[2], 0.32 * tile_alpha]
            } else {
                [brand[0], brand[1], brand[2], 0.11 * tile_alpha]
            };
            let fill_bot = if focused {
                [brand[0], brand[1], brand[2], 0.09 * tile_alpha]
            } else {
                [1.0, 1.0, 1.0, 0.02 * tile_alpha]
            };
            let border_col = if focused {
                [brand[0], brand[1], brand[2], 0.95 * tile_alpha]
            } else {
                [brand[0], brand[1], brand[2], 0.18 * tile_alpha]
            };
            let glow_r = if focused { 22.0 } else { 0.0 };
            let glow_col = [brand[0], brand[1], brand[2], 0.48 * tile_alpha];

            r.draw_rounded_rect(
                x,
                y,
                w,
                h,
                28.0 * scale,
                fill_top,
                fill_bot,
                if held { 2.2 } else { 1.2 },
                border_col,
                glow_r,
                glow_col,
                held,
            );

            // AppArt icon (96x96)
            let icon_s = 96.0 * scale;
            let ix = cx - icon_s * 0.5;
            let iy = y + (h - 64.0 * scale - icon_s) * 0.5;
            let icon_rad = icon_s * 0.28;

            r.draw_rounded_rect(
                ix,
                iy,
                icon_s,
                icon_s,
                icon_rad,
                [brand[0], brand[1], brand[2], 0.22 * tile_alpha],
                [brand[0], brand[1], brand[2], 0.16 * tile_alpha],
                0.0,
                [0.0; 4],
                0.0,
                [0.0; 4],
                false,
            );

            if let Some(tex) = r.preload_icon(&st.base_dir, &app.icon) {
                r.draw_texture(tex, ix, iy, icon_s, icon_s, icon_rad, [1.0, 1.0, 1.0, tile_alpha]);
            } else {
                let ch = app.title.chars().next().unwrap_or('A').to_string();
                r.draw_text(
                    &ch,
                    ix + icon_s * 0.5,
                    iy + icon_s * 0.5,
                    40,
                    true,
                    [1.0, 1.0, 1.0, 0.95 * tile_alpha],
                    0.5,
                    0.5,
                );
            }

            let lbl = truncate_title(&app.title, 18);
            r.draw_text(
                &lbl,
                cx,
                y + h - 38.0 * scale,
                20,
                true,
                [1.0, 1.0, 1.0, 0.95 * tile_alpha],
                0.5,
                0.5,
            );
        }
    }

    // Reorder hint pill at bottom-[6vh]
    if active && st.moving {
        if let Some(app) = items.get(selected_idx) {
            let hint = if is_vert {
                format!(
                    "Moving \"{}\" — ↑ ↓ reorder · ← Media  → Misc · OK save · Back cancel",
                    app.title
                )
            } else {
                format!(
                    "Moving \"{}\" — ← → reorder · ↑ Gaming  ↓ Streaming · OK save · Back cancel",
                    app.title
                )
            };
            let pw = 760.0;
            let ph = 52.0;
            let px = ox + 960.0 - pw * 0.5;
            let py = oy + 1080.0 - 65.0 - ph;
            r.draw_rounded_rect(
                px,
                py,
                pw,
                ph,
                26.0,
                [0.08, 0.10, 0.16, 0.92],
                [0.05, 0.07, 0.12, 0.92],
                1.0,
                [accent[0], accent[1], accent[2], 0.45],
                0.0,
                [0.0; 4],
                false,
            );
            r.draw_text(
                &hint,
                ox + 960.0,
                py + ph * 0.5,
                18,
                true,
                [0.933, 0.945, 1.0, 0.95],
                0.5,
                0.5,
            );
        }
    }
}

fn draw_settings_modal(r: &mut GlRenderer, st: &AppState, now_ms: u32) {
    // Fullscreen dark scrim (rgba(0,0,0,0.74))
    r.draw_rounded_rect(
        0.0,
        0.0,
        1920.0,
        1080.0,
        0.0,
        [0.0, 0.0, 0.0, 0.74],
        [0.0, 0.0, 0.0, 0.74],
        0.0,
        [0.0; 4],
        0.0,
        [0.0; 4],
        false,
    );

    let mw = 760.0;
    let mh = 760.0;
    let mx = (1920.0 - mw) * 0.5;
    let my = (1080.0 - mh) * 0.5;
    let accent = [0.486, 0.659, 1.0]; // #7ca8ff

    r.draw_rounded_rect(
        mx,
        my,
        mw,
        mh,
        28.0,
        [0.09, 0.10, 0.15, 0.96],
        [0.05, 0.06, 0.09, 0.94],
        1.0,
        [0.70, 0.74, 0.92, 0.18],
        24.0,
        [0.0, 0.0, 0.0, 0.75],
        false,
    );

    let title = match st.settings_pane {
        SettingsPane::Main => "Settings",
        SettingsPane::ManageApps => "Manage apps",
    };
    r.draw_text(
        title,
        mx + 44.0,
        my + 32.0,
        34,
        false,
        [accent[0], accent[1], accent[2], 0.98],
        0.0,
        0.0,
    );

    if st.settings_pane == SettingsPane::ManageApps {
        let flat = st.all_manageable_apps();
        let visible_rows = 10usize;
        let start = st
            .apps_row
            .saturating_sub(visible_rows / 2)
            .min(flat.len().saturating_sub(visible_rows));
        let end = (start + visible_rows).min(flat.len());

        let mut ry = my + 96.0;
        for idx in start..end {
            let (scr, app) = &flat[idx];
            let focused = idx == st.apps_row;
            let is_hidden = st.hidden_set.contains(&app.id);
            if focused {
                r.draw_rounded_rect(
                    mx + 36.0,
                    ry,
                    mw - 72.0,
                    52.0,
                    16.0,
                    [accent[0], accent[1], accent[2], 0.14],
                    [accent[0], accent[1], accent[2], 0.10],
                    1.0,
                    [accent[0], accent[1], accent[2], 0.45],
                    0.0,
                    [0.0; 4],
                    false,
                );
            }
            let cat_col = parse_hex_rgb(scr.accent_hex());
            r.draw_text(
                scr.label(),
                mx + 56.0,
                ry + 26.0,
                14,
                true,
                [cat_col[0], cat_col[1], cat_col[2], 0.85],
                0.0,
                0.5,
            );
            let title_str = truncate_title(&app.title, 32);
            r.draw_text(
                &title_str,
                mx + 156.0,
                ry + 26.0,
                19,
                false,
                [1.0, 1.0, 1.0, if is_hidden { 0.35 } else { 0.92 }],
                0.0,
                0.5,
            );
            let status_txt = if is_hidden { "Hidden" } else { "Shown" };
            let status_col = if is_hidden {
                [1.0, 1.0, 1.0, 0.40]
            } else {
                [0.494, 0.906, 0.651, 0.95]
            };
            r.draw_text(
                status_txt,
                mx + mw - 56.0,
                ry + 26.0,
                17,
                true,
                status_col,
                1.0,
                0.5,
            );
            ry += 56.0;
        }
    } else {
        let rows: [(&str, String, bool); SETTINGS_ROWS] = [
            (
                "Clock format",
                if st.cfg.clock24 { "24-hour" } else { "12-hour" }.to_string(),
                true,
            ),
            (
                "Temperature",
                if st.cfg.temp_unit == "fahrenheit" {
                    "°F"
                } else {
                    "°C"
                }
                .to_string(),
                true,
            ),
            (
                "Background animation",
                if st.cfg.bg_animated {
                    "On (30 fps plexus)"
                } else {
                    "Off (0% GPU static)"
                }
                .to_string(),
                true,
            ),
            (
                "Card & Nudge animations",
                if st.cfg.card_animations {
                    "On (Spring physics)"
                } else {
                    "Off (Instant snap)"
                }
                .to_string(),
                true,
            ),
            (
                "Color theme",
                match st.cfg.bg_theme.as_str() {
                    "mono" => "Mono Blue",
                    "ember" => "Ember",
                    "aurora" => "Aurora",
                    _ => "Classic",
                }
                .to_string(),
                true,
            ),
            (
                "Star density",
                match st.cfg.bg_density.as_str() {
                    "low" => "Low",
                    "high" => "High",
                    _ => "Normal",
                }
                .to_string(),
                true,
            ),
            (
                "Motion speed",
                match st.cfg.bg_speed.as_str() {
                    "slow" => "Slow",
                    "fast" => "Fast",
                    _ => "Normal",
                }
                .to_string(),
                true,
            ),
            (
                "Typography",
                if st.cfg.font_family == "titillium" {
                    "Titillium Web"
                } else {
                    "LG Smart UI"
                }
                .to_string(),
                true,
            ),
            (
                "System Telemetry HUD",
                if st.cfg.show_telemetry {
                    "Shown"
                } else {
                    "Hidden"
                }
                .to_string(),
                true,
            ),
            ("Manage apps", "OK to open".to_string(), false),
            (
                "Reset app order",
                if now_ms < st.reset_done_ms {
                    "Done!"
                } else {
                    "OK to reset"
                }
                .to_string(),
                false,
            ),
        ];

        let mut ry = my + 90.0;
        for (i, (label, val, is_choice)) in rows.iter().enumerate() {
            let focused = i == st.settings_row;
            if focused {
                r.draw_rounded_rect(
                    mx + 36.0,
                    ry,
                    mw - 72.0,
                    48.0,
                    14.0,
                    [accent[0], accent[1], accent[2], 0.14],
                    [accent[0], accent[1], accent[2], 0.10],
                    1.0,
                    [accent[0], accent[1], accent[2], 0.45],
                    0.0,
                    [0.0; 4],
                    false,
                );
            }
            r.draw_text(
                label,
                mx + 56.0,
                ry + 24.0,
                19,
                false,
                [1.0, 1.0, 1.0, 0.88],
                0.0,
                0.5,
            );
            let display_val = if *is_choice {
                format!("‹  {}  ›", val)
            } else {
                val.clone()
            };
            let val_col = if *is_choice {
                [accent[0], accent[1], accent[2], 0.95]
            } else {
                [1.0, 1.0, 1.0, 0.55]
            };
            r.draw_text(
                &display_val,
                mx + mw - 56.0,
                ry + 24.0,
                18,
                true,
                val_col,
                1.0,
                0.5,
            );
            ry += 52.0;
        }
    }

    let footer = match st.settings_pane {
        SettingsPane::Main => {
            "↑↓ select · ‹ › change · OK action · Back to close   (v1.0.0 Native Rust)"
        }
        SettingsPane::ManageApps => "↑↓ select · OK show/hide · Back to settings",
    };
    r.draw_text(
        footer,
        mx + mw * 0.5,
        my + mh - 32.0,
        15,
        false,
        [1.0, 1.0, 1.0, 0.42],
        0.5,
        0.5,
    );
}

fn main() {
    set_env("EGL_PLATFORM", "wayland");
    set_env("XDG_RUNTIME_DIR", "/tmp/xdg");
    set_hint("SDL_WEBOS_ACCESS_POLICY_KEYS_BACK", "true");
    set_hint("SDL_WEBOS_ACCESS_POLICY_KEYS_EXIT", "true");
    // Law 7: Always allow the 4K Aerial Screensaver and LG OLED panel protection!
    set_hint("SDL_VIDEO_ALLOW_SCREENSAVER", "1");

    unsafe {
        if SDL_Init(SDL_INIT_VIDEO | SDL_INIT_EVENTS | SDL_INIT_TIMER) != 0 {
            return;
        }
        IMG_Init(IMG_INIT_PNG);
        TTF_Init();

        SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_ES);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 2);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 0);
        SDL_GL_SetAttribute(SDL_GL_RED_SIZE, 8);
        SDL_GL_SetAttribute(SDL_GL_GREEN_SIZE, 8);
        SDL_GL_SetAttribute(SDL_GL_BLUE_SIZE, 8);
        SDL_GL_SetAttribute(SDL_GL_ALPHA_SIZE, 8);
        SDL_GL_SetAttribute(SDL_GL_DOUBLEBUFFER, 1);
        SDL_GL_SetAttribute(SDL_GL_DEPTH_SIZE, 0);
        SDL_GL_SetAttribute(SDL_GL_MULTISAMPLEBUFFERS, 1);
        SDL_GL_SetAttribute(SDL_GL_MULTISAMPLESAMPLES, 4);

        let title = CString::new("Axis Home (Native)").unwrap();
        let mut window = SDL_CreateWindow(
            title.as_ptr() as *const c_char,
            SDL_WINDOWPOS_UNDEFINED,
            SDL_WINDOWPOS_UNDEFINED,
            1920,
            1080,
            SDL_WINDOW_OPENGL | SDL_WINDOW_FULLSCREEN | SDL_WINDOW_SHOWN,
        );
        if window.is_null() {
            SDL_GL_SetAttribute(SDL_GL_MULTISAMPLEBUFFERS, 0);
            SDL_GL_SetAttribute(SDL_GL_MULTISAMPLESAMPLES, 0);
            window = SDL_CreateWindow(
                title.as_ptr() as *const c_char,
                SDL_WINDOWPOS_UNDEFINED,
                SDL_WINDOWPOS_UNDEFINED,
                1920,
                1080,
                SDL_WINDOW_OPENGL | SDL_WINDOW_FULLSCREEN | SDL_WINDOW_SHOWN,
            );
            if window.is_null() {
                return;
            }
        }

        let gl_ctx = SDL_GL_CreateContext(window);
        if gl_ctx.is_null() {
            return;
        }
        gl_load_functions();
        SDL_GL_SetSwapInterval(1);

        let mut dw: i32 = 1920;
        let mut dh: i32 = 1080;
        SDL_GL_GetDrawableSize(window, &mut dw, &mut dh);
        glViewport(0, 0, dw, dh);

        let base_dir = app_base_dir();
        let cfg = load_config(&base_dir);
        let now_ms = SDL_GetTicks();

        let mut renderer = GlRenderer::new(&base_dir, 1920.0, 1080.0);
        renderer.set_font_family(&base_dir, &cfg.font_family);

        // Pre-warm fonts and all app icons into VRAM on startup
        for sz in [12, 13, 14, 15, 16, 17, 18, 19, 20, 24, 26, 34, 36, 40, 42, 46, 48, 64] {
            let _ = renderer.get_font(sz, false);
            let _ = renderer.get_font(sz, true);
        }
        for app in cfg
            .categories
            .up
            .iter()
            .chain(cfg.categories.down.iter())
            .chain(cfg.categories.left.iter())
            .chain(cfg.categories.right.iter())
        {
            if !app.icon.is_empty() {
                let _ = renderer.preload_icon(&base_dir, &app.icon);
            }
        }

        let mut state = AppState::new(base_dir, cfg, now_ms);
        let mut constellations = ConstellationsState::new();
        constellations.ensure_config(
            &state.cfg.bg_density,
            &state.cfg.bg_speed,
            &state.cfg.bg_theme,
        );

        let mut running = true;
        let mut window_visible = true;
        let mut force_redraw = true;
        let mut last_tick_ms = SDL_GetTicks();
        let mut last_clock_sec = local_time().4;
        let _ = std::fs::write("/tmp/axis_visible", "1");
        let mut last_home_seq = std::fs::read_to_string("/tmp/axis_home_seq").unwrap_or_default();
        let mut last_home_poll_ms: u32 = 0;

        while running {
            let loop_start_ms = SDL_GetTicks();

            // Check if physical Home button (773/987) wrote a new sequence token into /tmp/axis_home_seq.
            // Using a sequence comparison in memory requires READ-ONLY access and is 100% immune to /tmp sticky-bit EPERM.
            if loop_start_ms.saturating_sub(last_home_poll_ms) >= 35 {
                last_home_poll_ms = loop_start_ms;
                if let Ok(seq) = std::fs::read_to_string("/tmp/axis_home_seq") {
                    if !seq.is_empty() && seq != last_home_seq {
                        last_home_seq = seq;
                        state.reset_to_home(loop_start_ms);
                        force_redraw = true;
                    }
                }
                // If in OLED Blackout and tvpowerd transitioned from "Screen Off" -> "Active"
                // (e.g. first remote key or ThinQ app wakeup consumed by tvpowerd), wake UI immediately!
                if state.ambient && loop_start_ms.saturating_sub(state.blackout_entered_ms) >= 1800 {
                    if let Ok(soff) = std::fs::read_to_string("/tmp/oled_screen_off") {
                        if soff.trim() == "0" {
                            state.ambient = false;
                            state.ambient_fade.set(0.0, true);
                            state.last_input_ms = loop_start_ms;
                            force_redraw = true;
                        }
                    }
                }
            }

            // Check if discovered apps arrived from background thread
            if let Ok(mut d_lock) = state.discovered_rx.try_lock() {
                if let Some(extra) = d_lock.take() {
                    for a in extra {
                        state.cfg.categories.right.push(a);
                    }
                    force_redraw = true;
                }
            }

            // Decide whether to poll immediately (active animation) or sleep in kernel (0.0% CPU/GPU idle!)
            let active_anim = window_visible
                && (force_redraw
                    || state.needs_continuous_redraw(loop_start_ms, &constellations));

            let mut ev: SDL_Event = std::mem::zeroed();
            let has_first_event = if active_anim {
                SDL_PollEvent(&mut ev) != 0
            } else {
                // Sleep in kernel until an input event arrives or timeout
                let timeout = if !window_visible {
                    500
                } else if state.screen != ScreenId::Home || state.settings_open || state.moving {
                    45
                } else {
                    250
                };
                SDL_WaitEventTimeout(&mut ev, timeout) != 0
            };

            if has_first_event {
                loop {
                    let ev_now = SDL_GetTicks();
                    match ev.type_ {
                        SDL_QUIT => {
                            running = false;
                        }
                        SDL_WINDOWEVENT => {
                            let wev = ev.window.event;
                            if wev == SDL_WINDOWEVENT_HIDDEN || wev == SDL_WINDOWEVENT_MINIMIZED {
                                window_visible = false;
                                let _ = std::fs::write("/tmp/axis_visible", "0");
                            } else if wev == SDL_WINDOWEVENT_SHOWN
                                || wev == SDL_WINDOWEVENT_RESTORED
                            {
                                let was_hidden = !window_visible;
                                window_visible = true;
                                let _ = std::fs::write("/tmp/axis_visible", "1");
                                if was_hidden
                                    && ev_now.saturating_sub(state.blackout_entered_ms) >= 2500
                                {
                                    state.reset_to_home(ev_now);
                                }
                                if !state.ambient {
                                    force_redraw = true;
                                }
                            } else if wev == SDL_WINDOWEVENT_FOCUS_GAINED
                                || wev == SDL_WINDOWEVENT_EXPOSED
                            {
                                let was_hidden = !window_visible;
                                window_visible = true;
                                let _ = std::fs::write("/tmp/axis_visible", "1");
                                if was_hidden
                                    && ev_now.saturating_sub(state.blackout_entered_ms) >= 2500
                                {
                                    state.reset_to_home(ev_now);
                                }
                                if !state.ambient {
                                    force_redraw = true;
                                }
                            }
                        }
                        SDL_KEYDOWN => {
                            let sc = ev.key.keysym.scancode;
                            let sym = ev.key.keysym.sym;
                            let rep = ev.key.repeat != 0;
                            if sc == SDL_WEBOS_SCANCODE_GREEN {
                                // 485 = CURSOR_HIDE: stop Magic Remote pointer attraction immediately
                                constellations.last_move_ms = 0;
                            } else {
                                state.handle_key_down(sc, sym, rep, ev_now);
                                renderer.set_font_family(&state.base_dir, &state.cfg.font_family);
                                constellations.ensure_config(
                                    &state.cfg.bg_density,
                                    &state.cfg.bg_speed,
                                    &state.cfg.bg_theme,
                                );
                                force_redraw = true;
                            }
                        }
                        SDL_KEYUP => {
                            let sc = ev.key.keysym.scancode;
                            let sym = ev.key.keysym.sym;
                            if sc != SDL_WEBOS_SCANCODE_GREEN && !state.ambient {
                                state.handle_key_up(sc, sym, ev_now);
                                force_redraw = true;
                            }
                        }
                        SDL_MOUSEMOTION => {
                            if ev.motion.xrel != 0 || ev.motion.yrel != 0 {
                                let mx = ev.motion.x as f32;
                                let my = ev.motion.y as f32;
                                state.wake_from_blackout(ev_now);
                                constellations.on_pointer_move(mx, my, ev_now);
                                force_redraw = true;
                            }
                        }
                        SDL_MOUSEBUTTONDOWN => {
                            let mx = ev.button.x as f32;
                            let my = ev.button.y as f32;
                            state.handle_mouse_click(mx, my, ev_now);
                            force_redraw = true;
                        }
                        SDL_MOUSEWHEEL => {
                            let wy = ev.wheel.y;
                            if state.ambient {
                                state.wake_from_blackout(ev_now);
                                force_redraw = true;
                            } else {
                                state.last_input_ms = ev_now;
                                if state.screen != ScreenId::Home && !state.settings_open {
                                    let count = state.visible_for(state.screen).len();
                                    if wy < 0 && state.selected + 1 < count {
                                        state.selected += 1;
                                        state.update_scroll_target(state.screen, state.selected);
                                        force_redraw = true;
                                    } else if wy > 0 && state.selected > 0 {
                                        state.selected -= 1;
                                        state.update_scroll_target(state.screen, state.selected);
                                        force_redraw = true;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    if SDL_PollEvent(&mut ev) == 0 {
                        break;
                    }
                }
            }

            if !window_visible {
                continue;
            }

            let now_ms = SDL_GetTicks();
            let cur_sec = local_time().4;
            if cur_sec != last_clock_sec {
                last_clock_sec = cur_sec;
                if state.screen == ScreenId::Home && !state.ambient {
                    force_redraw = true;
                }
            }

            // Long-press OK check (550ms -> enter reorder mode)
            if state.ok_down_ms > 0
                && !state.ok_held_fired
                && now_ms.saturating_sub(state.ok_down_ms) >= 550
            {
                state.ok_held_fired = true;
                if state.screen != ScreenId::Home {
                    state.move_snapshot = Some(state.cfg.categories.clone());
                    state.moving = true;
                    force_redraw = true;
                }
            }

            // Expire toast
            if state.toast_msg.is_some() && now_ms >= state.toast_until_ms {
                state.toast_msg = None;
                if !state.ambient {
                    force_redraw = true;
                }
            }

            // 15s Static Screen -> Full OLED Blackout (0.000 nits + turnOffScreen)
            if !state.ambient && now_ms.saturating_sub(state.last_input_ms) >= 15_000 {
                state.enter_blackout(now_ms);
                force_redraw = true;
            }

            // OLED Anti-Burn-In ±2px Jitter every 150s (only while active)
            if !state.ambient && now_ms.saturating_sub(state.last_jitter_ms) >= 150_000 {
                state.last_jitter_ms = now_ms;
                let jx = ((now_ms % 5) as f32) - 2.0;
                let jy = (((now_ms / 7) % 5) as f32) - 2.0;
                state.jitter_x = jx;
                state.jitter_y = jy;
                force_redraw = true;
            }

            // Update RSS memory telemetry every 2s (only while active)
            if !state.ambient && now_ms.saturating_sub(state.last_rss_ms) >= 2000 {
                state.last_rss_ms = now_ms;
                state.rss_mb = read_rss_mb();
            }

            let should_draw =
                force_redraw || state.needs_continuous_redraw(now_ms, &constellations);
            if !should_draw {
                last_tick_ms = now_ms;
                continue;
            }
            force_redraw = false;

            if state.ambient {
                // 100% Pure Blackout (#000000 = 0.000 nits on OLED) — zero overlays, zero clocks, zero animations
                glClearColor(0.0, 0.0, 0.0, 1.0);
                glClear(GL_COLOR_BUFFER_BIT);
                SDL_GL_SwapWindow(window);
                last_tick_ms = now_ms;
                continue;
            }

            let dt_ms = now_ms.saturating_sub(last_tick_ms).max(1) as f32;
            last_tick_ms = now_ms;
            let dt_sec = (dt_ms / 1000.0).min(0.05);
            let dt_frames = (dt_ms / 16.667).min(2.2);

            // Step spring physics
            let stiff = 215.0;
            let damp = 26.5;
            state.world_x.step(dt_sec, stiff, damp);
            state.world_y.step(dt_sec, stiff, damp);
            state.scroll_up.step(dt_sec, stiff, damp);
            state.scroll_down.step(dt_sec, stiff, damp);
            state.scroll_left.step(dt_sec, stiff, damp);
            state.scroll_right.step(dt_sec, stiff, damp);
            state.bg_fade.step(dt_sec, 140.0, 22.0);
            state.ambient_fade.step(dt_sec, 120.0, 20.0);

            // Render Frame
            glClearColor(0.015, 0.015, 0.02, 1.0);
            glClear(GL_COLOR_BUFFER_BIT);

            // 1. Background Constellations (fades out gently when panning away from Home)
            let const_alpha = state.bg_fade.val.clamp(0.0, 1.0);
            constellations.step_and_draw(
                &mut renderer,
                now_ms,
                dt_frames,
                state.cfg.bg_animated && state.screen == ScreenId::Home,
                const_alpha,
            );

            // 2. World Cross of 5 Panels
            let ui_alpha = 1.0;
            let wx = state.world_x.val + state.jitter_x;
            let wy = state.world_y.val + state.jitter_y;

            draw_home_panel(&mut renderer, &state, wx, wy, now_ms, ui_alpha);
            for scr in [
                ScreenId::Up,
                ScreenId::Down,
                ScreenId::Left,
                ScreenId::Right,
            ] {
                let (px, py) = scr.panel_offset();
                draw_alt_panel(&mut renderer, &state, scr, wx + px, wy + py, now_ms, ui_alpha);
            }

            // 4. Settings Modal
            if state.settings_open {
                draw_settings_modal(&mut renderer, &state, now_ms);
            }

            // 5. Toast Notification Pill
            if let Some(ref msg) = state.toast_msg {
                let tw = 440.0;
                let th = 50.0;
                let tx = 960.0 - tw * 0.5;
                let ty = 64.0;
                renderer.draw_rounded_rect(
                    tx,
                    ty,
                    tw,
                    th,
                    25.0,
                    [0.08, 0.10, 0.16, 0.94],
                    [0.05, 0.07, 0.12, 0.94],
                    1.0,
                    [0.486, 0.659, 1.0, 0.45],
                    0.0,
                    [0.0; 4],
                    false,
                );
                renderer.draw_text(
                    msg,
                    960.0,
                    ty + th * 0.5,
                    18,
                    true,
                    [0.933, 0.945, 1.0, 0.96],
                    0.5,
                    0.5,
                );
            }

            SDL_GL_SwapWindow(window);

            state.frame_counter += 1;
            if now_ms.saturating_sub(state.fps_timer_ms) >= 1000 {
                state.fps_display = state.frame_counter.max(60);
                state.frame_counter = 0;
                state.fps_timer_ms = now_ms;
            }
        }

        SDL_GL_DeleteContext(gl_ctx);
        SDL_DestroyWindow(window);
        TTF_Quit();
        IMG_Quit();
        SDL_Quit();
    }
}
