// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! W4: the Windows shell.
//!
//! The pet's decisions are not here. `roamling-core::PetRuntime` makes all of
//! them and this drives it -- reads the machine, hands it a `TickInput`, and
//! puts the result on screen. The macOS side reaches the same runtime through
//! uniffi; this side links it directly and pays no FFI.
//!
//! The window itself came out of the W0 spike (`output/w0/rust-overlay`), which
//! established on real hardware that Windows can do the seven things the
//! overlay needs. See `docs/windows.md`, section 9.

mod platform;
mod sprite;

use roamling_core::{
    look_frame_index, AnimationResolver, BehaviorState, DisplaySnapshot, InteractionOutput,
    PetAnimationPlayer, PetRuntime, RuntimeTuning, TickInput, WorldPoint, WorldSize,
};
use roamling_pet::PetAsset;
use sprite::Surface;
use std::cell::RefCell;
use std::time::Instant;
use windows::core::{w, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;

/// The pet's footprint in world units, matching `PetOverlayPanel.baseSize`.
/// The sheet's cells are twice this -- it is a 2x asset.
const BASE_WIDTH: f64 = 96.0;
const BASE_HEIGHT: f64 = 104.0;

struct App {
    pet: PetRuntime,
    asset: PetAsset,
    resolver: AnimationResolver,
    player: PetAnimationPlayer,
    /// Cached, because the tick needs the scale under the pet and enumerating
    /// monitors sixty times a second to learn it would be absurd.
    displays: Vec<DisplaySnapshot>,
    started: Instant,
    surface: Option<Surface>,
    /// Frame and scale currently on screen, so an unchanged tick redraws none.
    drawn: Option<(usize, f64)>,
    /// Set while the user is holding the pet, so drags can report a distance.
    drag_from: Option<POINT>,
    dragged: bool,
    /// What `SetTimer` is currently armed at, so it is only re-armed on change.
    tick_ms: u32,
    /// Last reported state, so the log says what changed rather than repeating.
    last_state: BehaviorState,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn main() -> Result<()> {
    // Before any window exists, or the process is stuck with the wrong answer.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };

    let Some(asset) = roamling_pet::built_in_mochi() else {
        eprintln!("the built-in mascot did not decode");
        return Ok(());
    };
    let displays = platform::displays();
    if displays.is_empty() {
        eprintln!("no displays; nothing to roam");
        return Ok(());
    }
    println!("{} display(s):", displays.len());
    for display in &displays {
        println!(
            "  {} {}x{} at ({},{})  scale {:.2}",
            display.name,
            display.frame.size.width,
            display.frame.size.height,
            display.frame.origin.x,
            display.frame.origin.y,
            display.scale
        );
    }

    let first = displays[0].visible_frame;
    let start = WorldPoint::new(
        first.origin.x + first.size.width / 2.0,
        first.origin.y + first.size.height / 2.0,
    );
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);

    let mut pet = PetRuntime::new(start, RuntimeTuning::default(), seed);
    pet.set_displays(displays.clone());
    pet.set_object_size(WorldSize::new(BASE_WIDTH, BASE_HEIGHT));

    let resolver = AnimationResolver::new(asset.tracks.clone(), asset.behavior_mappings.clone());
    let player = PetAnimationPlayer::new(&resolver);

    let hwnd = create_window()?;
    APP.with(|slot| {
        *slot.borrow_mut() = Some(App {
            pet,
            asset,
            resolver,
            player,
            displays,
            started: Instant::now(),
            surface: None,
            drawn: None,
            drag_from: None,
            dragged: false,
            tick_ms: 0,
            last_state: BehaviorState::Idle,
        })
    });

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        // The pet must not make its own seat look busy once capture arrives.
        // That also makes it invisible to a screenshot, so there is a way out:
        // without it there is no way to check by eye what was actually drawn.
        if std::env::var_os("ROAMLING_ALLOW_CAPTURE").is_none() {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        }
        SetTimer(hwnd, 1, 16, None);
    }
    println!("\nroaming. close the window to stop.");

    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

fn create_window() -> Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None)? };
    let class = w!("RoamlingOverlay");
    let mut wc = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        lpszClassName: class,
        ..Default::default()
    };
    // Without this the cursor turns into the busy pointer whenever the window
    // owns the click -- the pet looks like it has hung. Found by eye in W0.
    wc.hCursor = unsafe { LoadCursorW(None, IDC_ARROW)? };
    if unsafe { RegisterClassW(&wc) } == 0 {
        eprintln!("RegisterClassW failed");
    }

    // TRANSPARENT starts on: the pet is scenery until the runtime says it can
    // be grabbed. TOOLWINDOW keeps it off the taskbar, NOACTIVATE keeps it from
    // stealing focus from whatever the user is typing into.
    let ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT;
    unsafe {
        CreateWindowExW(
            ex,
            class,
            w!("Roamling"),
            WS_POPUP,
            0,
            0,
            BASE_WIDTH as i32,
            BASE_HEIGHT as i32,
            None,
            None,
            instance,
            None,
        )
    }
}

