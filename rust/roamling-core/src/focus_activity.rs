// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! An activity source with no agent behind it: the app the user is working in.
//!
//! Everything else that moves the pet arrives as a hook from a coding agent.
//! This one is assembled from two things any desktop can answer without a
//! permission -- which app is in front, and how long since the last keystroke
//! -- and turned into the same domain events the agents emit. `docs/
//! architecture.md`: domain events, not agent events. It needed one kind of
//! its own, `Present`, because nothing in the vocabulary walked the pet over
//! without also dressing it in something. Downstream, one rule reads where an
//! event came from: while an agent is on duty nothing the desk says is a
//! candidate for the pet's attention (`ActivityDirector::candidates`), so the
//! desk says it again every `BLOCKED_RESEND` until the agent lets go.
//!
//! The whole judgement is in `observe`, which is why it takes `now` rather than
//! reading a clock: the shell samples, this decides, and a test can run a day
//! of desk work in a millisecond.
//!
//! A sitting, as the user sees it:
//!
//! ```text
//! the app comes to the front       Present            walks over and sits
//! first keystroke of the session   ActivityStarted    hops, then HighIntensity
//! any keystroke after that         HighIntensity      runs
//! ten seconds without one          AttentionRequired  tilts its head
//! five seconds of that             ActivityEnded      gives the seat back
//! a keystroke after that           HighIntensity      walks back and runs
//! ```
//!
//! The pet answers typing, not the app merely being there: someone reading a
//! document has a pet sitting quietly beside them, not one tilting its head at
//! them for as long as they read.
//!
//! Nothing about the seat is said to a resting pet. The director drops an
//! event that does not wake a sitting or sleeping pet rather than keeping it,
//! and what wakes the pet is the keystroke itself, later in the same tick than
//! the sample -- so a hop said then was simply lost. The phase carries on while
//! the pet rests; only the telling waits, and the first sample it is awake says
//! what the phase is by then.
//!
//! Three spans do the filtering, and they are the reason the pet is not
//! exhausting to sit next to:
//!
//! - `FOCUS_GRACE` decides whether the user left at all. Glancing at a browser
//!   for a second is not the end of a work session, and without this the pet
//!   would stand up and walk back several times a minute.
//! - `BREAK` decides whether coming back is a new session. Only a real absence
//!   earns the hop again; after a one-minute errand the next keystroke simply
//!   puts the pet back to work.
//! - `WAITING_BEFORE_RELEASE` decides how long the pet asks before it lets the
//!   user be.

use std::collections::VecDeque;

use crate::activity::{ActivitySourceType, CompanionEvent, CompanionEventKind, UserContext};

/// How long after the last keystroke the pet keeps the working picture. Long
/// enough to survive a pause for thought, short enough that a pet still
/// "running" beside an abandoned keyboard is not a lie.
pub const TYPING_WINDOW: f64 = 10.0;

/// How long something else has to hold the front before the user counts as
/// having left. Anything shorter is a glance, and the pet keeps its seat.
pub const FOCUS_GRACE: f64 = 3.0;

/// Away for longer than this and coming back is a new session, so the first
/// keystroke hops again. Below it the session carries on where it stopped.
pub const BREAK: f64 = 120.0;

/// Typing this long in one sitting earns a wave on the way out. The counter is
/// time spent in the typing window, not keystrokes: the source cannot see
/// keystrokes and must never look as though it can.
pub const WAVE_AFTER_TYPING: f64 = 180.0;

/// One `jumping` track, from `PetdexState::standard_length`. How long the beat
/// after the greeting waits once the pet has actually been handed the hop.
///
/// It is measured from there and not from the keystroke, which is the whole
/// subtlety: `begin_watching` overwrites an undelivered arrival reaction with
/// whatever came last, so a beat timed from the keystroke replaced the `Spark`
/// before the pet had played it -- on any walk longer than this, and on any
/// greeting the director had not dispatched yet.
pub const GREETING_DELAY: f64 = 0.840;

/// How long a greeting the pet has been handed may hold the source silent
/// before the pet has put it on. Placement gives a trip `8 + distance / speed
/// * 2` seconds before it abandons it, and an agent can take the seat back
/// mid-walk, so the source must not wait on it forever. Counted from the
/// dispatch: before that the hop is not the pet's to play, and waiting for it
/// is `BLOCKED_RESEND`'s job.
pub const GREETING_TIMEOUT: f64 = 20.0;

/// How long seat news may go without being what the pet is acting on before
/// the source says where things stand again, under a new id.
///
/// While an agent is on duty nothing this source says is a candidate at all
/// (`ActivityDirector::candidates`), and attention forgets an event after
/// thirty seconds. Without saying it again there would be nothing left to
/// offer the moment the agent let go, and the pet would wander off until the
/// next minute's heartbeat. Shorter than those thirty, so there always is.
pub const BLOCKED_RESEND: f64 = 15.0;

/// How often a seated pet says it is still here. `ActivityLifetime::
/// SILENCE_BEFORE_EXPIRY` is 300 seconds, and a user reading a document types
/// nothing at all for far longer than that. Only a seat that is held repeats:
/// asking ends on its own within seconds, and a released seat has nothing to
/// keep.
pub const HEARTBEAT: f64 = 60.0;

/// How long the pet tilts its head at a still keyboard before it gives the
/// seat back and goes about its own day. Long enough to be seen, short enough
/// that it is a question and not a nag. Proposed, not measured: nobody has set
/// this number from use yet.
pub const WAITING_BEFORE_RELEASE: f64 = 5.0;

/// How long the end of a sitting waits for the pet to have the wave in front
/// of it.
///
/// A resting pet is woken for a wave and handed it only once it stands idle,
/// about 1.7 seconds later (`Wake`, then `Stretch`). An `ActivityEnded` said in
/// between took the queued wave back and sat the pet down in `Calm`, so the
/// goodbye waits until the director has dispatched the wave. The cap is for a
/// pet that is not handed it at all -- in the user's hand, given something else
/// first, or kept from it by an agent that came on duty after it went out --
/// so the seat is not left waiting on it. The end takes the wave out of the
/// director's `recent`, so the cap is also as late as a wave kept from the pet
/// can still be played. That bound is why no wave is said while an agent is
/// already on duty (`leave`): kept from the pet from the start, it waited out
/// the cap, and an agent finishing inside it handed the pet the desk's wave
/// after the user had gone.
pub const WAVE_HOLD_TIMEOUT: f64 = 5.0;

/// How many apps the menu offers. The list is there to find the two or three
/// apps someone actually works in, not to mirror the Dock.
const MAX_RECENT: usize = 6;

/// `ReactionPolicy` scales intensity by 0.8 in a working context and asks for
/// 0.5 before it draws `Work` rather than `Observe`, so this is the smallest
/// value that means "typing" rather than "present". The agent adapters
/// normalize to the same place; see `roamling-agent`'s `normalize.rs`.
const TYPING_INTENSITY: f64 = 0.7;
const PRESENT_INTENSITY: f64 = 0.5;
/// A wave, not a fanfare: below the 0.75 that would roll for `LargeCelebrate`.
const WAVE_INTENSITY: f64 = 0.8;

/// What the director knows this source as. One spelling, one place.
const SOURCE_PREFIX: &str = "focus:";

