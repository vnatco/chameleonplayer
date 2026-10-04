//! Windows shell surfaces (spec E): the tray icon tinted per track, the
//! taskbar thumbnail (the cover itself, never a shrunken transparent window)
//! with previous / play-pause / next buttons, and the no-cover image for the
//! media overlay.

use crate::audio::{AudioHandle, Command};
use base64::Engine;
use image::{imageops::FilterType, RgbaImage};
use parking_lot::Mutex;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

type Res<T> = Result<T, String>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellUpdate {
    pub path: Option<String>,
    pub title: String,
    pub artist: String,
    pub accent: [u8; 3],
    pub playing: bool,
    /// Cached cover image file, if the track has art.
    pub cover: Option<String>,
    /// PNG of the typographic no-cover sleeve (base64), when there is no art.
    pub fallback_png: Option<String>,
    /// Where the cover sits in the window (logical px), for the taskbar peek.
    pub cover_rect: crate::window::Rect,
}

#[derive(Default)]
struct State {
    cover_key: Option<String>,
    /// Square cover art, ~600 px, for taskbar thumbnails.
    art: Option<Arc<RgbaImage>>,
    playing: bool,
    accent: [u8; 3],
    cover_rect: crate::window::Rect,
}

#[derive(Clone)]
pub struct Shell {
    state: Arc<Mutex<State>>,
    app: AppHandle,
    fallback_dir: PathBuf,
}

impl Shell {
    pub fn new(app: &AppHandle, audio: AudioHandle, data_dir: &Path) -> Res<Self> {
        let shell = Shell { state: Arc::default(), app: app.clone(), fallback_dir: data_dir.join("covers").join("fallback") };
        shell.build_tray(audio.clone())?;
        #[cfg(windows)]
        if let Some(window) = app.get_webview_window("main") {
            taskbar::install(&window, audio, shell.state.clone());
        }
        Ok(shell)
    }

    fn build_tray(&self, audio: AudioHandle) -> Res<()> {
        let app = &self.app;
        let item = |id: &str, text: &str| MenuItem::with_id(app, id, text, true, None::<&str>).map_err(|e| e.to_string());
        let menu = Menu::with_items(
            app,
            &[
                &item("toggle", "Play / Pause")?,
                &item("next", "Next")?,
                &item("prev", "Previous")?,
                &PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?,
                &item("show", "Show Chameleon")?,
                &item("quit", "Quit")?,
            ],
        )
        .map_err(|e| e.to_string())?;
        let audio_menu = audio.clone();
        TrayIconBuilder::with_id("main")
            .icon(tray_icon([150, 150, 162]))
            .tooltip("Chameleon Player")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(move |app, e| match e.id().as_ref() {
                "toggle" => audio_menu.send(Command::Toggle),
                "next" => audio_menu.send(Command::Next),
                "prev" => audio_menu.send(Command::Prev),
                "show" => show(app),
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, e| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                    show(tray.app_handle());
                }
            })
            .build(app)
            .map_err(|e| format!("Can't create tray icon: {e}"))?;
        Ok(())
    }

    /// Called by the UI on every track or play-state change.
    pub fn update(&self, u: ShellUpdate) -> Res<Option<PathBuf>> {
        // Tray: icon in the accent, tooltip with the track.
        if let Some(tray) = self.app.tray_by_id("main") {
            let changed = {
                let s = self.state.lock();
                s.accent != u.accent
            };
            if changed {
                tray.set_icon(Some(tray_icon(u.accent))).map_err(|e| e.to_string())?;
            }
            let tip = if u.title.is_empty() { "Chameleon Player".to_string() } else if u.artist.is_empty() { u.title.clone() } else { format!("{} - {}", u.title, u.artist) };
            // Windows limits tray tooltips to 127 characters.
            let tip: String = tip.chars().take(120).collect();
            tray.set_tooltip(Some(tip)).map_err(|e| e.to_string())?;
        }

        // Taskbar art: decode once per cover.
        let key = u.cover.clone().or_else(|| u.fallback_png.as_ref().map(|p| format!("fallback:{}", crate::library::covers::hash(p.as_bytes()))));
        let art = {
            let s = self.state.lock();
            if s.cover_key == key { s.art.clone() } else { None }
        };
        let art = match (art, &u.cover, &u.fallback_png) {
            (Some(a), _, _) => Some(a),
            (None, Some(path), _) => image::open(path).ok().map(|i| Arc::new(i.resize_to_fill(600, 600, FilterType::Triangle).to_rgba8())),
            (None, None, Some(png)) => decode_png(png).map(|i| Arc::new(i.to_rgba8())),
            _ => None,
        };

        // Media overlay: never blank, so a no-cover track gets the fallback PNG.
        let mut fallback_file = None;
        if u.cover.is_none() {
            if let Some(png) = &u.fallback_png {
                let bytes = base64::engine::general_purpose::STANDARD.decode(png).map_err(|e| e.to_string())?;
                std::fs::create_dir_all(&self.fallback_dir).map_err(|e| e.to_string())?;
                let file = self.fallback_dir.join(format!("{}.png", crate::library::covers::hash(&bytes)));
                if !file.exists() {
                    crate::library::covers::write_atomic(&file, &bytes)?;
                }
                fallback_file = Some(file);
            }
        }

        let playing_changed;
        {
            let mut s = self.state.lock();
            playing_changed = s.playing != u.playing;
            s.cover_key = key;
            s.art = art;
            s.playing = u.playing;
            s.accent = u.accent;
            s.cover_rect = u.cover_rect;
        }
        #[cfg(windows)]
        if let Some(window) = self.app.get_webview_window("main") {
            taskbar::refresh(&window, &self.state, playing_changed);
        }
        let _ = playing_changed;
        Ok(fallback_file)
    }

    /// Delete fallback images from earlier runs.
    pub fn clear_fallbacks(dir: &Path) {
        let d = dir.join("covers").join("fallback");
        if d.exists() {
            if let Err(e) = std::fs::remove_dir_all(&d) {
                log::warn!("can't clear fallback covers: {e}");
            }
        }
    }
}

fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn decode_png(b64: &str) -> Option<image::DynamicImage> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    image::load_from_memory(&bytes).ok()
}

// ---- Drawing ---------------------------------------------------------------

/// The tray mark (spec E): a framed square (the sleeve) filled with the
/// accent, with a 1 px keyline when the accent is under 3:1 against the
/// taskbar.
fn tray_icon(accent: [u8; 3]) -> tauri::image::Image<'static> {
    const N: u32 = 32;
    let taskbar: [u8; 3] = if light_taskbar() { [243, 243, 243] } else { [32, 32, 36] };
    let keyline = crate::palette::color::contrast(accent, taskbar) < 3.0;
    let line: [u8; 3] = if light_taskbar() { [17, 17, 17] } else { [255, 255, 255] };
    let mut img = RgbaImage::new(N, N);
    for y in 0..N {
        for x in 0..N {
            let d_edge = x.min(y).min(N - 1 - x).min(N - 1 - y);
            let inner = (11..21).contains(&x) && (11..21).contains(&y);
            let px = if keyline && d_edge == 0 {
                Some((line, 140))
            } else if (1..=4).contains(&d_edge) || inner {
                Some((accent, 255))
            } else {
                None
            };
            if let Some((c, a)) = px {
                img.put_pixel(x, y, image::Rgba([c[0], c[1], c[2], a]));
            }
        }
    }
    tauri::image::Image::new_owned(img.into_raw(), N, N)
}

#[cfg(windows)]
fn light_taskbar() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut v: u32 = 0;
    let mut len = std::mem::size_of::<u32>() as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as *mut _),
            Some(&mut len),
        )
    };
    r.is_ok() && v == 1
}

#[cfg(not(windows))]
fn light_taskbar() -> bool {
    false
}

/// 16 px white transport glyphs, 4x supersampled.
#[derive(Clone, Copy)]
pub enum Glyph {
    Prev,
    Play,
    Pause,
    Next,
}

pub fn glyph(g: Glyph) -> RgbaImage {
    const N: u32 = 16;
    const SS: u32 = 4;
    let inside = |x: f32, y: f32| -> bool {
        // Coordinates in a 24-unit box, like the UI icons.
        let tri_right = |x: f32, y: f32, x0: f32, x1: f32| x >= x0 && x <= x1 && (y - 12.0).abs() <= (x1 - x) * 6.0 / (x1 - x0);
        let tri_left = |x: f32, y: f32, x0: f32, x1: f32| x >= x0 && x <= x1 && (y - 12.0).abs() <= (x - x0) * 6.0 / (x1 - x0);
        match g {
            Glyph::Play => (7.5..=19.0).contains(&x) && (y - 12.0).abs() <= (19.0 - x) * 7.0 / 11.5,
            Glyph::Pause => (5.0..=19.0).contains(&y) && ((6.5..=10.1).contains(&x) || (13.9..=17.5).contains(&x)),
            Glyph::Prev => ((5.0..=7.0).contains(&x) && (6.0..=18.0).contains(&y)) || tri_left(x, y, 8.0, 18.0),
            Glyph::Next => ((17.0..=19.0).contains(&x) && (6.0..=18.0).contains(&y)) || tri_right(x, y, 6.0, 16.0),
        }
    };
    let mut img = RgbaImage::new(N, N);
    for py in 0..N {
        for px in 0..N {
            let mut hits = 0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) * 24.0 / N as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) * 24.0 / N as f32;
                    if inside(x, y) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                img.put_pixel(px, py, image::Rgba([255, 255, 255, (hits * 255 / (SS * SS)) as u8]));
            }
        }
    }
    img
}