/// One turn of the loop: read the machine, step the core, show the answer.
fn tick(hwnd: HWND, app: &mut App) {
    let now = app.started.elapsed().as_secs_f64();

    // The core decides whether an accessibility round trip is worth paying
    // for. MVP 0 has no focus provider, so the answer is thrown away.
    let _wants_focus = app.pet.begin_tick(now);

    let pointer = platform::pointer();
    let position = app.pet.position();
    let scale = scale_under(app, position);
    let half_width = BASE_WIDTH * scale / 2.0;
    let half_height = BASE_HEIGHT * scale / 2.0;

    let output = app.pet.finish_tick(&TickInput {
        now,
        pointer,
        primary_button_down: platform::primary_button_down(),
        user_idle_duration: platform::user_idle_duration(),
        // Neither provider exists yet; the core stays on the MVP 3 path.
        capture_authorized: false,
        focus_authorized: false,
        did_query_focus: false,
        queried_focus: None,
        // Only the shell knows how big the pet is once the panel has scaled it.
        pointer_is_over_pet: (pointer.x - position.x).abs() <= half_width
            && (pointer.y - position.y).abs() <= half_height,
    });

    if output.state != app.last_state {
        println!(
            "{:7.1}s  {:?} -> {:?}   {:?}   at ({:.0}, {:.0})",
            now,
            app.last_state,
            output.state,
            output.capability,
            output.position.x,
            output.position.y
        );
        app.last_state = output.state;
    }

    app.player.set_capability(&app.resolver, output.capability);
    // A sheet with fewer than eleven rows has no directional look row, and
    // mochi-v3 has nine -- its `gaze` track does the watching instead.
    let look = output
        .look_direction_degrees
        .and_then(|degrees| look_frame_index(degrees, app.asset.columns, app.asset.rows));
    app.player.set_look_frame(look);
    app.player.update(output.delta_time * output.locomotion_rate);

    set_click_through(hwnd, !output.interaction_enabled);
    draw(hwnd, app, output.position, scale);

    // Let the tick fall back when the pet is resting. This is most of what the
    // app costs when nothing is happening -- see docs/battery.md.
    let wanted = (app.pet.preferred_tick_interval(now) * 1000.0).round() as u32;
    if wanted != app.tick_ms && wanted > 0 {
        app.tick_ms = wanted;
        unsafe { SetTimer(hwnd, 1, wanted, None) };
    }
}

/// The scale of whichever display the pet is standing on, so it stays the same
/// physical size as it crosses a mixed-DPI seam.
fn scale_under(app: &App, position: WorldPoint) -> f64 {
    let mut fallback = 1.0;
    for display in &app.displays {
        if display.frame.contains(position) {
            return display.scale;
        }
        fallback = display.scale;
    }
    fallback
}

fn draw(hwnd: HWND, app: &mut App, position: WorldPoint, scale: f64) {
    let width = (BASE_WIDTH * scale).round() as i32;
    let height = (BASE_HEIGHT * scale).round() as i32;
    let resize = app
        .surface
        .as_ref()
        .map_or(true, |s| s.width != width || s.height != height);
    if resize {
        app.surface = Surface::new(width, height);
        app.drawn = None;
        // The footprint changed, so where the pet may stand changed with it.
        app.pet
            .set_scale(WorldSize::new(BASE_WIDTH * scale, BASE_HEIGHT * scale));
    }

    let frame = app.player.current_frame_index();
    let Some(surface) = app.surface.as_mut() else {
        return;
    };
    // Only touch pixels when the frame actually changed. Most ticks do not
    // change one -- the walk cycle advances every eighth of a second.
    if app.drawn != Some((frame, scale)) {
        if let Some(rect) = app.asset.frame_rect(frame) {
            if let Some(sheet) = app.asset.sheet(rect.sheet) {
                surface.draw_frame(sheet, rect);
                app.drawn = Some((frame, scale));
            }
        }
    }

    // The runtime reports a centre; the window wants a top-left corner.
    let corner = POINT {
        x: (position.x - width as f64 / 2.0).round() as i32,
        y: (position.y - height as f64 / 2.0).round() as i32,
    };
    surface.present(hwnd, corner);
}

fn set_click_through(hwnd: HWND, through: bool) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let want = if through {
            ex | WS_EX_TRANSPARENT.0
        } else {
            ex & !WS_EX_TRANSPARENT.0
        };
        if want != ex {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want as isize);
        }
    }
}

/// Every interaction ends the same way: the runtime answers, and if it says the
/// picture changed, it goes up.
fn apply(hwnd: HWND, app: &mut App, output: InteractionOutput) {
    if let Some(enabled) = output.set_interaction_enabled {
        set_click_through(hwnd, !enabled);
    }
    if output.render {
        app.player.set_capability(&app.resolver, output.capability);
        let scale = scale_under(app, output.position);
        draw(hwnd, app, output.position, scale);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let handled = APP.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let Some(app) = borrow.as_mut() else {
            return false;
        };
        let now = app.started.elapsed().as_secs_f64();
        match msg {
            WM_TIMER => {
                tick(hwnd, app);
                true
            }
            WM_LBUTTONDOWN => {
                let point = platform::pointer();
                app.drag_from = Some(POINT {
                    x: point.x as i32,
                    y: point.y as i32,
                });
                app.dragged = false;
                let out = app.pet.pointer_down(point, now);
                apply(hwnd, app, out);
                SetCapture(hwnd);
                true
            }
            WM_MOUSEMOVE if app.drag_from.is_some() => {
                let point = platform::pointer();
                let from = app.drag_from.unwrap();
                let dx = point.x - from.x as f64;
                let dy = point.y - from.y as f64;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance > 4.0 {
                    app.dragged = true;
                }
                let out = app.pet.pointer_dragged(point, distance, now);
                apply(hwnd, app, out);
                true
            }
            WM_LBUTTONUP => {
                let point = platform::pointer();
                let was_dragged = app.dragged;
                app.drag_from = None;
                app.dragged = false;
                let _ = ReleaseCapture();
                let out = app.pet.pointer_up(point, was_dragged, now);
                apply(hwnd, app, out);
                true
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                let carried = app.pet.position();
                let fresh = platform::displays();
                app.displays = fresh.clone();
                app.pet.handle_display_change(fresh, carried, now);
                true
            }
            _ => false,
        }
    });
    if handled {
        return LRESULT(0);
    }
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
