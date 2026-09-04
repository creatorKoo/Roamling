// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only


// A console follows a console-subsystem process around, which is wrong for
// something that lives in the tray. Debug builds keep it, because the state
// log printed there is how the loop is watched while it is being built.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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

mod capture;
mod diagnostics;
mod duplication;
mod focus;
mod platform;
mod settings;
mod shell;
mod sprite;
mod strings;
mod tray;
mod tuning;

use roamling_core::{
    look_frame_index, AnimationResolver, BehaviorState, DisplaySnapshot, InteractionOutput,
    PetAnimationPlayer, PetRuntime, RuntimeTuning, RuntimeTuningKey, TickInput, WorldPoint,
    WorldSize,
};
use roamling_agent::{installer, Agent, Receiver};
use roamling_pet::{package, PetAsset};
use settings::Settings;
use sprite::Surface;
use std::cell::RefCell;
use std::path::PathBuf;
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
    /// Packages found on disk, and which one is showing. `None` is the
    /// built-in mascot, which is always available and never fails to load.
    catalog: Vec<package::PetDescriptor>,
    current_package: Option<std::path::PathBuf>,
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
    /// The user's own size multiplier, multiplied into the display's scale.
    /// `runtime.scale` on the macOS side, and the same settings key.
    scale: f64,
    /// What changed and when, for "Copy Diagnostics". Fed from the core's
    /// per-tick `diagnostics`, which keeps nothing itself.
    log: diagnostics::DiagnosticsLog,
    /// The runtime takes these but does not hand them back, so the menu keeps
    /// its own copy of what it last set.
    roaming: bool,
    avoiding: bool,
    interactive: bool,
    /// Windows has no permission prompt for either of these, so consent is a
    /// setting instead -- and both start off. `docs/windows.md` section 6.
    visual: bool,
    /// Whether the caret may be read. Reachable now that agent events exist --
    /// the core only asks for a caret while watching a window, and that watch
    /// starts with one.
    cursor_aware: bool,
    capturer: capture::Capturer,
    /// The two loopback endpoints, and the channel their threads post to. This
    /// is what makes the pet notice anyone working -- without it the caret and
    /// the work seat are both unreachable. `docs/windows.md` W5b.
    ///
    /// Held, never read: dropping a `Receiver` stops its listener, so the field
    /// is what keeps the endpoints open for as long as the app runs.
    #[allow(dead_code)]
    receivers: Vec<Receiver>,
    agent_events: std::sync::mpsc::Receiver<roamling_core::CompanionEvent>,
    tokens: [(Agent, String); 2],
    /// Whether each agent's endpoint actually bound, for the menu's second
    /// status line. A port in use means another copy of Roamling has it.
    listening: [bool; 2],
    /// Events waiting for their moment, with the time each is due. Only the
    /// test reaction uses this -- it is two events three seconds apart, and a
    /// timer thread for that would need a second way into the runtime.
    deferred: Vec<(f64, roamling_core::CompanionEvent)>,
    /// When the last luminance readback happened, for the interval the core asks for.
    luminance_at: f64,
    settings: Settings,
    /// Last reported state, so the log says what changed rather than repeating.
    last_state: BehaviorState,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn main() -> Result<()> {
    // Before any window exists, or the process is stuck with the wrong answer.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };

    let Some(built_in) = roamling_pet::built_in_mochi() else {
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

    let stored = Settings::load();
    let first = displays[0].visible_frame;
    let centre = WorldPoint::new(
        first.origin.x + first.size.width / 2.0,
        first.origin.y + first.size.height / 2.0,
    );
    // A remembered seat is only worth restoring if it is still on a screen.
    // Unplugging the monitor it was sleeping on should not strand it offscreen.
    let start = match (
        stored.bool(settings::HAS_POSITION, false),
        stored.number(settings::POSITION_X),
        stored.number(settings::POSITION_Y),
    ) {
        (true, Some(x), Some(y))
            if displays
                .iter()
                .any(|display| display.frame.contains(WorldPoint::new(x, y))) =>
        {
            WorldPoint::new(x, y)
        }
        _ => centre,
    };
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);

    // Whatever is on disk, and whichever of it was showing last. A package
    // that has gone away or stopped loading falls back to the built-in rather
    // than leaving the user with no pet.
    let catalog = package::discover(&package::default_roots());
    let wanted = stored.text(settings::PET_PACKAGE_PATH).map(PathBuf::from);
    let (asset, current_package) = match wanted {
        Some(path) if catalog.iter().any(|found| found.package == path) => {
            match package::load(&path) {
                Ok(loaded) => {
                    for warning in &loaded.warnings {
                        println!("   pet  {warning}");
                    }
                    println!("pet: {} from {}", loaded.asset.display_name, path.display());
                    (loaded.asset, Some(path))
                }
                Err(error) => {
                    println!("{}: {error}", path.display());
                    (built_in, None)
                }
            }
        }
        _ => (built_in, None),
    };
    println!("{} pet package(s) found", catalog.len());

    let roaming = stored.bool(settings::ROAMING, true);
    let avoiding = stored.bool(settings::AVOID_POINTER, true);
    let interactive = stored.bool(settings::INTERACTIONS, true);
    let visual = stored.bool(settings::VISUAL_PLACEMENT, false);
    let cursor_aware = stored.bool(settings::CURSOR_AWARENESS, false);
    // Clamped to the range the menu offers, so a hand-edited settings file
    // cannot produce a pet too small to catch or too big to walk around.
    let scale = stored.number(settings::SCALE).unwrap_or(1.0).clamp(0.5, 2.0);

    let mut pet = PetRuntime::new(start, stored_tuning(&stored), seed);
    pet.set_displays(displays.clone());
    pet.set_object_size(WorldSize::new(BASE_WIDTH, BASE_HEIGHT));
    pet.set_flags(roaming, avoiding, interactive);

    let resolver = AnimationResolver::new(asset.tracks.clone(), asset.behavior_mappings.clone());
    let player = PetAnimationPlayer::new(&resolver);

    // One token per agent, kept in settings under the same keys the macOS
    // runtime writes. It has to survive restarts: the hook command in the
    // user's own config carries a copy, so a fresh token every launch would
    // silently break every install.
    let mut stored = stored;
    let mut tokens = Vec::new();
    for agent in [Agent::ClaudeCode, Agent::Codex] {
        let key = roamling_agent::token_key(agent);
        let token = match stored.text(key) {
            Some(existing) if existing.len() >= 24 => existing,
            _ => {
                let fresh = roamling_agent::make_token();
                stored.set(key, &fresh);
                fresh
            }
        };
        tokens.push((agent, token));
    }
    let tokens: [(Agent, String); 2] = [tokens[0].clone(), tokens[1].clone()];

    let (agent_sender, agent_events) = std::sync::mpsc::channel();
    let started = Instant::now();
    let mut receivers = Vec::new();
    let mut listening = [false; 2];
    for (index, (agent, token)) in tokens.iter().enumerate() {
        // A port already in use means another copy of Roamling is running.
        // Losing one endpoint is not worth refusing to start over.
        match Receiver::start(*agent, token.clone(), started, agent_sender.clone()) {
            Ok(receiver) => {
                println!(
                    "listening for {} on 127.0.0.1:{}",
                    agent.display_name(),
                    agent.port()
                );
                receivers.push(receiver);
                listening[index] = true;
            }
            Err(error) => println!("{} endpoint unavailable: {error}", agent.display_name()),
        }
    }

    let hwnd = create_window()?;
    let tray_ok = tray::add(hwnd);
    APP.with(|slot| {
        *slot.borrow_mut() = Some(App {
            pet,
            asset,
            resolver,
            player,
            displays,
            started,
            surface: None,
            drawn: None,
            drag_from: None,
            dragged: false,
            tick_ms: 0,
            scale,
            log: diagnostics::DiagnosticsLog::new(),
            roaming,
            avoiding,
            interactive,
            visual,
            cursor_aware,
            catalog,
            current_package,
            capturer: capture::Capturer::default(),
            receivers,
            agent_events,
            tokens,
            listening,
            deferred: Vec::new(),
            luminance_at: f64::NEG_INFINITY,
            settings: stored,
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
    println!("\ntray icon registered: {tray_ok}   (Windows 11 files new ones behind the chevron)");
    println!("roaming. right-click the tray icon for the menu.");

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

    // Whatever the endpoints took while the loop was elsewhere. Draining here
    // rather than on the listener thread keeps the runtime single-threaded.
    let mut pending: Vec<_> = app.agent_events.try_iter().collect();
    // Anything the test reaction left for later, once its moment has come.
    let mut still_waiting = Vec::new();
    for (due, event) in std::mem::take(&mut app.deferred) {
        if due <= now {
            pending.push(event);
        } else {
            still_waiting.push((due, event));
        }
    }
    app.deferred = still_waiting;
    for event in pending {
        println!("{:7.1}s  agent {:?} {:?}", now, event.kind, event.source_id);
        app.pet.handle_activity_event(event, now);
    }

    // The core decides whether the caret is worth a synchronous round trip;
    // the shell decides whether the user allowed one at all.
    // Anything the tuning panel changed. It runs in this thread's message
    // loop but never reaches into the runtime -- see `tuning.rs`.
    if let Some(tuning) = tuning::take_pending() {
        app.pet.apply_tuning(tuning, now);
        remember_tuning(app, tuning);
    }

    let wants_focus = app.pet.begin_tick(now) && app.cursor_aware;
    let queried = if wants_focus { focus::focus() } else { None };

    let pointer = platform::pointer();
    let position = app.pet.position();
    let scale = render_scale(app, position);
    let half_width = BASE_WIDTH * scale / 2.0;
    let half_height = BASE_HEIGHT * scale / 2.0;

    let output = app.pet.finish_tick(&TickInput {
        now,
        pointer,
        primary_button_down: platform::primary_button_down(),
        user_idle_duration: platform::user_idle_duration(),
        // Neither provider exists yet; the core stays on the MVP 3 path.
        capture_authorized: app.visual,
        focus_authorized: app.cursor_aware,
        did_query_focus: wants_focus,
        queried_focus: queried,
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
    // The core clears these every tick and keeps nothing; whoever wants a
    // history keeps one. Only transitions survive -- see `diagnostics.rs`.
    for (category, message) in &output.diagnostics {
        app.log.record(category, message, now);
    }

    app.player.set_capability(&app.resolver, output.capability);
    // A sheet with fewer than eleven rows has no directional look row, and
    // mochi-v3 has nine -- its `gaze` track does the watching instead.
    let look = output
        .look_direction_degrees
        .and_then(|degrees| look_frame_index(degrees, app.asset.columns, app.asset.rows));
    app.player.set_look_frame(look);
    app.player.update(output.delta_time * output.locomotion_rate);

    refresh_luminance(app, &output.luminance_requests, now);

    if output.persist_position {
        remember(app, output.position);
    }

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

/// Service whatever the core asked to look at, if the user allowed it and the
/// interval it named has passed. The core sets that interval by situation --
/// three seconds while watching a window, six while wandering -- because on
/// macOS one capture costs 62 ms and the period is the latency. See
/// `docs/battery.md`: this is the most expensive thing the app does.
fn refresh_luminance(app: &mut App, requests: &[roamling_core::LuminanceRequest], now: f64) {
    if !app.visual || !capture::screen_is_readable() {
        return;
    }
    let Some(request) = requests.first() else {
        return;
    };
    if now - app.luminance_at < request.interval {
        return;
    }
    let centre = request.region.center();
    let display = app
        .displays
        .iter()
        .find(|display| display.frame.contains(centre))
        .or_else(|| app.displays.first());
    let Some(display) = display.cloned() else {
        return;
    };
    app.luminance_at = now;
    let capture = app.capturer.luminance(&display);
    // The cost that decides the interval, and the thing docs/battery.md calls
    // the dominant term. macOS pays 62 ms for the same answer.
    println!(
        "   capture  read {:.1} ms + shrink {:.1} ms  -> {}",
        capture.read_ms,
        capture.shrink_ms,
        if capture.unchanged {
            "unchanged".to_string()
        } else {
            capture
                .field
                .as_ref()
                .map_or("nothing".to_string(), |f| format!("{}x{}", f.columns, f.rows))
        }
    );
    // An unchanged screen still has the luminance it had, so the previous
    // field stays. Replacing it with `None` would throw away a good answer.
    if !capture.unchanged {
        app.pet.set_luminance(capture.field);
    }
}

/// The eleven tunable values, under one sub-key each.
///
/// `RuntimeTuningKey` declares the wire order and this walks it, so a key added
/// to the core is saved here without this function being touched.
fn tuning_key(key: RuntimeTuningKey) -> String {
    // The camelCase names the macOS blob uses, derived rather than tabulated.
    let name = format!("{key:?}");
    let mut out = String::with_capacity(name.len());
    for (index, character) in name.chars().enumerate() {
        if index == 0 {
            out.extend(character.to_lowercase());
        } else {
            out.push(character);
        }
    }
    format!("{}{out}", settings::TUNING_PREFIX)
}

fn remember_tuning(app: &mut App, tuning: RuntimeTuning) {
    for key in roamling_core::TUNING_KEYS {
        let name = tuning_key(key);
        app.settings.set(&name, tuning.get(key));
    }
}

/// Whatever was saved, re-clamped on the way in.
///
/// A missing value keeps the default rather than becoming zero: a settings file
/// written by an older build has fewer keys, and a walking speed of nothing is
/// a pet that never moves again.
fn stored_tuning(stored: &Settings) -> RuntimeTuning {
    let mut tuning = RuntimeTuning::default();
    for key in roamling_core::TUNING_KEYS {
        if let Some(value) = stored.number(&tuning_key(key)) {
            tuning = tuning.with(key, value);
        }
    }
    tuning
}

/// The pet came to rest somewhere worth keeping. Same three keys the macOS
/// runtime writes, so the two platforms describe a seat the same way.
fn remember(app: &mut App, position: WorldPoint) {
    app.settings.set(settings::POSITION_X, position.x);
    app.settings.set(settings::POSITION_Y, position.y);
    app.settings.set(settings::HAS_POSITION, true);
}

/// Swap the pet showing on screen.
///
/// Everything derived from the sheet has to go with it: the resolver reads the
/// tracks, the player holds a position inside one of them, and the surface is
/// sized for the old cell. Keeping any of them would draw the new sheet through
/// the old sheet's idea of where its frames are.
fn adopt(app: &mut App, asset: PetAsset, package: Option<PathBuf>) {
    println!("pet: {}", asset.display_name);
    app.resolver = AnimationResolver::new(asset.tracks.clone(), asset.behavior_mappings.clone());
    app.player = PetAnimationPlayer::new(&app.resolver);
    app.asset = asset;
    app.surface = None;
    app.drawn = None;
    app.current_package = package.clone();
    match package {
        Some(path) => app
            .settings
            .set(settings::PET_PACKAGE_PATH, &path.to_string_lossy()),
        // Empty rather than absent, so the choice of the built-in survives a
        // restart instead of falling back to whatever was selected before.
        None => app.settings.set(settings::PET_PACKAGE_PATH, ""),
    }
}

/// "Test Reaction": a turn starting, then finishing three seconds later.
///
/// Ported from `RoamlingRuntime.testAgentReaction`, including the pause -- the
/// point is to show the whole arc, and an achievement on its own looks like a
/// twitch. macOS schedules the second half on the main queue; here it goes in
/// `deferred` and the tick picks it up, which keeps the runtime single-threaded.
fn test_reaction(app: &mut App, agent: Agent, now: f64) {
    use roamling_core::{CompanionEvent, CompanionEventKind, UserContext};

    let source = format!("{}:test", agent.id());
    let make = |kind, at: f64| {
        let mut event = CompanionEvent::new(
            format!("{source}:{kind:?}:{at}"),
            source.clone(),
            at,
            kind,
            0.55,
            None,
        );
        event.context = Some(UserContext::Working);
        event
    };
    let started = make(CompanionEventKind::ActivityStarted, now);
    let finished = make(CompanionEventKind::Achievement, now + 3.0);
    println!("{:7.1}s  test reaction for {}", now, agent.display_name());
    app.pet.handle_activity_event(started, now);
    app.deferred.push((now + 3.0, finished));
}

/// How big to draw: the display's own scale, times the user's choice.
///
/// The display half is what keeps the pet the same physical size across a
/// mixed-DPI seam; the user half is the Size menu.
fn render_scale(app: &App, position: WorldPoint) -> f64 {
    scale_under(app, position) * app.scale
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
    if output.persist_position {
        remember(app, output.position);
    }
    if let Some(enabled) = output.set_interaction_enabled {
        set_click_through(hwnd, !enabled);
    }
    if output.render {
        app.player.set_capability(&app.resolver, output.capability);
        let scale = render_scale(app, output.position);
        draw(hwnd, app, output.position, scale);
    }
}

/// Windows re-enters this procedure from inside its own calls.
///
/// `SetWindowLongPtrW` on the ex-style sends a message straight back here,
/// synchronously, while the tick that called it is still running. Holding a
/// `RefCell` borrow across that panics -- and a panic cannot unwind out of an
/// `extern "system"` function, so it aborts the process rather than failing
/// something small. That is what "the pet crashes when the cursor comes near"
/// was: the cursor arriving is what flips `interaction_enabled` and makes the
/// call happen at all.
///
/// So the state is moved out for the duration instead of borrowed. A re-entrant
/// message finds nothing there and falls through to `DefWindowProc`, which for
/// the style-change notification is exactly right.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        tray::remove(hwnd);
        PostQuitMessage(0);
        return LRESULT(0);
    }
    let taken = APP.with(|slot| slot.borrow_mut().take());
    let Some(mut app) = taken else {
        return DefWindowProcW(hwnd, msg, wp, lp);
    };
    let handled = dispatch(hwnd, msg, lp, &mut app);
    APP.with(|slot| *slot.borrow_mut() = Some(app));
    if handled {
        LRESULT(0)
    } else {
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}

unsafe fn dispatch(hwnd: HWND, msg: u32, lp: LPARAM, app: &mut App) -> bool {
    {
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
            // The tray forwards the raw mouse message in lParam.
            tray::WM_TRAY
                if matches!(lp.0 as u32, WM_RBUTTONUP | WM_LBUTTONUP | WM_CONTEXTMENU) =>
            {
                let coverage = app.resolver.coverage();
                let names = |set: &[roamling_core::PetCapability]| {
                    let mut names: Vec<String> = set
                        .iter()
                        .map(|c| roamling_core::capability_name(*c).to_string())
                        .collect();
                    names.sort();
                    names
                };
                let state = tray::MenuState {
                    pet_name: app.asset.display_name.clone(),
                    covered: coverage.covered(),
                    total: coverage.total(),
                    substituted: names(&coverage.substituted),
                    placeholder: names(&coverage.placeholder),
                    scale: app.scale,
                    pets: app
                        .catalog
                        .iter()
                        .map(|found| {
                            (
                                found.display_name.clone(),
                                app.current_package.as_deref() == Some(found.package.as_path()),
                            )
                        })
                        .collect(),
                    built_in: app.current_package.is_none(),
                    roaming: app.roaming,
                    avoiding: app.avoiding,
                    interactive: app.interactive,
                    visual: app.visual,
                    cursor_aware: app.cursor_aware,
                    agents: [
                        (
                            app.tokens[0].0,
                            installer::status(app.tokens[0].0, &app.tokens[0].1),
                            app.listening[0],
                        ),
                        (
                            app.tokens[1].0,
                            installer::status(app.tokens[1].0, &app.tokens[1].1),
                            app.listening[1],
                        ),
                    ],
                };
                match tray::show_menu(hwnd, state) {
                    tray::CMD_ROAMING => {
                        app.roaming = !app.roaming;
                        app.pet.set_roaming_enabled(app.roaming, now);
                        app.settings.set(settings::ROAMING, app.roaming);
                    }
                    tray::CMD_AVOID_POINTER => {
                        app.avoiding = !app.avoiding;
                        app.pet.set_pointer_avoidance_enabled(app.avoiding);
                        app.settings.set(settings::AVOID_POINTER, app.avoiding);
                    }
                    tray::CMD_INTERACTIONS => {
                        app.interactive = !app.interactive;
                        app.pet.set_interactions_enabled(app.interactive);
                        app.settings.set(settings::INTERACTIONS, app.interactive);
                    }
                    tray::CMD_VISUAL => {
                        app.visual = !app.visual;
                        app.settings.set(settings::VISUAL_PLACEMENT, app.visual);
                        if !app.visual {
                            // Drop what was captured, and the GPU resource
                            // that reads it, the moment consent ends.
                            app.capturer.release();
                            app.pet.set_luminance(None);
                        }
                    }
                    tray::CMD_CURSOR_AWARE => {
                        app.cursor_aware = !app.cursor_aware;
                        app.settings.set(settings::CURSOR_AWARENESS, app.cursor_aware);
                    }
                    tray::CMD_TUNING => tuning::show(app.pet.tuning()),
                    tray::CMD_RELOAD_PETS => {
                        app.catalog = package::discover(&package::default_roots());
                        println!("{} pet package(s) found", app.catalog.len());
                    }
                    tray::CMD_PET_BUILT_IN => {
                        if let Some(asset) = roamling_pet::built_in_mochi() {
                            adopt(app, asset, None);
                        }
                    }
                    // The pet list, in the order the catalogue reports.
                    picked if picked >= tray::CMD_PET_BASE => {
                        let index = picked - tray::CMD_PET_BASE;
                        if let Some(found) = app.catalog.get(index) {
                            let path = found.package.clone();
                            match package::load(&path) {
                                Ok(loaded) => {
                                    for warning in &loaded.warnings {
                                        println!("   pet  {warning}");
                                    }
                                    adopt(app, loaded.asset, Some(path));
                                }
                                Err(error) => println!("{}: {error}", path.display()),
                            }
                        }
                    }
                    tray::CMD_OPEN_PET_FOLDER => shell::open_pet_folder(),
                    tray::CMD_COPY_DIAGNOSTICS => {
                        let text = app.log.text(now);
                        if !shell::copy_to_clipboard(hwnd, &text) {
                            println!("could not put the diagnostics on the clipboard");
                        }
                    }
                    tray::CMD_ABOUT => shell::about(hwnd),
                    tray::CMD_VIEW_SOURCE => shell::view_source(),
                    // Agent ids live in blocks of ten from 100, below the
                    // pet list -- so this arm has to be bounded at both ends.
                    picked
                        if (tray::CMD_AGENT_BASE..tray::CMD_PET_BUILT_IN).contains(&picked) =>
                    {
                        let index = (picked - tray::CMD_AGENT_BASE) / tray::CMD_AGENT_STRIDE;
                        let action = (picked - tray::CMD_AGENT_BASE) % tray::CMD_AGENT_STRIDE;
                        if let Some((agent, token)) = app.tokens.get(index) {
                            let agent = *agent;
                            if action == tray::CMD_AGENT_TEST {
                                test_reaction(app, agent, now);
                            } else {
                                let outcome = if action == tray::CMD_AGENT_REMOVE {
                                    installer::remove(agent)
                                } else {
                                    installer::install(agent, token)
                                };
                                match outcome {
                                    Ok(()) => println!("{} hooks updated", agent.display_name()),
                                    Err(error) => println!("{}: {error}", agent.display_name()),
                                }
                            }
                        }
                    }
                    // The Size list, in the order `SCALE_CHOICES` declares.
                    picked if picked >= tray::CMD_SCALE_BASE
                        && picked < tray::CMD_SCALE_BASE + tray::SCALE_CHOICES.len() =>
                    {
                        app.scale = tray::SCALE_CHOICES[picked - tray::CMD_SCALE_BASE].1;
                        app.settings.set(settings::SCALE, app.scale);
                        // `draw` notices the footprint changed and tells the
                        // core, which is what keeps the pet on screen when it
                        // grows next to an edge.
                    }
                    tray::CMD_QUIT => {
                        tray::remove(hwnd);
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
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
    }
}