fn source_id(app: &str) -> String {
    format!("{SOURCE_PREFIX}{app}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// No watched app holds the front, past the grace.
    Away,
    /// Sitting beside the app with nothing to show: it came to the front and
    /// nobody has typed into it since.
    Present,
    /// Keys within `TYPING_WINDOW` -- running, or hopping first.
    Typing,
    /// The keys stopped. The pet is asking, for `WAITING_BEFORE_RELEASE`.
    Waiting,
    /// Nobody answered, so the seat went back. The session is not over: the
    /// app is still in front, and the next keystroke brings the pet back to
    /// work without a hop.
    Released,
}

/// A hop the user has not finished seeing.
#[derive(Debug, Clone)]
struct Greeting {
    /// The `ActivityStarted` this waits on -- the latest one, when an agent
    /// on duty has had it said more than once. Whether the pet has it is read
    /// from the director having dispatched *this* id. `None` while it has not
    /// gone out: the keystroke came while the pet was resting, and the hop
    /// waits for it to wake.
    event_id: Option<String>,
    /// The first sample at which the director had dispatched it. Until then
    /// the hop is not spent: an agent on duty can keep it from the pet for as
    /// long as the user types, and the session still owes it afterwards. The
    /// safety cap counts from here.
    dispatched_at: Option<f64>,
    /// The first sample at which the pet had been handed the hop and put it
    /// on, or `None` until then. The beat after it is measured from here.
    arrived_at: Option<f64>,
}

/// The latest seat news that went out, and the last sample at which it was
/// what the pet was acting on -- or when it went out, if it has not been yet.
#[derive(Debug, Clone)]
struct SeatNews {
    id: String,
    heard_at: f64,
}

/// The end of a sitting, held back behind the wave that went out first.
#[derive(Debug, Clone)]
struct Goodbye {
    app: String,
    wave_id: String,
    waved_at: f64,
}

#[derive(Debug)]
pub struct FocusActivity {
    phase: Phase,
    /// The watched app the pet is sitting beside, if any. Still set while the
    /// seat is released: the user has not left the app.
    app: Option<String>,
    /// The app of the latest session, kept after the user leaves it, so that
    /// coming back to a different one is a new session.
    last_app: Option<String>,
    /// When the user was last seen leaving a watched app for good. `None`
    /// means they have not left one since sitting down -- or never sat down,
    /// which the session rule reads as "first time", the same answer it gives
    /// after a long absence.
    left_at: Option<f64>,
    /// When something other than the watched app took the front. Cleared the
    /// moment the watched app comes back, which is what makes a glance free.
    away_since: Option<f64>,
    /// The first sample of the current unbroken run with the watched app in
    /// front. Only a key pressed after it is typing: Cmd-Tab and Alt-Tab are
    /// keys, and the one that brought the app forward is not work.
    in_front_since: Option<f64>,
    /// Whether this session's hop has reached the pet. Not whether anyone has
    /// typed: a hop an agent kept from the pet is still owed.
    session_typed: bool,
    /// When the last key that counted as typing into this app was pressed.
    /// The typing window is measured from here and not from the raw last
    /// key, which is also the Cmd-Tab that went to another window and back.
    last_counted_key_at: Option<f64>,
    /// When the pet started asking, while it is.
    waiting_since: Option<f64>,
    /// Seconds spent inside the typing window this session.
    typed_seconds: f64,
    last_observed_at: Option<f64>,
    last_emitted_at: f64,
    /// The greeting in progress, if any. While one is, the source says nothing
    /// at all -- anything it said would replace the hop before it played.
    greeting: Option<Greeting>,
    /// Whether the pet was resting at this sample. Seat news is held back
    /// while it is.
    pet_resting: bool,
    /// Seat news was held back for a resting pet and nothing has made it good
    /// since. The first awake sample says what the phase is by then -- not
    /// what was held back, which may be stale.
    news_owed: bool,
    /// What was last said about the seat, while there is a seat to say it
    /// about. An `ActivityEnded` clears it.
    seat_news: Option<SeatNews>,
    /// The user left after a long stretch, a wave went out, and the end of
    /// the sitting waits for the pet to have it (`WAVE_HOLD_TIMEOUT`).
    goodbye: Option<Goodbye>,
    /// Most recent first. The menu's list of apps to choose from.
    recent: VecDeque<String>,
    next_id: u64,
}

impl Default for FocusActivity {
    fn default() -> Self {
        Self {
            phase: Phase::Away,
            app: None,
            last_app: None,
            left_at: None,
            away_since: None,
            in_front_since: None,
            session_typed: false,
            last_counted_key_at: None,
            waiting_since: None,
            typed_seconds: 0.0,
            last_observed_at: None,
            last_emitted_at: f64::NEG_INFINITY,
            greeting: None,
            pet_resting: false,
            news_owed: false,
            seat_news: None,
            goodbye: None,
            recent: VecDeque::new(),
            next_id: 0,
        }
    }
}

impl FocusActivity {
    pub fn new() -> Self {
        Self::default()
    }

    /// One sample of the desk.
    ///
    /// - `app`: what is in front, or `None` when the shell cannot say. **The
    ///   pet's own app is `None`**: `MacWindowProvider` answers nil for itself,
    ///   because clicking the menu bar is not the user going to work.
    /// - `watched`: whether the user has said that app is work.
    /// - `seconds_since_key`: since the last keystroke, at whatever resolution
    ///   the platform can manage without an input hook.
    /// - `dispatched_event`: the event the pet last acted on,
    ///   `PetRuntime::last_dispatched_activity_id`. **Not which source holds
    ///   the seat**: the seat is usually this source's already when the first
    ///   keystroke comes, because the pet sat down on `Present`, so "on duty
    ///   here" is true before the greeting has reached anyone. It also says
    ///   when the pet has been handed the wave.
    /// - `arrival_pending`: whether the pet still owes a reaction for what it
    ///   was last told, `PetRuntime::has_arrival_reaction`.
    /// - `pet_resting`: whether the pet is sitting, looking for a place to
    ///   sleep or asleep, `PetRuntime::is_resting`. **The same test the
    ///   director uses** to drop an event that does not wake the pet, so
    ///   nothing this source says about the seat is ever dropped for that.
    /// - `agent_on_duty`: whether an agent is on duty,
    ///   `PetRuntime::agent_on_duty` -- **the director's own test** for keeping
    ///   the desk from the pet. Read when the user leaves: a long stretch left
    ///   while it holds ends without a wave (plan §9.7a).
    ///
    /// The last four are the facts this source cannot work out for itself.
    /// The first two are about the greeting and the wave: without them the hop is lost on
    /// any walk longer than `GREETING_DELAY` and on any greeting the director
    /// has not dispatched yet. The third is whether anything said now would be
    /// heard at all, and the fourth whether a wave would be the pet's to play.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        app: Option<&str>,
        watched: bool,
        seconds_since_key: f64,
        dispatched_event: Option<&str>,
        arrival_pending: bool,
        pet_resting: bool,
        agent_on_duty: bool,
        now: f64,
    ) -> Vec<CompanionEvent> {
        let elapsed = self.last_observed_at.map_or(0.0, |last| (now - last).max(0.0));
        self.last_observed_at = Some(now);
        self.pet_resting = pet_resting;
        // The pet acting on what was said last is what keeps it from being
        // said again.
        if let Some(news) = self.seat_news.as_mut() {
            if dispatched_event == Some(news.id.as_str()) {
                news.heard_at = now;
            }
        }

        let on_seat = watched && app.is_some() && self.app.as_deref() == app;
        // Anything else in front -- another app, or this one's own menu --
        // breaks the run, so the key that brings the work app back is not
        // counted as typing into it.
        if !on_seat {
            self.in_front_since = None;
        }

        let mut events = Vec::new();
        // The end of a sitting that waved goodbye, once the pet has the wave
        // -- whatever is in front by then, the pet's own menu included: the
        // user left before it, and not knowing what came after does not
        // undo that.
        if self.goodbye_is_due(dispatched_event, now) {
            self.say_goodbye(now, &mut events);
        }

        // Nothing to say about a desk nobody can describe. "Unknown" is not
        // "the user left": the pet's own menu bar and its tuning panel both
        // come through here as `None`, and reading them as a departure ended
        // the session -- with a goodbye wave -- for anyone who left a menu open
        // for three seconds.
        let Some(app) = app else { return events };
        self.remember(app);

        // Time only counts as typing while the app it was typed into is the
        // one being watched. Within the leaving grace the seat is still held,
        // but the keys are going somewhere else.
        if on_seat && self.phase == Phase::Typing {
            self.typed_seconds += elapsed;
        }

        if !watched {
            self.leave(now, agent_on_duty, &mut events);
        } else if on_seat {
            self.away_since = None;
            let since = *self.in_front_since.get_or_insert(now);
            let typed = seconds_since_key < TYPING_WINDOW && now - seconds_since_key > since;
            if typed {
                self.last_counted_key_at = Some(now - seconds_since_key);
            }
            self.sustain(typed, dispatched_event, arrival_pending, now, &mut events);
        } else {
            self.arrive(app, now, &mut events);
        }
        events
    }

    /// The apps the user has had in front lately, most recent first. This app
    /// is not among them: the shell answers `None` for itself.
    pub fn recent_apps(&self) -> Vec<String> {
        self.recent.iter().cloned().collect()
    }

    /// Which watched app the pet is sitting beside, or `None`. Read by tests.
    pub fn seated_app(&self) -> Option<&str> {
        self.app.as_deref()
    }

    // ------------------------------------------------------------- transitions

    /// A watched app the pet is not already sitting beside.
    fn arrive(&mut self, app: &str, now: f64, events: &mut Vec<CompanionEvent>) {
        // Back before the pet has the last sitting's wave: that sitting ends
        // first, so the director hears the end before the new start.
        self.say_goodbye(now, events);
        // Straight from one watched app to another: the first one is over.
        // The director starts a trip per source, so without this the pet would
        // stay at the old window with nothing to say it should move.
        if let Some(previous) = self.app.take() {
            self.tell(&previous, CompanionEventKind::ActivityEnded, PRESENT_INTENSITY, now, events);
        }
        // A real absence, the first time, or a different window: a new
        // session, and the counter starts over because the wave is for one
        // sitting. Straight from one app to another reaches here with
        // `left_at` empty, which reads as rested.
        let rested = self.left_at.is_none_or(|left| now - left >= BREAK);
        if rested || self.last_app.as_deref() != Some(app) {
            self.session_typed = false;
            self.typed_seconds = 0.0;
        }
        self.app = Some(app.to_owned());
        self.last_app = Some(app.to_owned());
        self.away_since = None;
        self.left_at = None;
        self.in_front_since = Some(now);
        self.greeting = None;
        self.waiting_since = None;
        self.phase = Phase::Present;
        // No hop and no question: the app being in front says nothing about
        // what the user wants yet.
        self.tell(app, CompanionEventKind::Present, PRESENT_INTENSITY, now, events);
    }

    /// The pet is already beside this app, on its way there, or has given the
    /// seat back while the app stays in front.
    fn sustain(
        &mut self,
        typed: bool,
        dispatched_event: Option<&str>,
        arrival_pending: bool,
        now: f64,
        events: &mut Vec<CompanionEvent>,
    ) {
        if self.greeting.is_some() {
            self.sustain_greeting(dispatched_event, arrival_pending, now, events);
            return;
        }

        let heartbeat_due = now - self.last_emitted_at >= HEARTBEAT;
        match self.phase {
            // Asking is here too: a hop an agent kept from the pet is still
            // the session's, and the keystroke after the question pays it.
            Phase::Present | Phase::Waiting | Phase::Released if typed => {
                if self.session_typed {
                    self.work(now, events);
                } else {
                    self.greet(now, events);
                }
            }
            // The director needs to hear that the seat is still earned.
            Phase::Present if heartbeat_due => {
                self.say(CompanionEventKind::Present, now, events);
            }
            Phase::Typing if self.keys_stopped(now) => self.ask(now, events),
            Phase::Typing if heartbeat_due => self.work(now, events),
            Phase::Waiting
                if self
                    .waiting_since
                    .is_some_and(|since| now - since >= WAITING_BEFORE_RELEASE) =>
            {
                self.release(now, events);
            }
            _ => {}
        }

        // Awake again with seat news held back, and nothing above has said
        // anything since: tell the pet where things stand now.
        if self.news_owed && !self.pet_resting {
            self.news_owed = false;
            self.say_where_things_stand(now, events);
        }

        // Said, and for a while now not what the pet is acting on: an agent
        // is on duty. Say it again, so that attention still has it the moment
        // the agent lets go. Anything said above has just reset the clock.
        if self.resend_due(now) {
            self.say_where_things_stand(now, events);
        }
    }

    /// The seat news for the phase as it stands. A released seat, and no
    /// seat, have nothing to say.
    fn say_where_things_stand(&mut self, now: f64, events: &mut Vec<CompanionEvent>) {
        match self.phase {
            Phase::Present => self.say(CompanionEventKind::Present, now, events),
            Phase::Typing => self.say(CompanionEventKind::HighIntensity, now, events),
            Phase::Waiting => self.say(CompanionEventKind::AttentionRequired, now, events),
            Phase::Away | Phase::Released => {}
        }
    }

    /// A hop in progress: held back for a resting pet, out but kept from the
    /// pet by an agent on duty, or with the pet and waiting to be seen. The
    /// source says nothing else meanwhile -- anything it said would replace
    /// the hop before it played.
    fn sustain_greeting(
        &mut self,
        dispatched_event: Option<&str>,
        arrival_pending: bool,
        now: f64,
        events: &mut Vec<CompanionEvent>,
    ) {
        let Some((event_id, dispatched)) = self
            .greeting
            .as_ref()
            .map(|greeting| (greeting.event_id.clone(), greeting.dispatched_at.is_some()))
        else {
            return;
        };
        let Some(event_id) = event_id else {
            // The session's first keystroke came while the pet was resting.
            // The hop goes out the first sample it is awake.
            self.send_greeting(now, events);
            return;
        };
        if !dispatched {
            if dispatched_event == Some(event_id.as_str()) {
                // The pet has it, so the session has had its hop -- whether
                // or not the walk there works out.
                if let Some(greeting) = self.greeting.as_mut() {
                    greeting.dispatched_at = Some(now);
                }
                self.session_typed = true;
            } else if self.keys_stopped(now) {
                // Kept from the pet for as long as the user typed. The hop was
                // never seen, so it is not spent: the question now, and the
                // next keystroke hops.
                self.greeting = None;
                self.ask(now, events);
                return;
            } else {
                if self.resend_due(now) {
                    self.send_greeting(now, events);
                }
                return;
            }
        }
        if !self.greeting_is_over(dispatched_event, arrival_pending, now) {
            return;
        }
        self.greeting = None;
        // The hop has been seen. What follows is what the keyboard says now,
        // which after a long walk may already be a pause.
        if self.keys_stopped(now) {
            self.ask(now, events);
        } else {
            self.work(now, events);
        }
    }

    /// Something else is in front.
    fn leave(&mut self, now: f64, agent_on_duty: bool, events: &mut Vec<CompanionEvent>) {
        let Some(app) = self.app.clone() else { return };
        let since = *self.away_since.get_or_insert(now);
        if now - since < FOCUS_GRACE {
            return;
        }
        if self.typed_seconds >= WAVE_AFTER_TYPING {
            // "That was a good stretch." The director clears the seat on an
            // achievement and celebrates where the pet stands, which is
            // exactly the shape of leaving well -- beside the app, or out
            // roaming after the seat went back: the stretch was typed either
            // way (plan §9.6a).
            //
            // Not while an agent is on duty (plan §9.7a). Nothing the desk
            // says is a candidate then (`ActivityDirector::candidates`), but
            // the director does not forget it: a wave passed over stayed in
            // `recent` until the end behind it went out, up to
            // `WAVE_HOLD_TIMEOUT` later, and an agent finishing in between
            // queued it -- the pet reacted to the desk seconds after the user
            // had gone. So the leave asks the director's own question and says
            // only the end. What the pet last acted on is no stand-in for it:
            // a finished agent's id stays there and kept a free pet from
            // waving, and the release before an ordinary goodbye empties it.
            //
            // Spent the moment it is earned, said or not.
            self.typed_seconds = 0.0;
            if !agent_on_duty {
                let wave =
                    self.tell(&app, CompanionEventKind::Achievement, WAVE_INTENSITY, now, events);
                self.goodbye =
                    wave.map(|wave_id| Goodbye { app: app.clone(), wave_id, waved_at: now });
            }
        }
        // Behind a wave, the end waits for the pet to have it: said now, it
        // took back a wave still queued for a pet getting up to play it.
        if self.goodbye.is_none() {
            self.tell(&app, CompanionEventKind::ActivityEnded, PRESENT_INTENSITY, now, events);
        }
        self.app = None;
        self.phase = Phase::Away;
        self.away_since = None;
        self.greeting = None;
        self.waiting_since = None;
        // When they actually left, not when the grace ran out: otherwise every
        // absence would read three seconds shorter than it was.
        self.left_at = Some(now - FOCUS_GRACE);
    }

    /// The session's first keystroke: the hop. The source then holds its
    /// tongue until the pet has played it. Sending it does not spend it --
    /// the pet being handed it does (`sustain_greeting`).
    fn greet(&mut self, now: f64, events: &mut Vec<CompanionEvent>) {
        if self.app.is_none() {
            return;
        }
        self.greeting = Some(Greeting { event_id: None, dispatched_at: None, arrived_at: None });
        self.waiting_since = None;
        self.phase = Phase::Typing;
        self.send_greeting(now, events);
    }

    /// Sends the hop the greeting waits on -- again, under a new id, when an
    /// agent on duty kept the last one from the pet -- unless the pet is
    /// resting; then the next sample tries again.
    fn send_greeting(&mut self, now: f64, events: &mut Vec<CompanionEvent>) {
        let Some(app) = self.app.clone() else { return };
        let Some(id) =
            self.tell(&app, CompanionEventKind::ActivityStarted, PRESENT_INTENSITY, now, events)
        else {
            return;
        };
        if let Some(greeting) = self.greeting.as_mut() {
            greeting.event_id = Some(id);
        }
    }

    fn work(&mut self, now: f64, events: &mut Vec<CompanionEvent>) {
        self.say(CompanionEventKind::HighIntensity, now, events);
        self.waiting_since = None;
        self.phase = Phase::Typing;
    }

    fn ask(&mut self, now: f64, events: &mut Vec<CompanionEvent>) {
        self.say(CompanionEventKind::AttentionRequired, now, events);
        self.waiting_since = Some(now);
        self.phase = Phase::Waiting;
    }

    /// Gives the seat back. The app, the session and the typing counter all
    /// stay: the user is still in front of their work.
    fn release(&mut self, now: f64, events: &mut Vec<CompanionEvent>) {
        self.say(CompanionEventKind::ActivityEnded, now, events);
        self.waiting_since = None;
        self.phase = Phase::Released;
    }

    // ----------------------------------------------------------------- helpers

    /// The pet has been handed the wave, or has had as long as it gets.
    fn goodbye_is_due(&self, dispatched_event: Option<&str>, now: f64) -> bool {
        self.goodbye.as_ref().is_some_and(|goodbye| {
            dispatched_event == Some(goodbye.wave_id.as_str())
                || now - goodbye.waved_at >= WAVE_HOLD_TIMEOUT
        })
    }

    /// Ends the sitting a wave went out ahead of. Nothing, if none did.
    fn say_goodbye(&mut self, now: f64, events: &mut Vec<CompanionEvent>) {
        let Some(goodbye) = self.goodbye.take() else { return };
        self.tell(&goodbye.app, CompanionEventKind::ActivityEnded, PRESENT_INTENSITY, now, events);
    }

    /// Whether the user has seen the hop and the beat after it is due.
    ///
    /// The hop is seen once the director has dispatched **this** greeting and
    /// the pet owes nothing more for it -- it walked there, or was there
    /// already, and put it on. Both halves are needed:
    ///
    /// - `arrival_pending` is false after the hop is worn, but also before the
    ///   director has dispatched anything at all: for a pet in the user's hand
    ///   the event is queued, and for an agent that outranks this source it is
    ///   passed over. Reading it alone, the beat went out on top of a hop the
    ///   pet had not been given.
    /// - Which source holds the seat says nothing: the pet sat down on this
    ///   source's `Present`, so the seat is ours before the greeting exists.
    ///
    /// The cap is the only other thing that can end a greeting, and it counts
    /// from the dispatch: before that nothing is on its way to the pet.
    fn greeting_is_over(
        &mut self,
        dispatched_event: Option<&str>,
        arrival_pending: bool,
        now: f64,
    ) -> bool {
        let Some(greeting) = self.greeting.as_mut() else { return true };
        let Some(dispatched_at) = greeting.dispatched_at else { return false };
        if now - dispatched_at >= GREETING_TIMEOUT {
            return true;
        }
        let Some(arrived_at) = greeting.arrived_at else {
            if dispatched_event.is_some() && dispatched_event == greeting.event_id.as_deref()
                && !arrival_pending
            {
                // Handed over this very sample: the hop is playing now, so the
                // wait starts here rather than back at the keystroke.
                greeting.arrived_at = Some(now);
            }
            return false;
        };
        now - arrived_at >= GREETING_DELAY
    }

    fn say(&mut self, kind: CompanionEventKind, now: f64, events: &mut Vec<CompanionEvent>) {
        let Some(app) = self.app.clone() else { return };
        self.tell(&app, kind, Self::intensity(kind), now, events);
    }

    /// Every event leaves through here. Seat news for a resting pet does not
    /// leave at all: the director would drop it, so it is owed instead, and
    /// made good by the first sample the pet is awake. Returns the id of what
    /// went out.
    fn tell(
        &mut self,
        app: &str,
        kind: CompanionEventKind,
        intensity: f64,
        now: f64,
        events: &mut Vec<CompanionEvent>,
    ) -> Option<String> {
        let seat_news = Self::is_seat_news(kind);
        if seat_news {
            if self.pet_resting {
                self.news_owed = true;
                return None;
            }
            self.news_owed = false;
        }
        let event = self.event(app, kind, intensity, now);
        let id = event.id.clone();
        events.push(event);
        if seat_news {
            self.seat_news = Some(SeatNews { id: id.clone(), heard_at: now });
        } else if kind == CompanionEventKind::ActivityEnded {
            self.seat_news = None;
        }
        Some(id)
    }

    /// Whether the keyboard has been still for the typing window, counting
    /// only keys that were typing into this app. A Cmd-Tab to another window
    /// and back is two keys, and neither of them is work.
    fn keys_stopped(&self, now: f64) -> bool {
        self.last_counted_key_at.is_none_or(|at| now - at >= TYPING_WINDOW)
    }

    /// Seat news has gone `BLOCKED_RESEND` without being what the pet acts on.
    fn resend_due(&self, now: f64) -> bool {
        self.seat_news.as_ref().is_some_and(|news| now - news.heard_at >= BLOCKED_RESEND)
    }

    /// The kinds that dress a pet at a seat, and that the director drops for
    /// a resting pet because none of them wakes it. `ActivityEnded` is handled
    /// before the director looks at rest, and a wave is for leaving: both go
    /// out whatever the pet is doing.
    fn is_seat_news(kind: CompanionEventKind) -> bool {
        matches!(
            kind,
            CompanionEventKind::Present
                | CompanionEventKind::ActivityStarted
                | CompanionEventKind::HighIntensity
                | CompanionEventKind::AttentionRequired
        )
    }

    fn intensity(kind: CompanionEventKind) -> f64 {
        if kind == CompanionEventKind::HighIntensity {
            TYPING_INTENSITY
        } else {
            PRESENT_INTENSITY
        }
    }

    /// Every event gets a fresh id even when the kind repeats: the director
    /// drops a repeat of the id it last acted on, so a heartbeat that reused
    /// one would be silently discarded and the seat would expire.
    fn event(
        &mut self,
        app: &str,
        kind: CompanionEventKind,
        intensity: f64,
        now: f64,
    ) -> CompanionEvent {
        self.next_id += 1;
        let source_id = source_id(app);
        let id = format!("{source_id}:{}", self.next_id);
        self.last_emitted_at = now;
        CompanionEvent::new(id, source_id, now, kind, intensity, None)
            .with_context(Some(UserContext::Working))
            .with_source_type(ActivitySourceType::System)
    }

    fn remember(&mut self, app: &str) {
        if self.recent.front().map(String::as_str) == Some(app) {
            return;
        }
        if let Some(index) = self.recent.iter().position(|seen| seen == app) {
            self.recent.remove(index);
        }
        self.recent.push_front(app.to_owned());
        while self.recent.len() > MAX_RECENT {
            self.recent.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{ActivityLifetime, CompanionReaction};
    use crate::activity_director::{wants_window_hint, ActivityDirector, ActivityEffect};
    use crate::geometry::WorldRect;
    use crate::world::LocationHint;

    use CompanionEventKind::{
        Achievement, ActivityEnded, ActivityStarted, AttentionRequired, HighIntensity, Present,
    };

    const HWP: &str = "Hwp.exe";
    const HSHOW: &str = "Hshow.exe";
    const CHROME: &str = "chrome.exe";

    /// No key in a long while.
    const QUIET: f64 = 3_600.0;

    /// Samples it takes to leave: the first one away only starts the grace.
    const LEAVE: f64 = FOCUS_GRACE + 0.5;

    /// What the pet is acting on while an agent has it.
    const AGENT_EVENT: &str = "claude-code:session-1:9";

    fn kinds(events: &[CompanionEvent]) -> Vec<CompanionEventKind> {
        events.iter().map(|event| event.kind).collect()
    }

    /// The agent the director tests sit beside.
    const AGENT_SOURCE: &str = "claude-code:session-1";

    /// What the shell does before handing a desk event over: the window in
    /// front, for the kinds that want one.
    fn located(mut event: CompanionEvent) -> CompanionEvent {
        if wants_window_hint(event.kind) {
            event.location_hint = Some(LocationHint::new(
                Some(WorldRect::new(200.0, 200.0, 700.0, 500.0)),
                0.8,
            ));
        }
        event
    }

    /// The agent's `said`th event, at a terminal of its own.
    fn agent_says(said: usize, kind: CompanionEventKind, at: f64) -> CompanionEvent {
        let terminal = WorldRect::new(1_000.0, 450.0, 300.0, 300.0);
        CompanionEvent::new(
            format!("{AGENT_SOURCE}:{said}"),
            AGENT_SOURCE,
            at,
            kind,
            0.8,
            Some(LocationHint::new(Some(terminal), 0.8)),
        )
        .with_context(Some(UserContext::Working))
    }

    /// Every event the pet acts on, in order.
    fn note_acted_on(director: &ActivityDirector, acted_on: &mut Vec<String>) {
        if let Some(id) = director.last_dispatched_id() {
            if acted_on.last().map(String::as_str) != Some(id) {
                acted_on.push(id.to_owned());
            }
        }
    }

    /// The desk as the shell sees it, sampled every half second, beside a pet
    /// already standing at the window: whatever the source says is dispatched
    /// at once and worn by the next sample. `resting` is that pet asleep in
    /// the seat, and `blocked` an agent on duty, which the director lets
    /// nothing from the desk past and which the source is told as
    /// `agent_on_duty`. The tests about a pet that is not ready in other ways
    /// drive `observe` by hand instead.
    struct Desk {
        source: FocusActivity,
        front: Option<&'static str>,
        last_key_at: f64,
        dispatched: Option<String>,
        pending: bool,
        /// Sitting or asleep, as `PetRuntime::is_resting` reads it.
        resting: bool,
        blocked: bool,
        now: f64,
        said: Vec<CompanionEvent>,
    }

    impl Desk {
        fn new() -> Self {
            Self {
                source: FocusActivity::new(),
                front: None,
                last_key_at: 100.0 - QUIET,
                dispatched: None,
                pending: false,
                resting: false,
                blocked: false,
                now: 100.0,
                said: Vec::new(),
            }
        }

        /// An agent takes the pet, and keeps it until `unblock`.
        fn block(&mut self) {
            self.blocked = true;
            self.dispatched = Some(AGENT_EVENT.to_owned());
            self.pending = false;
        }

        /// The agent lets go. The director queues the latest thing the desk
        /// said that attention can still see, and the pet takes it up: an
        /// `ActivityEnded` took the desk out of the running instead.
        fn unblock(&mut self) {
            self.blocked = false;
            match self.said.last() {
                Some(last) if last.kind != ActivityEnded && self.now - last.timestamp <= 30.0 => {
                    self.dispatched = Some(last.id.clone());
                    self.pending = wants_window_hint(last.kind);
                }
                _ => {
                    self.dispatched = None;
                    self.pending = false;
                }
            }
        }

        /// Chrome is the one app nobody called work.
        fn focus(&mut self, app: Option<&'static str>) {
            self.front = app;
        }

        /// A key pressed between this sample and the next.
        fn press(&mut self) {
            self.last_key_at = self.now + 0.1;
        }

        fn step(&mut self) -> Vec<CompanionEventKind> {
            self.now += 0.5;
            let watched = self.front.is_some_and(|app| app != CHROME);
            let events = self.source.observe(
                self.front,
                watched,
                self.now - self.last_key_at,
                self.dispatched.as_deref(),
                self.pending,
                self.resting,
                self.blocked,
                self.now,
            );
            self.pending = false;
            if let Some(last) = events.last().filter(|_| !self.blocked) {
                self.dispatched = Some(last.id.clone());
                self.pending = wants_window_hint(last.kind);
            }
            self.said.extend(events.iter().cloned());
            kinds(&events)
        }

        /// Samples for `seconds` with nobody touching the keyboard.
        fn idle_for(&mut self, seconds: f64) -> Vec<CompanionEventKind> {
            let until = self.now + seconds;
            let mut out = Vec::new();
            while self.now + 0.25 < until {
                out.extend(self.step());
            }
            out
        }

        /// Samples for `seconds` with a key before every one of them.
        fn type_for(&mut self, seconds: f64) -> Vec<CompanionEventKind> {
            let until = self.now + seconds;
            let mut out = Vec::new();
            while self.now + 0.25 < until {
                self.press();
                out.extend(self.step());
            }
            out
        }

        /// The work app in front, the pet sat down, nothing typed yet.
        fn seated() -> Self {
            let mut desk = Self::new();
            desk.focus(Some(HWP));
            assert_eq!(desk.step(), [Present]);
            desk
        }
    }

    // ------------------------------------------------------------ plan §6: 1

    #[test]
    fn a_work_app_in_front_is_present_and_says_so_each_minute() {
        let mut desk = Desk::seated();
        let first = desk.said[0].clone();
        assert_eq!(first.source_id, "focus:Hwp.exe");
        assert_eq!(first.context, Some(UserContext::Working));
        // The desk, not an agent: an agent on duty keeps the pet (plan §9 B2).
        assert_eq!(first.source_type, ActivitySourceType::System);

        assert_eq!(desk.idle_for(HEARTBEAT - 0.5), [], "the pet reacted to the app being there");
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.idle_for(HEARTBEAT * 3.0), [Present, Present, Present]);

        let mut ids: Vec<&str> = desk.said.iter().map(|event| event.id.as_str()).collect();
        let said = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), said, "a heartbeat reused an id");
    }

    // ------------------------------------------------------------ plan §6: 2

    /// Cmd-Tab and Alt-Tab are keys. The one that brought the app forward was
    /// pressed before the sample that first saw it, and is not typing.
    #[test]
    fn the_key_that_brought_the_app_forward_is_not_typing() {
        let mut desk = Desk::new();
        desk.press();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.idle_for(8.0), [], "the switch itself was read as typing");

        // A key with the app already in front is.
        assert_eq!(desk.type_for(0.5), [ActivityStarted]);
    }

    /// The same key read at the exact sample of arrival is still before it.
    #[test]
    fn a_key_at_the_moment_of_arrival_is_not_typing() {
        let mut source = FocusActivity::new();
        let events = source.observe(Some(HWP), true, 0.0, None, false, false, false, 100.0);
        assert_eq!(kinds(&events), [Present]);
        assert!(source.observe(Some(HWP), true, 0.5, None, false, false, false, 100.5).is_empty());
        let events = source.observe(Some(HWP), true, 0.2, None, false, false, false, 101.0);
        assert_eq!(kinds(&events), [ActivityStarted]);
    }

    /// A glance away and back by keyboard is two keys and no typing.
    #[test]
    fn a_glance_away_and_back_by_keyboard_is_not_typing() {
        let mut desk = Desk::seated();
        desk.idle_for(5.0);
        desk.press();
        desk.focus(Some(CHROME));
        assert_eq!(desk.step(), []);
        desk.press();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [], "back inside the grace, and the pet never stood up");
        assert_eq!(desk.idle_for(8.0), [], "the key that came back was read as typing");
    }

    // ------------------------------------------------------------ plan §6: 3

    #[test]
    fn the_first_keystroke_hops_and_the_beat_waits_for_the_hop() {
        let mut source = FocusActivity::new();
        let present = source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0);
        let seat = present[0].id.clone();

        let events = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 101.0);
        assert_eq!(kinds(&events), [ActivityStarted]);
        let hop = events[0].id.clone();

        // Not yet dispatched: the seat's `Present` is still the last thing the
        // pet acted on, and nothing is owed.
        assert!(source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 101.5).is_empty());
        // Dispatched, not yet worn.
        assert!(source.observe(Some(HWP), true, 0.2, Some(&hop), true, false, false, 102.0).is_empty());
        // Worn: the hop plays from here.
        assert!(source.observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, 102.5).is_empty());
        assert!(source
            .observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, 102.5 + GREETING_DELAY - 0.01)
            .is_empty());
        let events =
            source.observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, 102.5 + GREETING_DELAY);
        assert_eq!(kinds(&events), [HighIntensity]);
        assert_eq!(events[0].intensity, TYPING_INTENSITY);
    }

    // ------------------------------------------------------------ plan §6: 4

    /// A pet that has not been handed the greeting -- in the user's hand, or
    /// watching an agent that outranks this app. Its seat is ours and it owes
    /// nothing, which is exactly what "arrived" looked like before. (A resting
    /// pet is not sent the greeting at all until it wakes: plan §6 20a.)
    #[test]
    fn a_greeting_the_pet_has_not_been_handed_holds_the_beat() {
        let mut source = FocusActivity::new();
        let present = source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0);
        let seat = present[0].id.clone();
        let hop = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 101.0)[0].id.clone();

        let mut now = 101.0;
        while now < 105.0 {
            now += 0.5;
            assert!(
                source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, now).is_empty(),
                "the beat started before the pet had the hop, at {now}"
            );
        }
        // Another source's event is not ours either.
        now += 0.5;
        assert!(source
            .observe(Some(HWP), true, 0.2, Some("claude-code:session-1:9"), false, false, false, now)
            .is_empty());

        // Handed over at last.
        now += 0.5;
        assert!(source.observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, now).is_empty());
        let events = source.observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, now + GREETING_DELAY);
        assert_eq!(kinds(&events), [HighIntensity]);
    }

    // --------------------------------------------- plan §9 B3: kept from the pet

    /// And if it never is -- an agent is on duty, and the director lets
    /// nothing from the desk past it -- the hop is said again every
    /// `BLOCKED_RESEND` under a new id, so that attention still has it the
    /// moment the agent lets go. It never times out: nothing is on its way to
    /// the pet yet. Handed the latest one, the beat follows.
    #[test]
    fn a_hop_kept_from_the_pet_is_said_again_and_never_given_up() {
        let mut source = FocusActivity::new();
        let seat = source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0)[0].id.clone();
        let first = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 101.0);
        assert_eq!(kinds(&first), [ActivityStarted]);

        let mut hops = vec![first[0].id.clone()];
        let mut now = 101.0;
        while now < 101.0 + GREETING_TIMEOUT * 2.0 {
            now += 0.5;
            let events = source.observe(Some(HWP), true, 0.2, Some(AGENT_EVENT), false, false, false, now);
            match kinds(&events).as_slice() {
                [] => {}
                [ActivityStarted] => {
                    assert_eq!(now - 101.0, BLOCKED_RESEND * hops.len() as f64, "said again off the beat");
                    hops.push(events[0].id.clone());
                }
                other => panic!("said {other:?} at {now} while the hop was kept from the pet"),
            }
        }
        assert_eq!(hops.len(), 3, "the hop was not said again every {BLOCKED_RESEND} s: {hops:?}");
        let mut unique = hops.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), hops.len(), "said again under an old id");

        // Handed the latest one at last, already standing there.
        now += 0.5;
        let latest = hops.last().unwrap().clone();
        assert!(source.observe(Some(HWP), true, 0.2, Some(&latest), false, false, false, now).is_empty());
        let events =
            source.observe(Some(HWP), true, 0.2, Some(&latest), false, false, false, now + GREETING_DELAY);
        assert_eq!(kinds(&events), [HighIntensity]);
    }

    /// Handed over, and then never put on -- the walk failed, or an agent took
    /// the seat back on the way. The cap counts from the dispatch, not from
    /// the keystroke an agent kept waiting.
    #[test]
    fn the_cap_on_a_hop_counts_from_when_the_pet_was_handed_it() {
        let mut source = FocusActivity::new();
        let seat = source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0)[0].id.clone();
        let hop = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 101.0)[0].id.clone();

        let mut now = 101.0;
        while now < 111.0 {
            now += 0.5;
            assert!(source.observe(Some(HWP), true, 0.2, Some(AGENT_EVENT), false, false, false, now).is_empty());
        }
        let handed = now + 0.5;
        now = handed;
        while now < handed + GREETING_TIMEOUT {
            assert!(
                source.observe(Some(HWP), true, 0.2, Some(&hop), true, false, false, now).is_empty(),
                "gave up on the hop at {now}, counting from before the pet had it"
            );
            now += 0.5;
        }
        let events =
            source.observe(Some(HWP), true, 0.2, Some(&hop), true, false, false, handed + GREETING_TIMEOUT);
        assert_eq!(kinds(&events), [HighIntensity]);
    }

    /// Sitting beside the app while an agent has the pet: said again every
    /// `BLOCKED_RESEND`, each time under its own id, and no more once the pet
    /// has it.
    #[test]
    fn a_seat_kept_from_the_pet_is_said_again_until_the_pet_has_it() {
        let mut desk = Desk::new();
        desk.block();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.idle_for(BLOCKED_RESEND - 0.5), []);
        assert_eq!(desk.step(), [Present], "sitting was not said again");
        assert_eq!(desk.idle_for(BLOCKED_RESEND), [Present]);

        let mut ids: Vec<&str> = desk.said.iter().map(|event| event.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3, "said again under an old id");

        desk.unblock();
        assert_eq!(desk.idle_for(BLOCKED_RESEND * 3.0), [], "said again to a pet that had it");
    }

    /// An agent takes the pet mid-typing. Typing is said again for as long as
    /// the agent keeps it; the question and the release keep their own time;
    /// and a released seat is not said again.
    #[test]
    fn typing_kept_from_the_pet_is_said_again_but_a_released_seat_is_not() {
        let mut desk = Desk::seated();
        assert_eq!(desk.type_for(3.0), [ActivityStarted, HighIntensity]);
        desk.block();
        assert_eq!(desk.type_for(BLOCKED_RESEND - 0.5), []);
        assert_eq!(desk.type_for(0.5), [HighIntensity], "typing was not said again");
        assert_eq!(desk.type_for(BLOCKED_RESEND), [HighIntensity]);

        assert_eq!(
            desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0),
            [AttentionRequired, ActivityEnded]
        );
        assert_eq!(desk.idle_for(BLOCKED_RESEND * 4.0), [], "a released seat was said again");

        // Typing again, still kept from the pet: running, because this
        // session's hop was played, and said again like the rest.
        assert_eq!(desk.type_for(0.5), [HighIntensity]);
        assert_eq!(desk.type_for(BLOCKED_RESEND - 0.5), []);
        assert_eq!(desk.type_for(0.5), [HighIntensity]);
    }

    /// The first keystroke's hop, kept from the pet until the user stopped
    /// typing. Nobody saw it, so it is not spent: the question rather than a
    /// beat after a hop, and the first keystroke once the agent lets go hops.
    #[test]
    fn a_hop_kept_from_the_pet_is_still_owed() {
        let mut desk = Desk::new();
        desk.block();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.type_for(3.0), [ActivityStarted]);
        assert_eq!(
            desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0),
            [AttentionRequired, ActivityEnded],
            "the beat after the hop went out on a hop nobody saw"
        );

        desk.unblock();
        assert_eq!(desk.type_for(0.5), [ActivityStarted], "the hop was spent on a pet that never got it");
        assert_eq!(desk.type_for(3.0), [HighIntensity]);
        // And now it is spent.
        assert_eq!(
            desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0),
            [AttentionRequired, ActivityEnded]
        );
        assert_eq!(desk.type_for(0.5), [HighIntensity], "the session hopped twice");
    }

    /// Still typing when the agent lets go: the pet is handed the latest hop,
    /// and the beat waits for that one.
    #[test]
    fn a_hop_kept_from_the_pet_plays_when_the_seat_frees_mid_typing() {
        let mut desk = Desk::new();
        desk.block();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.type_for(BLOCKED_RESEND + 1.0), [ActivityStarted, ActivityStarted]);

        desk.unblock();
        assert_eq!(desk.dispatched.as_deref(), desk.said.last().map(|event| event.id.as_str()));
        assert_eq!(desk.type_for(3.0), [HighIntensity], "no beat after the hop");
        assert_eq!(
            desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0),
            [AttentionRequired, ActivityEnded]
        );
        assert_eq!(desk.type_for(0.5), [HighIntensity], "the session hopped twice");
    }

    /// Saying it again is seat news like any other: held back for a resting
    /// pet, and said once when it wakes.
    #[test]
    fn a_resting_pet_is_not_told_again_either() {
        let mut desk = Desk::new();
        desk.block();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        desk.resting = true;
        assert_eq!(desk.idle_for(BLOCKED_RESEND * 3.0), [], "said again to a sleeping pet");
        desk.resting = false;
        assert_eq!(desk.step(), [Present], "waking did not say it");
        assert_eq!(desk.idle_for(BLOCKED_RESEND - 0.5), [], "waking said it twice");
        assert_eq!(desk.step(), [Present]);
    }

    /// A walk long enough for the user to stop typing: the beat after the hop
    /// is the question, not a burst of running first.
    #[test]
    fn a_greeting_that_outlasts_the_typing_asks_instead() {
        let mut source = FocusActivity::new();
        let seat = source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0)[0].id.clone();
        let hop = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 101.0)[0].id.clone();
        let mut now = 101.0;
        while now < 113.0 {
            now += 0.5;
            assert!(source.observe(Some(HWP), true, now - 100.8, Some(&hop), true, false, false, now).is_empty());
        }
        assert!(source.observe(Some(HWP), true, now - 100.8, Some(&hop), false, false, false, now).is_empty());
        let at = now + GREETING_DELAY;
        let events = source.observe(Some(HWP), true, at - 100.8, Some(&hop), false, false, false, at);
        assert_eq!(kinds(&events), [AttentionRequired]);
    }

    // ------------------------------------------------------------ plan §6: 5

    #[test]
    fn a_later_burst_in_the_same_session_runs_without_a_hop() {
        let mut desk = Desk::seated();
        assert_eq!(desk.type_for(3.0), [ActivityStarted, HighIntensity]);
        assert_eq!(desk.idle_for(12.0), [AttentionRequired]);
        assert_eq!(desk.type_for(1.0), [HighIntensity], "the second burst hopped");
    }

    // ------------------------------------------------------------ plan §6: 6

    #[test]
    fn ten_quiet_seconds_ask_and_five_more_give_the_seat_back() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        // The last key was a moment before the last sample of the burst.
        assert_eq!(desk.idle_for(9.5), [], "a pause for thought ended the work picture");
        assert_eq!(desk.idle_for(0.5), [AttentionRequired]);
        assert_eq!(desk.said.last().unwrap().intensity, PRESENT_INTENSITY);
        assert_eq!(desk.idle_for(WAITING_BEFORE_RELEASE - 0.5), []);
        assert_eq!(desk.idle_for(0.5), [ActivityEnded]);
        assert_eq!(desk.source.seated_app(), Some(HWP), "a released seat is not the user leaving");

        // Typing again brings the pet back to work, without a hop.
        assert_eq!(desk.type_for(0.5), [HighIntensity]);
    }

    // ------------------------------------------------------------ plan §6: 7

    #[test]
    fn a_keystroke_while_asking_goes_back_to_work() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        assert_eq!(desk.idle_for(12.0), [AttentionRequired]);
        assert_eq!(desk.type_for(0.5), [HighIntensity]);
        assert_eq!(desk.idle_for(WAITING_BEFORE_RELEASE + 2.0), [], "the seat was released anyway");
    }

    // ------------------------------------------------------------ plan §6: 8

    #[test]
    fn a_released_seat_stays_quiet() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        assert_eq!(desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0), [
            AttentionRequired,
            ActivityEnded
        ]);
        assert_eq!(desk.idle_for(300.0), [], "a released seat kept talking");
    }

    // ------------------------------------------------------------ plan §6: 9

    #[test]
    fn a_long_break_starts_a_new_session() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
        assert_eq!(desk.idle_for(BREAK), []);

        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present], "coming back hopped before anyone typed");
        assert_eq!(desk.type_for(0.5), [ActivityStarted]);
    }

    // ----------------------------------------------------------- plan §6: 10

    #[test]
    fn a_short_break_keeps_the_session() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
        assert_eq!(desk.idle_for(60.0), []);

        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.type_for(0.5), [HighIntensity], "an errand earned a second hop");
    }

    /// Back inside the break, but nobody had typed before leaving: the session
    /// still owes its first hop.
    #[test]
    fn a_short_break_before_any_typing_still_owes_the_hop() {
        let mut desk = Desk::seated();
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.type_for(0.5), [ActivityStarted]);
    }

    // ----------------------------------------------------------- plan §6: 11

    #[test]
    fn switching_work_apps_ends_one_and_seats_the_other() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        desk.focus(Some(HSHOW));
        assert_eq!(desk.step(), [ActivityEnded, Present]);
        let n = desk.said.len();
        assert_eq!(desk.said[n - 2].source_id, "focus:Hwp.exe");
        assert_eq!(desk.said[n - 1].source_id, "focus:Hshow.exe");
        assert_eq!(desk.source.seated_app(), Some(HSHOW));

        assert_eq!(desk.type_for(0.5), [ActivityStarted], "the new app got no hop of its own");
        assert_eq!(desk.said.last().unwrap().source_id, "focus:Hshow.exe");
    }

    /// Leaving one work app for a browser and opening another is a different
    /// window, however short the gap.
    #[test]
    fn a_different_app_after_a_short_break_is_a_new_session() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE + 10.0), [ActivityEnded]);
        desk.focus(Some(HSHOW));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.type_for(0.5), [ActivityStarted]);
    }

    // ----------------------------------------------------------- plan §6: 12

    #[test]
    fn a_glance_elsewhere_costs_nothing_and_a_real_leave_ends_it() {
        let mut desk = Desk::seated();
        // Long enough for the beat after the hop to be out: a greeting still
        // owed is paid on the first sample back, glance or not.
        assert_eq!(desk.type_for(3.0), [ActivityStarted, HighIntensity]);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(2.0), []);
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [], "back inside the grace, and the pet never stood up");

        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE - 0.5), []);
        assert_eq!(desk.step(), [ActivityEnded]);
        assert_eq!(desk.said.last().unwrap().source_id, "focus:Hwp.exe");
        assert_eq!(desk.source.seated_app(), None);
    }

    /// Leaving ends the session the same way whatever the pet was doing.
    #[test]
    fn leaving_while_asking_or_released_ends_it_the_same_way() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        assert_eq!(desk.idle_for(12.0), [AttentionRequired]);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);

        let mut desk = Desk::seated();
        desk.type_for(3.0);
        assert_eq!(desk.idle_for(20.0), [AttentionRequired, ActivityEnded]);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
        assert_eq!(desk.source.seated_app(), None);
    }

    #[test]
    fn a_long_sitting_earns_a_wave_and_the_counter_resets() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [Achievement]);
        assert_eq!(desk.said.last().unwrap().intensity, WAVE_INTENSITY);
        // The pet has the wave by the next sample, and the sitting ends.
        assert_eq!(desk.step(), [ActivityEnded]);

        // Back within the break, and away again: no wave, the counter was spent.
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
    }

    /// Asking and a released seat are not typing. Each round below counts 14.5
    /// typing seconds -- 4.5 of keys, ten of the window after them -- then
    /// five asking and five released. Eleven rounds are 159.5 seconds of
    /// typing, short of the wave, and 214.5 with the asking: so a counter that
    /// strayed into either asking or the release would wave. Eight rounds were
    /// 116 and 156, and caught only the release (plan §9.3).
    #[test]
    fn asking_and_released_time_is_not_counted_toward_the_wave() {
        const ROUNDS: f64 = 11.0;
        let mut desk = Desk::seated();
        for _ in 0..ROUNDS as usize {
            desk.type_for(5.0);
            desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 5.0);
        }
        assert!(
            desk.source.typed_seconds + ROUNDS * WAITING_BEFORE_RELEASE > WAVE_AFTER_TYPING,
            "too few rounds for counting the asking to reach the wave"
        );
        // Straight out of the released seat, which waves like any other once
        // the stretch is long enough (plan §9.6a).
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
    }

    // ----------------------------------------------------------- plan §9.2

    /// A Cmd-Tab to another window and back is two keys. Neither is typing,
    /// and neither moves the typing window: the question comes ten seconds
    /// after the last key that was.
    #[test]
    fn a_keyboard_glance_away_does_not_restart_the_typing_window() {
        let mut desk = Desk::seated();
        assert_eq!(desk.type_for(3.0), [ActivityStarted, HighIntensity]);
        let last_typed = desk.last_key_at;
        assert_eq!(desk.idle_for(2.0), []);
        desk.press();
        desk.focus(Some(CHROME));
        assert_eq!(desk.step(), []);
        assert_eq!(desk.idle_for(1.5), []);
        desk.press();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [], "back inside the grace");

        let mut asked_at = None;
        while desk.now < last_typed + TYPING_WINDOW * 2.0 {
            let said = desk.step();
            assert!(said.iter().all(|kind| *kind == AttentionRequired), "said {said:?}");
            if !said.is_empty() {
                asked_at = Some(desk.now);
                break;
            }
        }
        let asked_at = asked_at.expect("never asked");
        assert!(
            asked_at >= last_typed + TYPING_WINDOW && asked_at < last_typed + TYPING_WINDOW + 0.5,
            "asked {} s after the last key typed into the app",
            asked_at - last_typed
        );
    }

    // ----------------------------------------------------------- plan §9.3

    /// The pet's own menu in front mid-sitting. Nothing is said while nobody
    /// can say what is in front -- not the question, not the release, not a
    /// heartbeat -- and nothing about the sitting is lost: the typing counter
    /// does not run, and the first sample back says what fell due meanwhile,
    /// once. The clocks are the wall's, not paused: a question that fell due
    /// in the menu is asked on return, and a release keeps its time.
    #[test]
    fn an_unknown_front_app_mid_sitting_says_nothing_and_loses_nothing() {
        // Typing, then the menu for longer than a heartbeat and the typing
        // window together.
        let mut desk = Desk::seated();
        assert_eq!(desk.type_for(3.0), [ActivityStarted, HighIntensity]);
        let typed = desk.source.typed_seconds;
        desk.focus(None);
        assert_eq!(desk.idle_for(HEARTBEAT + TYPING_WINDOW), [], "the menu bar was told something");
        assert_eq!(desk.source.seated_app(), Some(HWP));
        assert_eq!(desk.source.typed_seconds, typed, "the menu counted as typing");
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [AttentionRequired], "coming back did not pick up the pause");
        assert_eq!(desk.idle_for(WAITING_BEFORE_RELEASE), [ActivityEnded]);

        // Asking, then the menu for a moment: the release keeps its time.
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        assert_eq!(desk.idle_for(TYPING_WINDOW + 0.5), [AttentionRequired]);
        desk.focus(None);
        assert_eq!(desk.idle_for(2.0), []);
        desk.focus(Some(HWP));
        assert_eq!(desk.idle_for(WAITING_BEFORE_RELEASE - 3.0), []);
        assert_eq!(desk.step(), [ActivityEnded], "the menu moved the release");

        // Sitting, then the menu past a heartbeat: said once, on return.
        let mut desk = Desk::seated();
        desk.focus(None);
        assert_eq!(desk.idle_for(HEARTBEAT + 5.0), []);
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.idle_for(HEARTBEAT - 0.5), [], "the heartbeat came twice");
    }

    /// Typing for an hour is still one session, and the director has to be
    /// told so or it drops the seat at five minutes.
    #[test]
    fn a_typing_desk_says_so_every_minute_too() {
        let mut desk = Desk::seated();
        let said = desk.type_for(200.0);
        // The hop and the beat after it, then the three minutes' heartbeats.
        assert_eq!(said, [ActivityStarted, HighIntensity, HighIntensity, HighIntensity, HighIntensity]);
    }

    /// The director drops the seat after five minutes of silence, and a user
    /// reading a page types nothing for far longer.
    #[test]
    fn heartbeats_keep_the_seat_from_expiring() {
        let mut source = FocusActivity::new();
        let mut director = ActivityDirector::default();
        let mut now = 100.0;
        while now < 100.0 + 400.0 {
            now += 0.5;
            let dispatched = director.last_dispatched_id().map(str::to_owned);
            let pending = director.has_arrival_reaction();
            for event in source.observe(Some(HWP), true, QUIET, dispatched.as_deref(), pending, false, false, now) {
                director.handle_event(event, false, false, 0.5, now);
            }
            assert!(
                director.expire_silent(false, now).is_empty(),
                "the seat was given up at {now}"
            );
            assert_eq!(director.active_source_id(), Some("focus:Hwp.exe"));
        }
        // And the heartbeat is what did it: the same seat with nothing more
        // said does expire.
        assert!(now - 100.0 > ActivityLifetime::SILENCE_BEFORE_EXPIRY);
        let silent = director.expire_silent(false, now + ActivityLifetime::SILENCE_BEFORE_EXPIRY);
        assert!(!silent.is_empty(), "silence past the limit should end the watch");
    }

    /// The `ActivityEnded` after the wave must not take the celebration back.
    #[test]
    fn the_end_after_the_wave_does_not_cancel_it() {
        let mut desk = Desk::seated();
        let mut director = ActivityDirector::default();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        for event in desk.said.drain(..) {
            let at = event.timestamp;
            director.handle_event(event, false, false, 0.5, at);
        }
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [Achievement]);
        assert_eq!(desk.step(), [ActivityEnded]);

        let mut reactions = Vec::new();
        for event in desk.said.drain(..) {
            let at = event.timestamp;
            for effect in director.handle_event(event, false, false, 0.5, at) {
                if let ActivityEffect::ApplyReaction { reaction } = effect {
                    reactions.push(reaction);
                }
            }
        }
        assert_eq!(
            reactions,
            [CompanionReaction::SmallCelebrate],
            "the wave was followed by something that replaced it"
        );
        assert_eq!(director.active_source_id(), None);
    }

    /// The pet's own windows come through as `None`, and "unknown" must not
    /// read as "the user left". Opening the menu bar for three seconds used to
    /// end the session -- with the goodbye wave, if the sitting had been long
    /// enough.
    #[test]
    fn the_pets_own_app_in_front_is_not_the_user_leaving() {
        let mut desk = Desk::seated();
        desk.focus(None);
        assert_eq!(desk.idle_for(10.0), [], "the menu bar ended the session");
        assert_eq!(desk.source.seated_app(), Some(HWP));
        // Coming back is not an arrival, because nobody went anywhere -- and
        // the key that closed the menu is not typing.
        desk.press();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), []);
        assert_eq!(desk.idle_for(5.0), []);
        assert_eq!(desk.source.seated_app(), Some(HWP));
    }

    /// Leaving mid-greeting. The hop was owed and never paid, and the session
    /// must end cleanly anyway rather than leaving a beat to fire later.
    #[test]
    fn leaving_during_a_greeting_ends_it_without_a_beat() {
        let mut source = FocusActivity::new();
        let seat = source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0)[0].id.clone();
        let events = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 101.0);
        assert_eq!(kinds(&events), [ActivityStarted]);

        source.observe(Some(CHROME), false, 0.2, Some(&seat), false, false, false, 101.5);
        let events = source.observe(Some(CHROME), false, 3.2, Some(&seat), false, false, false, 104.5);
        assert_eq!(kinds(&events), [ActivityEnded]);

        let mut now = 104.5;
        while now < 130.0 {
            now += 0.5;
            assert!(
                source.observe(Some(CHROME), false, QUIET, None, false, false, false, now).is_empty(),
                "the abandoned greeting fired at {now}"
            );
        }
    }

    /// The whole sitting through a real director, located the way the shell
    /// locates it and worn as soon as it is owed: what the user is promised
    /// is what the pet is told to wear.
    #[test]
    fn a_sitting_through_the_director_wears_what_it_promises() {
        fn located(mut event: CompanionEvent) -> CompanionEvent {
            if wants_window_hint(event.kind) {
                event.location_hint = Some(LocationHint::new(
                    Some(WorldRect::new(200.0, 200.0, 700.0, 500.0)),
                    0.8,
                ));
            }
            event
        }
        fn worn(effects: Vec<ActivityEffect>, into: &mut Vec<CompanionReaction>) {
            for effect in effects {
                if let ActivityEffect::ApplyReaction { reaction } = effect {
                    into.push(reaction);
                }
            }
        }

        let mut source = FocusActivity::new();
        let mut director = ActivityDirector::default();
        let mut pictures = Vec::new();
        let mut now = 100.0;
        let mut last_key = now - QUIET;
        let run = |source: &mut FocusActivity,
                       director: &mut ActivityDirector,
                       pictures: &mut Vec<CompanionReaction>,
                       now: &mut f64,
                       last_key: &mut f64,
                       seconds: f64,
                       typing: bool| {
            let until = *now + seconds;
            while *now + 0.25 < until {
                if typing {
                    *last_key = *now + 0.1;
                }
                *now += 0.5;
                let dispatched = director.last_dispatched_id().map(str::to_owned);
                let pending = director.has_arrival_reaction();
                for event in
                    source.observe(Some(HWP), true, *now - *last_key, dispatched.as_deref(), pending, false, false, *now)
                {
                    worn(director.handle_event(located(event), false, false, 0.5, *now), pictures);
                }
                if director.has_arrival_reaction() {
                    worn(director.deliver_arrival_reaction(false, *now), pictures);
                }
            }
        };

        run(&mut source, &mut director, &mut pictures, &mut now, &mut last_key, 5.0, false);
        assert_eq!(pictures, [CompanionReaction::Calm], "the app alone dressed the pet");
        run(&mut source, &mut director, &mut pictures, &mut now, &mut last_key, 4.0, true);
        run(&mut source, &mut director, &mut pictures, &mut now, &mut last_key, 12.0, false);
        assert_eq!(director.active_source_id(), Some("focus:Hwp.exe"));
        run(&mut source, &mut director, &mut pictures, &mut now, &mut last_key, 5.0, false);
        assert_eq!(director.active_source_id(), None, "asking went on past the release");
        run(&mut source, &mut director, &mut pictures, &mut now, &mut last_key, 2.0, true);
        assert_eq!(
            pictures,
            [
                CompanionReaction::Calm,
                CompanionReaction::Spark,
                CompanionReaction::Work,
                CompanionReaction::Paw,
                CompanionReaction::Calm,
                CompanionReaction::Work,
            ]
        );
        assert_eq!(director.active_source_id(), Some("focus:Hwp.exe"));
    }

    // ---------------------------------------------------------- plan §6: 20a

    /// The first keystroke reaches the sample while the pet is still asleep:
    /// the key wakes it later in the same tick. A hop said then would be
    /// dropped, so it goes out on the first sample the pet is awake, and the
    /// beat after it is timed from there.
    #[test]
    fn a_hop_for_a_resting_pet_goes_out_when_it_wakes() {
        let mut source = FocusActivity::new();
        let seat =
            source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0)[0].id.clone();

        assert!(
            source.observe(Some(HWP), true, 0.2, Some(&seat), false, true, false, 101.0).is_empty(),
            "the hop was said to a sleeping pet"
        );
        assert!(source.observe(Some(HWP), true, 0.7, Some(&seat), false, true, false, 101.5).is_empty());

        let events = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, 102.0);
        assert_eq!(kinds(&events), [ActivityStarted], "waking did not bring the hop");
        let hop = events[0].id.clone();

        // Worn on the next sample; the beat follows the hop's own length.
        assert!(source.observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, 102.5).is_empty());
        let at = 102.5 + GREETING_DELAY;
        assert!(source
            .observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, at - 0.01)
            .is_empty());
        let events = source.observe(Some(HWP), true, 0.2, Some(&hop), false, false, false, at);
        assert_eq!(kinds(&events), [HighIntensity]);
    }

    /// The cap on waiting for a hop counts from when the pet was handed it --
    /// after it went out, which is after the keystroke the pet slept through.
    #[test]
    fn the_cap_on_a_held_back_hop_counts_from_when_the_pet_was_handed_it() {
        let mut source = FocusActivity::new();
        let seat =
            source.observe(Some(HWP), true, QUIET, None, false, false, false, 100.0)[0].id.clone();

        let mut now = 100.5;
        while now < 110.0 {
            now += 0.5;
            assert!(
                source.observe(Some(HWP), true, 0.2, Some(&seat), false, true, false, now).is_empty(),
                "something was said to a sleeping pet at {now}"
            );
        }
        now += 0.5;
        let events = source.observe(Some(HWP), true, 0.2, Some(&seat), false, false, false, now);
        assert_eq!(kinds(&events), [ActivityStarted]);
        let hop = events[0].id.clone();

        // Handed over on the next sample, and never put on.
        let handed = now + 0.5;
        now = handed;
        while now < handed + GREETING_TIMEOUT {
            assert!(
                source.observe(Some(HWP), true, 0.2, Some(&hop), true, false, false, now).is_empty(),
                "gave up on the hop at {now}, counting from before the pet had it"
            );
            now += 0.5;
        }
        let events =
            source.observe(Some(HWP), true, 0.2, Some(&hop), true, false, false, handed + GREETING_TIMEOUT);
        assert_eq!(kinds(&events), [HighIntensity]);
    }

    // ---------------------------------------------------------- plan §6: 20b

    /// The app comes forward while the pet sleeps: nothing, until it wakes.
    #[test]
    fn a_resting_pet_hears_about_the_app_when_it_wakes() {
        let mut desk = Desk::new();
        desk.resting = true;
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [], "a sleeping pet was told about the app");
        assert_eq!(desk.idle_for(5.0), []);
        assert_eq!(desk.source.seated_app(), Some(HWP), "the app was forgotten while the pet slept");

        desk.resting = false;
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.idle_for(10.0), [], "waking told the pet twice");
    }

    /// A nap beside someone reading: the minute's heartbeat is held back and
    /// goes out on waking -- and a nap with nothing held back wakes to silence.
    #[test]
    fn a_heartbeat_held_back_by_a_nap_goes_out_on_waking() {
        let mut desk = Desk::seated();
        desk.resting = true;
        assert_eq!(desk.idle_for(HEARTBEAT + 10.0), [], "a heartbeat reached a sleeping pet");
        desk.resting = false;
        assert_eq!(desk.step(), [Present]);

        desk.resting = true;
        assert_eq!(desk.idle_for(5.0), []);
        desk.resting = false;
        assert_eq!(desk.idle_for(5.0), [], "a nap with nothing held back woke to news");
    }

    /// Plan §9.3. The question falls due while the pet naps: held back, and
    /// asked once when it wakes. The release keeps the time the question
    /// began, not the time it was finally told.
    #[test]
    fn a_question_held_back_by_a_nap_is_asked_once_on_waking() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        assert_eq!(desk.idle_for(TYPING_WINDOW - 0.5), []);
        desk.resting = true;
        assert_eq!(desk.step(), [], "the question reached a sleeping pet");
        let asked = desk.now;
        assert_eq!(desk.step(), []);
        desk.resting = false;
        assert_eq!(desk.step(), [AttentionRequired], "waking did not ask");
        while desk.now + 0.5 < asked + WAITING_BEFORE_RELEASE {
            assert_eq!(desk.step(), [], "waking asked twice, or released early, at {}", desk.now);
        }
        assert_eq!(desk.step(), [ActivityEnded], "the release waited for the late question");
    }

    /// Plan §9.3. From one work app to another while the pet naps: the end of
    /// the first is said at once, like any leaving; the second is seat news,
    /// said when the pet wakes -- and a new session, with its own hop.
    #[test]
    fn switching_work_apps_while_the_pet_rests() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        desk.resting = true;
        desk.focus(Some(HSHOW));
        assert_eq!(desk.step(), [ActivityEnded], "the switch waited for the pet, or told it the new app");
        assert_eq!(desk.said.last().unwrap().source_id, "focus:Hwp.exe");
        assert_eq!(desk.source.seated_app(), Some(HSHOW));
        assert_eq!(desk.idle_for(5.0), []);

        desk.resting = false;
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.said.last().unwrap().source_id, "focus:Hshow.exe");
        assert_eq!(desk.idle_for(5.0), [], "waking told the pet twice");
        assert_eq!(desk.type_for(0.5), [ActivityStarted], "the new app got no hop of its own");
    }

    // ---------------------------------------------------------- plan §6: 20c

    /// The seat was given back, the pet dozed off, and the user typed again.
    #[test]
    fn typing_back_to_a_resting_pet_waits_for_it_to_wake() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        assert_eq!(desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0), [
            AttentionRequired,
            ActivityEnded
        ]);
        desk.resting = true;
        assert_eq!(desk.idle_for(30.0), []);
        assert_eq!(desk.type_for(1.0), [], "a sleeping pet was told to work");

        desk.resting = false;
        desk.press();
        assert_eq!(desk.step(), [HighIntensity], "waking did not bring the pet back to work");
        assert_eq!(desk.type_for(5.0), [], "waking sent the work picture twice");
    }

    // ---------------------------------------------------------- plan §6: 20d

    /// Leaving is said at once, asleep or not: the director ends a seat before
    /// it looks at rest, and the wave is for leaving. What was held back for
    /// the seat is not said afterwards -- there is no seat. (After a wave the
    /// end waits for the pet to have the wave, not for it to wake: plan §9.4.)
    #[test]
    fn leaving_is_said_to_a_resting_pet_at_once() {
        let mut desk = Desk::seated();
        desk.type_for(3.0);
        desk.resting = true;
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded], "leaving waited for the pet to wake");

        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        desk.resting = true;
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [Achievement], "the wave waited for the pet to wake");
        assert_eq!(desk.step(), [ActivityEnded]);

        let mut desk = Desk::seated();
        desk.resting = true;
        assert_eq!(desk.idle_for(HEARTBEAT + 1.0), []);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
        desk.resting = false;
        assert_eq!(desk.idle_for(5.0), [], "news held back for a seat went out after leaving it");
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        assert_eq!(desk.idle_for(5.0), []);
    }

    // ----------------------------------------------------------- plan §9.4

    /// Three minutes typed at this seat, and the user leaves: the wave goes
    /// out, and the end of the sitting waits until the pet has been handed the
    /// wave -- a resting pet is woken for it and gets it only once it stands
    /// idle -- and then goes out once.
    #[test]
    fn the_end_after_a_wave_waits_for_the_pet_to_have_the_wave() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        let seat = desk.dispatched.clone().expect("the pet acts on nothing");
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE - 0.5), []);

        let mut now = desk.now + 0.5;
        let events = desk.source.observe(Some(CHROME), false, QUIET, Some(&seat), false, false, false, now);
        assert_eq!(kinds(&events), [Achievement]);
        let wave = events[0].id.clone();
        let waved = now;

        // Getting up to play it.
        while now < waved + 2.0 {
            now += 0.5;
            assert!(
                desk.source.observe(Some(CHROME), false, QUIET, Some(&seat), false, false, false, now).is_empty(),
                "the end went ahead of a wave the pet did not have yet, at {now}"
            );
        }
        now += 0.5;
        let events = desk.source.observe(Some(CHROME), false, QUIET, Some(&wave), false, false, false, now);
        assert_eq!(kinds(&events), [ActivityEnded], "the pet had the wave and the sitting went on");
        assert_eq!(events[0].source_id, "focus:Hwp.exe");

        while now < waved + WAVE_HOLD_TIMEOUT * 3.0 {
            now += 0.5;
            assert!(
                desk.source.observe(Some(CHROME), false, QUIET, Some(&wave), false, false, false, now).is_empty(),
                "the sitting ended twice, at {now}"
            );
        }
    }

    // ----------------------------------------------------------- plan §9.6a

    /// The seat went back after the question and the pet is off roaming --
    /// the director forgot what it last acted on -- and the user leaves after
    /// a long stretch. The wave goes out all the same, where the pet is, and
    /// the end follows once the pet has it. The stretch is spent: back within
    /// the break and away again is no second wave.
    #[test]
    fn leaving_a_released_seat_after_a_long_stretch_waves() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        assert_eq!(
            desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0),
            [AttentionRequired, ActivityEnded]
        );
        desk.dispatched = None;
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [Achievement], "no wave out of a released seat");
        assert_eq!(desk.said.last().unwrap().intensity, WAVE_INTENSITY);
        // The pet has the wave by the next sample, and the sitting ends.
        assert_eq!(desk.step(), [ActivityEnded]);
        assert_eq!(desk.said.last().unwrap().source_id, "focus:Hwp.exe");

        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded], "the same stretch waved twice");
    }

    /// Just short of the stretch, out of a released seat: the sitting ends
    /// and nothing else.
    #[test]
    fn a_stretch_just_short_of_the_wave_leaves_without_one() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING - TYPING_WINDOW - 1.0);
        assert_eq!(
            desk.idle_for(TYPING_WINDOW + WAITING_BEFORE_RELEASE + 1.0),
            [AttentionRequired, ActivityEnded]
        );
        let typed = desk.source.typed_seconds;
        assert!(
            typed < WAVE_AFTER_TYPING && typed > WAVE_AFTER_TYPING - 2.0,
            "not just short of the wave: {typed}"
        );
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded]);
    }

    /// The wave does not read what the pet last acted on: the id a finished
    /// agent leaves behind outlives its turn, and it kept a free pet from
    /// waving. Whether an agent is still on duty is asked of the director
    /// itself (`a_long_stretch_left_beside_an_agent_on_duty_ends_without_a_wave`).
    #[test]
    fn the_source_waves_whatever_the_pet_last_acted_on() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        // An agent had the pet and finished: its event is the last one acted
        // on, and nothing keeps the desk from the pet any more.
        desk.dispatched = Some(AGENT_EVENT.to_owned());
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [Achievement], "no wave with an agent's event last");
        assert_eq!(desk.step(), [ActivityEnded], "the end did not follow the wave");

        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded], "the same stretch waved twice");
    }

    /// Plan §9.7a. A long stretch left while an agent is on duty ends without
    /// a wave, and the stretch is spent. The director would keep a wave from
    /// the pet then but not forget it: it waited in `recent` for the end
    /// behind it, and an agent finishing meanwhile handed it to the pet late.
    #[test]
    fn a_long_stretch_left_beside_an_agent_on_duty_ends_without_a_wave() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        desk.block();
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded], "a wave beside an agent on duty");
        assert_eq!(desk.source.typed_seconds, 0.0, "the stretch was not spent");
        assert_eq!(desk.idle_for(WAVE_HOLD_TIMEOUT * 2.0), [], "the sitting ended twice");

        desk.unblock();
        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [Present]);
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [ActivityEnded], "the stretch waved once the agent was gone");
    }

    /// A wave the pet is never handed -- in the user's hand, or given
    /// something else first. The sitting ends anyway when the cap is up, with
    /// the pet's own menu in front meanwhile or not.
    #[test]
    fn a_wave_the_pet_never_gets_holds_the_end_for_the_cap_at_most() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        let seat = desk.dispatched.clone().expect("the pet acts on nothing");
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE - 0.5), []);

        let mut now = desk.now + 0.5;
        let waved = now;
        let events = desk.source.observe(Some(CHROME), false, QUIET, Some(&seat), false, false, false, now);
        assert_eq!(kinds(&events), [Achievement]);
        while now + 0.5 < waved + WAVE_HOLD_TIMEOUT {
            now += 0.5;
            assert!(
                desk.source.observe(None, false, QUIET, Some(&seat), false, false, false, now).is_empty(),
                "gave up on the wave {} s after it", now - waved
            );
        }
        now += 0.5;
        assert_eq!(now - waved, WAVE_HOLD_TIMEOUT);
        let events = desk.source.observe(None, false, QUIET, Some(&seat), false, false, false, now);
        assert_eq!(kinds(&events), [ActivityEnded], "the sitting never ended");
    }

    /// Back at the app before the pet has the wave: the old sitting's end is
    /// said first, then the arrival -- and it is the same session, inside the
    /// break.
    #[test]
    fn coming_back_before_the_wave_is_had_ends_the_old_sitting_first() {
        let mut desk = Desk::seated();
        desk.type_for(WAVE_AFTER_TYPING + 5.0);
        let seat = desk.dispatched.clone().expect("the pet acts on nothing");
        desk.focus(Some(CHROME));
        assert_eq!(desk.idle_for(LEAVE), [Achievement]);
        // Still getting up to play it.
        desk.dispatched = Some(seat);

        desk.focus(Some(HWP));
        assert_eq!(desk.step(), [ActivityEnded, Present]);
        let n = desk.said.len();
        assert_eq!(desk.said[n - 2].source_id, "focus:Hwp.exe");
        assert_eq!(desk.idle_for(WAVE_HOLD_TIMEOUT * 2.0), [], "the old sitting ended twice");
        assert_eq!(desk.type_for(0.5), [HighIntensity], "an errand earned a second hop");
    }

    /// Through a real director, awake and asleep. A pet asleep at the seat is
    /// woken for the wave and handed it only once it stands idle; an end said
    /// on the first awake sample took the queued wave back and sat the pet
    /// down in `Calm`. Now the wave is worn either way, the end follows on the
    /// next sample, and the end replaces nothing.
    #[test]
    fn a_wave_through_the_director_is_worn_before_the_seat_is_given_back() {
        /// `Wake`, then `Stretch`: how long a woken pet takes to stand idle.
        const GETTING_UP: f64 = 1.7;

        for asleep in [false, true] {
            let mut source = FocusActivity::new();
            let mut director = ActivityDirector::default();
            let mut now = 100.0;
            let mut last_key = now - QUIET;
            let mut resting = false;
            let mut woke_at: Option<f64> = None;
            let mut worn_after_leaving = Vec::new();
            let mut ended_at = Vec::new();

            let sat_until = 105.0;
            let left_at = sat_until + WAVE_AFTER_TYPING + 5.0;
            while now < left_at + FOCUS_GRACE + WAVE_HOLD_TIMEOUT * 2.0 {
                now += 0.5;
                // Asleep in the seat through the whole stretch: only the
                // keyboard clock moves, and nothing but the wave wakes it.
                if asleep && now > sat_until && woke_at.is_none() {
                    resting = true;
                }
                if now > sat_until && now <= left_at {
                    last_key = now - 0.1;
                }
                let front = if now <= left_at { HWP } else { CHROME };

                // The sample, then the tick: the order `RoamlingRuntime::tick` has.
                let dispatched = director.last_dispatched_id().map(str::to_owned);
                let pending = director.has_arrival_reaction();
                let on_duty = director.agent_on_duty(now);
                let mut effects = Vec::new();
                for event in source.observe(
                    Some(front),
                    front == HWP,
                    now - last_key,
                    dispatched.as_deref(),
                    pending,
                    resting,
                    on_duty,
                    now,
                ) {
                    if front == CHROME && event.kind == ActivityEnded {
                        ended_at.push(now);
                    }
                    effects.extend(director.handle_event(located(event), false, resting, 0.5, now));
                }
                if effects.contains(&ActivityEffect::CancelRest) && resting {
                    resting = false;
                    woke_at = Some(now);
                }
                let idle = !resting && woke_at.is_none_or(|at| now - at >= GETTING_UP);
                effects.extend(director.resume_pending_if_ready(idle, false, resting, 0.5, now));
                if director.has_arrival_reaction() {
                    effects.extend(director.deliver_arrival_reaction(resting, now));
                }
                if front == CHROME {
                    for effect in effects {
                        if let ActivityEffect::ApplyReaction { reaction } = effect {
                            worn_after_leaving.push((now, reaction));
                        }
                    }
                }
            }

            let worn: Vec<CompanionReaction> =
                worn_after_leaving.iter().map(|(_, reaction)| *reaction).collect();
            assert_eq!(worn, [CompanionReaction::SmallCelebrate], "asleep {asleep}: the wave was lost");
            let celebrated_at = worn_after_leaving[0].0;
            if asleep {
                let woke_at = woke_at.expect("the wave did not wake the pet");
                assert!(celebrated_at - woke_at >= GETTING_UP, "handed the wave before it stood idle");
            }
            assert_eq!(ended_at, [celebrated_at + 0.5], "asleep {asleep}: the end did not follow the wave");
            assert_eq!(director.active_source_id(), None, "asleep {asleep}: the seat was kept");
        }
    }

    /// Plan §9.6a and §9.7a, through a real director. An agent works at its
    /// own window, a tool call every ten seconds, while the user types three
    /// minutes into the app beside it and then leaves. Nothing the desk says
    /// is acted on beside the working agent -- not the hop, the running or the
    /// question -- and the leave says only the end, at once: no wave while an
    /// agent is on duty. When the agent finishes a couple of seconds later,
    /// well inside the age attention still counts what the desk said, there
    /// is nothing of the desk's left to queue.
    #[test]
    fn beside_a_working_agent_a_long_sitting_ends_at_once_without_a_wave() {
        use crate::attention::AttentionConfiguration;

        let mut source = FocusActivity::new();
        let mut director = ActivityDirector::default();
        let mut acted_on = Vec::new();
        let mut agent_said = 0;
        director.handle_event(agent_says(agent_said, HighIntensity, 100.0), false, false, 0.5, 100.0);
        director.deliver_arrival_reaction(false, 100.0);
        note_acted_on(&director, &mut acted_on);

        let typing_from = 105.0;
        let left_at = typing_from + WAVE_AFTER_TYPING + 5.0;
        let mut said_after_leaving = Vec::new();
        let mut ended_at: Option<f64> = None;
        let mut agent_done_at: Option<f64> = None;
        let mut now = 100.0;
        while now < left_at + 40.0 {
            now += 0.5;
            // The agent: a tool call every ten seconds, until it finishes a
            // couple of seconds after the desk's goodbye.
            if agent_done_at.is_none() {
                let finishing = ended_at.is_some_and(|ended| now >= ended + 2.0);
                if finishing || (now - 100.0) % 10.0 == 0.0 {
                    agent_said += 1;
                    let kind = if finishing { Achievement } else { HighIntensity };
                    director.handle_event(agent_says(agent_said, kind, now), false, false, 0.5, now);
                    note_acted_on(&director, &mut acted_on);
                    if finishing {
                        agent_done_at = Some(now);
                    }
                }
            }

            // The desk, sampled the way the shell samples it.
            let front = if now <= left_at { HWP } else { CHROME };
            let last_key = if now > typing_from && now <= left_at { now - 0.1 } else { 100.0 - QUIET };
            let dispatched = director.last_dispatched_id().map(str::to_owned);
            let pending = director.has_arrival_reaction();
            let on_duty = director.agent_on_duty(now);
            for event in source.observe(
                Some(front),
                front == HWP,
                now - last_key,
                dispatched.as_deref(),
                pending,
                false,
                on_duty,
                now,
            ) {
                if front == CHROME {
                    said_after_leaving.push(event.kind);
                    if event.kind == ActivityEnded {
                        ended_at = Some(now);
                    }
                }
                director.handle_event(located(event), false, false, 0.5, now);
                note_acted_on(&director, &mut acted_on);
            }
            director.resume_pending_if_ready(true, false, false, 0.5, now);
            note_acted_on(&director, &mut acted_on);
            if director.has_arrival_reaction() {
                director.deliver_arrival_reaction(false, now);
            }
        }

        assert_eq!(said_after_leaving, [ActivityEnded], "the leave said more than the end");
        let ended = ended_at.expect("the sitting never ended");
        assert_eq!(ended, left_at + 0.5 + FOCUS_GRACE, "the end waited for something");
        assert!(
            acted_on.iter().all(|id| id.starts_with(AGENT_SOURCE)),
            "the desk reached the pet beside a working agent: {acted_on:?}"
        );
        let done = agent_done_at.expect("the agent never finished");
        assert!(
            done - ended < AttentionConfiguration::default().maximum_event_age,
            "the agent finished too late for anything the desk left in the director to be queued"
        );
        assert_eq!(
            acted_on.last(),
            Some(&format!("{AGENT_SOURCE}:{agent_said}")),
            "something was acted on after the agent finished: {acted_on:?}"
        );
        assert_eq!(director.active_source_id(), None);
    }

    /// Plan §9.7a, the window the 9.6 review found: the same sitting, but the
    /// agent finishes about a second after the user has left. Had the desk
    /// waved, the director would have passed the wave over and kept it in
    /// `recent` for the end behind it, up to `WAVE_HOLD_TIMEOUT` later; the
    /// agent's finish queued it, and the pet wore the desk's reaction as soon
    /// as it was done celebrating the agent. Nothing of the desk's is worn in
    /// the ten seconds after the agent finishes.
    #[test]
    fn an_agent_finishing_just_after_the_user_leaves_brings_nothing_of_the_desk_late() {
        use crate::behavior::timing::CELEBRATE;

        let mut source = FocusActivity::new();
        let mut director = ActivityDirector::default();
        let mut acted_on = Vec::new();
        let mut agent_said = 0;
        director.handle_event(agent_says(agent_said, HighIntensity, 100.0), false, false, 0.5, 100.0);
        director.deliver_arrival_reaction(false, 100.0);

        let typing_from = 105.0;
        let left_at = typing_from + WAVE_AFTER_TYPING + 5.0;
        let mut desk_left_at: Option<f64> = None;
        let mut agent_done_at: Option<f64> = None;
        // The pet celebrating the agent: nothing queued is handed over until
        // it stands idle again.
        let mut busy_until = f64::NEG_INFINITY;
        let mut worn_after_agent = Vec::new();
        let mut now = 100.0;
        while now < left_at + 60.0 && agent_done_at.is_none_or(|done| now + 0.5 <= done + 10.0) {
            now += 0.5;
            // The agent: a tool call every ten seconds, then its finish a
            // second after the desk said the user left. What the agent's own
            // events make the pet wear is the agent's, and not collected.
            if agent_done_at.is_none() {
                let finishing = desk_left_at.is_some_and(|left| now >= left + 1.0);
                if finishing || (now - 100.0) % 10.0 == 0.0 {
                    agent_said += 1;
                    let kind = if finishing { Achievement } else { HighIntensity };
                    director.handle_event(agent_says(agent_said, kind, now), false, false, 0.5, now);
                    if finishing {
                        agent_done_at = Some(now);
                        busy_until = now + CELEBRATE;
                    }
                }
            }

            // The desk, and everything that can hand the pet something of it.
            let front = if now <= left_at { HWP } else { CHROME };
            let last_key = if now > typing_from && now <= left_at { now - 0.1 } else { 100.0 - QUIET };
            let dispatched = director.last_dispatched_id().map(str::to_owned);
            let pending = director.has_arrival_reaction();
            let on_duty = director.agent_on_duty(now);
            let mut effects = Vec::new();
            for event in source.observe(
                Some(front),
                front == HWP,
                now - last_key,
                dispatched.as_deref(),
                pending,
                false,
                on_duty,
                now,
            ) {
                if front == CHROME && desk_left_at.is_none() {
                    desk_left_at = Some(now);
                }
                effects.extend(director.handle_event(located(event), false, false, 0.5, now));
            }
            effects.extend(director.resume_pending_if_ready(now >= busy_until, false, false, 0.5, now));
            if director.has_arrival_reaction() {
                effects.extend(director.deliver_arrival_reaction(false, now));
            }
            note_acted_on(&director, &mut acted_on);
            if agent_done_at.is_some() {
                worn_after_agent.extend(effects.into_iter().filter_map(|effect| match effect {
                    ActivityEffect::ApplyReaction { reaction } => Some((now, reaction)),
                    _ => None,
                }));
            }
        }

        let left = desk_left_at.expect("the user never left");
        let done = agent_done_at.expect("the agent never finished");
        assert!(
            done > left && done - left <= 1.0,
            "the agent did not finish about a second after the user left: {left} -> {done}"
        );
        assert!(now >= done + 10.0, "watched only {} s after the agent finished", now - done);
        assert!(
            worn_after_agent.is_empty(),
            "the desk was worn after the agent finished: {worn_after_agent:?}"
        );
        assert!(
            acted_on.iter().all(|id| id.starts_with(AGENT_SOURCE)),
            "the desk reached the pet: {acted_on:?}"
        );
        assert_eq!(director.active_source_id(), None, "the seat was taken after the agent finished");
    }

    #[test]
    fn an_app_nobody_asked_for_is_not_an_event() {
        let mut source = FocusActivity::new();
        for step in 0..40 {
            let now = 100.0 + f64::from(step) * 0.5;
            assert!(source.observe(Some(CHROME), false, 0.2, None, false, false, false, now).is_empty());
        }
        assert_eq!(source.seated_app(), None);
        // It is still offered in the menu: the user has to be able to say
        // that this one is work.
        assert_eq!(source.recent_apps(), [CHROME]);
    }

    #[test]
    fn the_recent_list_is_ordered_short_and_free_of_repeats() {
        let mut source = FocusActivity::new();
        let apps = ["a", "b", "c", "d", "e", "f", "g"];
        let mut now = 100.0;
        for app in apps {
            now += 1.0;
            source.observe(Some(app), false, QUIET, None, false, false, false, now);
            now += 1.0;
            source.observe(Some(app), false, QUIET, None, false, false, false, now);
        }
        assert_eq!(source.recent_apps(), ["g", "f", "e", "d", "c", "b"]);

        // Seeing an old one again moves it to the front rather than doubling.
        source.observe(Some("c"), false, QUIET, None, false, false, false, now + 1.0);
        assert_eq!(source.recent_apps(), ["c", "g", "f", "e", "d", "b"]);

        // This app itself is never in the list: the shell answers None for it.
        source.observe(None, false, QUIET, None, false, false, false, now + 2.0);
        assert_eq!(source.recent_apps(), ["c", "g", "f", "e", "d", "b"]);
    }
}