// ---- Taskbar (Windows) ---------------------------------------------------------

#[cfg(windows)]
mod taskbar {
    use super::{glyph, Glyph, State};
    use crate::audio::{AudioHandle, Command};
    use image::RgbaImage;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use tauri::WebviewWindow;
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Dwm::{
        DwmInvalidateIconicBitmaps, DwmSetIconicLivePreviewBitmap, DwmSetIconicThumbnail, DwmSetWindowAttribute, DWMWA_FORCE_ICONIC_REPRESENTATION, DWMWA_HAS_ICONIC_BITMAP,
    };
    use windows::Win32::Graphics::Gdi::{CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{DefSubclassProc, ITaskbarList3, SetWindowSubclass, TaskbarList, THBF_ENABLED, THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, RegisterWindowMessageW, HICON, ICONINFO, WM_COMMAND};

    const WM_DWMSENDICONICTHUMBNAIL: u32 = 0x0323;
    const WM_DWMSENDICONICLIVEPREVIEWBITMAP: u32 = 0x0326;
    const ID_PREV: u32 = 1;
    const ID_PLAY: u32 = 2;
    const ID_NEXT: u32 = 3;

    struct Ctx {
        audio: AudioHandle,
        state: Arc<Mutex<State>>,
        taskbar_created: u32,
        list: Mutex<Option<ITaskbarList3>>,
        icons: [HICON; 4],
        scale: Mutex<f64>,
    }
    // HICONs and the COM pointer are only touched on the window's thread.
    unsafe impl Send for Ctx {}
    unsafe impl Sync for Ctx {}

    static CTX: std::sync::OnceLock<Ctx> = std::sync::OnceLock::new();

    pub fn install(window: &WebviewWindow, audio: AudioHandle, state: Arc<Mutex<State>>) {
        let w = window.clone();
        let res = window.run_on_main_thread(move || unsafe {
            let Ok(h) = w.hwnd() else { return };
            let hwnd = HWND(h.0 as _);
            let icons = [Glyph::Prev, Glyph::Play, Glyph::Pause, Glyph::Next].map(|g| icon(&glyph(g)).unwrap_or_default());
            let ctx = Ctx {
                audio,
                state,
                taskbar_created: RegisterWindowMessageW(w!("TaskbarButtonCreated")),
                list: Mutex::new(None),
                icons,
                scale: Mutex::new(w.scale_factor().unwrap_or(1.0)),
            };
            if CTX.set(ctx).is_err() {
                return;
            }
            let on: windows::core::BOOL = true.into();
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_FORCE_ICONIC_REPRESENTATION, &on as *const _ as _, 4);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_HAS_ICONIC_BITMAP, &on as *const _ as _, 4);
            if !SetWindowSubclass(hwnd, Some(proc), 0xC4A1, 0).as_bool() {
                log::warn!("taskbar: can't subclass window; no thumbnail buttons");
            }
        });
        if let Err(e) = res {
            log::warn!("taskbar setup failed: {e}");
        }
    }

    /// Repaint the thumbnail after a cover change; swap play/pause.
    pub fn refresh(window: &WebviewWindow, _state: &Arc<Mutex<State>>, playing_changed: bool) {
        let w = window.clone();
        let _ = window.run_on_main_thread(move || unsafe {
            let Ok(h) = w.hwnd() else { return };
            let hwnd = HWND(h.0 as _);
            if let Some(ctx) = CTX.get() {
                *ctx.scale.lock() = w.scale_factor().unwrap_or(1.0);
                if playing_changed {
                    update_buttons(ctx, hwnd);
                }
            }
            let _ = DwmInvalidateIconicBitmaps(hwnd);
        });
    }

    unsafe fn add_buttons(ctx: &Ctx, hwnd: HWND) {
        let list: ITaskbarList3 = match CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) {
            Ok(l) => l,
            Err(e) => {
                log::warn!("taskbar: no ITaskbarList3: {e}");
                return;
            }
        };
        if let Err(e) = list.HrInit() {
            log::warn!("taskbar: init failed: {e}");
            return;
        }
        if let Err(e) = list.ThumbBarAddButtons(hwnd, &buttons(ctx)) {
            log::warn!("taskbar: can't add buttons: {e}");
            return;
        }
        *ctx.list.lock() = Some(list);
    }

    unsafe fn update_buttons(ctx: &Ctx, hwnd: HWND) {
        if let Some(list) = ctx.list.lock().as_ref() {
            if let Err(e) = list.ThumbBarUpdateButtons(hwnd, &buttons(ctx)) {
                log::warn!("taskbar: can't update buttons: {e}");
            }
        }
    }

    fn buttons(ctx: &Ctx) -> [THUMBBUTTON; 3] {
        let playing = ctx.state.lock().playing;
        let b = |id: u32, icon: HICON, tip: &str| {
            let mut sz = [0u16; 260];
            for (i, c) in tip.encode_utf16().take(259).enumerate() {
                sz[i] = c;
            }
            THUMBBUTTON { dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS, iId: id, iBitmap: 0, hIcon: icon, szTip: sz, dwFlags: THBF_ENABLED }
        };
        [
            b(ID_PREV, ctx.icons[0], "Previous"),
            if playing { b(ID_PLAY, ctx.icons[2], "Pause") } else { b(ID_PLAY, ctx.icons[1], "Play") },
            b(ID_NEXT, ctx.icons[3], "Next"),
        ]
    }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
        if let Some(ctx) = CTX.get() {
            if msg == ctx.taskbar_created {
                add_buttons(ctx, hwnd);
            } else if msg == WM_COMMAND && ((wp.0 >> 16) & 0xffff) as u32 == THBN_CLICKED {
                match (wp.0 & 0xffff) as u32 {
                    ID_PREV => ctx.audio.send(Command::Prev),
                    ID_PLAY => ctx.audio.send(Command::Toggle),
                    ID_NEXT => ctx.audio.send(Command::Next),
                    _ => {}
                }
                return LRESULT(0);
            } else if msg == WM_DWMSENDICONICTHUMBNAIL {
                let (mw, mh) = (((lp.0 >> 16) & 0xffff) as u32, (lp.0 & 0xffff) as u32);
                let art = ctx.state.lock().art.clone();
                if let Some(art) = art {
                    let side = mw.min(mh).max(1);
                    let img = image::imageops::resize(art.as_ref(), side, side, image::imageops::FilterType::Triangle);
                    if let Some(bmp) = dib(&img) {
                        let _ = DwmSetIconicThumbnail(hwnd, bmp, 0);
                        let _ = DeleteObject(bmp.into());
                    }
                }
                return LRESULT(0);
            } else if msg == WM_DWMSENDICONICLIVEPREVIEWBITMAP {
                // Peek: the cover where it sits on screen, nothing else.
                let (art, rect) = {
                    let s = ctx.state.lock();
                    (s.art.clone(), s.cover_rect)
                };
                if let (Some(art), true) = (art, rect.w > 0.0) {
                    let scale = *ctx.scale.lock();
                    let side = (rect.w * scale).round().max(1.0) as u32;
                    let img = image::imageops::resize(art.as_ref(), side, side, image::imageops::FilterType::Triangle);
                    if let Some(bmp) = dib(&img) {
                        let pt = POINT { x: (rect.x * scale).round() as i32, y: (rect.y * scale).round() as i32 };
                        let _ = DwmSetIconicLivePreviewBitmap(hwnd, bmp, Some(&pt), 0);
                        let _ = DeleteObject(bmp.into());
                    }
                }
                return LRESULT(0);
            }
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }

    /// Top-down 32-bit premultiplied BGRA DIB section.
    unsafe fn dib(img: &RgbaImage) -> Option<HBITMAP> {
        let (w, h) = img.dimensions();
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
        for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(img.pixels()) {
            let a = s[3] as u32;
            d[0] = (s[2] as u32 * a / 255) as u8;
            d[1] = (s[1] as u32 * a / 255) as u8;
            d[2] = (s[0] as u32 * a / 255) as u8;
            d[3] = s[3];
        }
        Some(bmp)
    }

    unsafe fn icon(img: &RgbaImage) -> Option<HICON> {
        let color = dib_straight(img)?;
        let (w, h) = img.dimensions();
        let mask = CreateBitmap(w as i32, h as i32, 1, 1, None);
        let info = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }

    /// Icons want straight (not premultiplied) alpha.
    unsafe fn dib_straight(img: &RgbaImage) -> Option<HBITMAP> {
        let (w, h) = img.dimensions();
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
        for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(img.pixels()) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
        Some(bmp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_have_ink_and_are_distinct() {
        let ink = |g| glyph(g).pixels().filter(|p| p[3] > 0).count();
        for g in [Glyph::Prev, Glyph::Play, Glyph::Pause, Glyph::Next] {
            let n = ink(g);
            assert!((20..200).contains(&n), "glyph ink {n}");
        }
        assert_ne!(glyph(Glyph::Play).into_raw(), glyph(Glyph::Pause).into_raw());
    }

    #[test]
    fn tray_icon_is_square_32() {
        let i = tray_icon([214, 28, 40]);
        assert_eq!((i.width(), i.height()), (32, 32));
    }
}
