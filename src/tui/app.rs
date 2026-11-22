use std::{
    io::Write,
    time::{Duration, Instant},
};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use zbus::blocking::Connection;

use crate::{
    guard::inhibit::{self, InhibitorRow},
    session::{Options, Session},
    time,
};

pub const PRESETS: [(&str, Option<u64>); 7] = [
    ("10m", Some(600)),
    ("30m", Some(1800)),
    ("1h", Some(3600)),
    ("2h", Some(7200)),
    ("4h", Some(14400)),
    ("8h", Some(28800)),
    ("∞", None),
];
const DEFAULT_PRESET: usize = 2;
const STEP_MIN: i64 = 5;
const POLL_EVERY: Duration = Duration::from_secs(2);
const TOAST_FOR: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Choice {
    Preset(usize),
    Custom(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputKind {
    Duration,
    Reason,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    Help,
    Input { kind: InputKind, buf: String },
    ConfirmQuit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Info,
    Ok,
    Warn,
    Err,
}

/// Foto del temporizador para dibujar (separada para poder simularla en tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct Status {
    pub active: bool,
    pub remaining: Option<Duration>,
    pub elapsed: Duration,
    pub span: Option<Duration>,
    pub screen: bool,
}

pub struct App {
    pub session: Session,
    pub opts: Options,
    pub choice: Choice,
    pub inhibitors: Vec<InhibitorRow>,
    pub toast: Option<(String, Level, Instant)>,
    pub modal: Option<Modal>,
    pub env: String,
    pub tick: u64,
    pub quit: bool,
    bus: Option<Connection>,
    bus_failed: bool,
    last_poll: Option<Instant>,
}

impl App {
    pub fn new(opts: Options, initial: Option<Option<Duration>>) -> Self {
        let choice = match initial {
            None => Choice::Preset(DEFAULT_PRESET),
            Some(None) => Choice::Preset(PRESETS.len() - 1),
            Some(Some(d)) => PRESETS
                .iter()
                .position(|(_, s)| *s == Some(d.as_secs()))
                .map_or(Choice::Custom(d), Choice::Preset),
        };
        Self {
            session: Session::default(),
            opts,
            choice,
            inhibitors: Vec::new(),
            toast: None,
            modal: None,
            env: detect_env(),
            tick: 0,
            quit: false,
            bus: None,
            bus_failed: false,
            last_poll: None,
        }
    }

    pub fn selected(&self) -> Option<Duration> {
        match self.choice {
            Choice::Preset(i) => PRESETS[i].1.map(Duration::from_secs),
            Choice::Custom(d) => Some(d),
        }
    }

    pub fn status(&self) -> Status {
        Status {
            active: self.session.active(),
            remaining: self.session.remaining(),
            elapsed: self.session.elapsed(),
            span: self.session.span(),
            screen: self.session.screen_on(),
        }
    }

    fn say(&mut self, msg: impl Into<String>, lvl: Level) {
        self.toast = Some((msg.into(), lvl, Instant::now()));
    }

    /// Trabajo periódico: vencimiento, lista de inhibidores, toasts.
    pub fn update(&mut self) {
        if self.session.expired() {
            self.session.stop();
            self.say("tiempo cumplido · bloqueo liberado", Level::Ok);
            let _ = std::io::stdout().write_all(b"\x07");
        }
        if self.last_poll.is_none_or(|t| t.elapsed() >= POLL_EVERY) {
            self.last_poll = Some(Instant::now());
            self.poll_inhibitors();
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, _, t)| t.elapsed() >= TOAST_FOR)
        {
            self.toast = None;
        }
    }

    fn poll_inhibitors(&mut self) {
        if self.bus.is_none() {
            match Connection::system() {
                Ok(c) => self.bus = Some(c),
                Err(e) => {
                    if !self.bus_failed {
                        self.bus_failed = true;
                        self.say(format!("sin acceso a D-Bus: {e}"), Level::Err);
                    }
                    return;
                }
            }
        }
        if let Some(conn) = &self.bus {
            match inhibit::list_on(conn) {
                Ok(mut rows) => {
                    rows.sort_by_key(|r| r.1.to_lowercase());
                    self.inhibitors = rows;
                }
                Err(_) => self.bus = None, // reconecta en el próximo ciclo
            }
        }
    }

    // ───────────────────────────── acciones ─────────────────────────────

    fn start(&mut self) {
        let was = self.session.active();
        let res = self.session.start(&self.opts, self.selected());
        self.sync_screen();
        match res {
            Ok(()) => {
                let verb = if was {
                    "contador reiniciado"
                } else {
                    "bloqueo activo"
                };
                self.say(
                    format!("{verb} · {}", time::fmt_short(self.selected())),
                    Level::Ok,
                );
            }
            Err(e) => self.say(format!("error: {e}"), Level::Err),
        }
        self.last_poll = None;
    }

    fn stop(&mut self) {
        if self.session.active() {
            self.session.stop();
            self.say("bloqueo liberado · configuración restaurada", Level::Ok);
            self.last_poll = None;
        } else {
            self.say("no hay bloqueo activo", Level::Warn);
        }
    }

    fn pick(&mut self, choice: Choice) {
        self.choice = choice;
        if self.session.active() {
            self.start();
        } else {
            self.say(
                format!("duración · {}", time::fmt_short(self.selected())),
                Level::Info,
            );
        }
    }

    fn bump(&mut self, minutes: i64) {
        let sign = if minutes > 0 { "+" } else { "−" };
        if self.session.active() {
            if self.session.bump(minutes) {
                self.say(format!("{sign}{} min", minutes.abs()), Level::Info);
            } else {
                self.say("sin límite · elige una duración para usar ±", Level::Warn);
            }
            return;
        }
        let Some(cur) = self.selected() else {
            self.say("sin límite · elige una duración para usar ±", Level::Warn);
            return;
        };
        let step = Duration::from_secs(minutes.unsigned_abs() * 60);
        let next = if minutes > 0 {
            (cur + step).min(Duration::from_secs(365 * 86400))
        } else {
            cur.saturating_sub(step).max(Duration::from_secs(300))
        };
        self.choice = PRESETS
            .iter()
            .position(|(_, s)| *s == Some(next.as_secs()))
            .map_or(Choice::Custom(next), Choice::Preset);
    }

    fn toggle(&mut self, screen: bool) {
        if screen {
            self.opts.screen = !self.opts.screen;
        } else {
            self.opts.lid = !self.opts.lid;
        }
        let res = if self.session.active() {
            self.session.apply(&self.opts)
        } else {
            Ok(())
        };
        self.sync_screen();
        match res {
            Err(e) => self.say(format!("error: {e}"), Level::Err),
            Ok(()) => {
                let msg = match (
                    screen,
                    screen && self.opts.screen || !screen && self.opts.lid,
                ) {
                    (true, true) => "pantalla · siempre encendida",
                    (true, false) => "pantalla · comportamiento normal",
                    (false, true) => "tapa · cerrarla no suspende",
                    (false, false) => "tapa · cerrarla suspende",
                };
                self.say(msg, Level::Info);
            }
        }
        self.last_poll = None;
    }

    /// Si la pantalla no se pudo tomar, la opción refleja la realidad.
    fn sync_screen(&mut self) {
        if self.session.active() {
            self.opts.screen = self.session.screen_on();
        }
    }

    // ───────────────────────────── teclado ─────────────────────────────

    pub fn on_key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if let Some(m) = self.modal.take() {
            self.modal = self.on_modal_key(m, k);
            return;
        }
        match k.code {
            KeyCode::Char('q') => {
                if self.session.active() {
                    self.modal = Some(Modal::ConfirmQuit);
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Enter | KeyCode::Char('s') => self.start(),
            KeyCode::Char('x') => self.stop(),
            KeyCode::Char('+' | '=') | KeyCode::Right => self.bump(STEP_MIN),
            KeyCode::Char('-') | KeyCode::Left => self.bump(-STEP_MIN),
            KeyCode::Char(c @ '1'..='7') => self.pick(Choice::Preset(c as usize - '1' as usize)),
            KeyCode::Char('c' | 'd') => {
                self.modal = Some(Modal::Input {
                    kind: InputKind::Duration,
                    buf: String::new(),
                });
            }
            KeyCode::Char('r') => {
                self.modal = Some(Modal::Input {
                    kind: InputKind::Reason,
                    buf: self.opts.reason.clone(),
                });
            }
            KeyCode::Char('p') => self.toggle(true),
            KeyCode::Char('t') => self.toggle(false),
            KeyCode::Char('?') | KeyCode::F(1) => self.modal = Some(Modal::Help),
            _ => {}
        }
    }

    fn on_modal_key(&mut self, m: Modal, k: KeyEvent) -> Option<Modal> {
        match m {
            Modal::Help => None,
            Modal::ConfirmQuit => match k.code {
                KeyCode::Char('s' | 'S' | 'y' | 'Y') | KeyCode::Enter => {
                    self.quit = true;
                    None
                }
                _ => None,
            },
            Modal::Input { kind, mut buf } => match k.code {
                KeyCode::Esc => None,
                KeyCode::Backspace => {
                    buf.pop();
                    Some(Modal::Input { kind, buf })
                }
                KeyCode::Char(c) if buf.chars().count() < 60 => {
                    buf.push(c);
                    Some(Modal::Input { kind, buf })
                }
                KeyCode::Enter => match kind {
                    InputKind::Duration => match time::parse_duration(&buf) {
                        Ok(None) => {
                            self.pick(Choice::Preset(PRESETS.len() - 1));
                            None
                        }
                        Ok(Some(d)) => {
                            let c = PRESETS
                                .iter()
                                .position(|(_, s)| *s == Some(d.as_secs()))
                                .map_or(Choice::Custom(d), Choice::Preset);
                            self.pick(c);
                            None
                        }
                        Err(_) => Some(Modal::Input { kind, buf }),
                    },
                    InputKind::Reason => {
                        let r = buf.trim();
                        if !r.is_empty() && r != self.opts.reason {
                            self.opts.reason = r.to_owned();
                            if self.session.active()
                                && let Err(e) = self.session.apply(&self.opts)
                            {
                                self.say(format!("error: {e}"), Level::Err);
                                return None;
                            }
                            self.say("razón actualizada", Level::Info);
                        }
                        None
                    }
                },
                _ => Some(Modal::Input { kind, buf }),
            },
        }
    }
}

fn detect_env() -> String {
    let session = match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("x11") => "X11".to_owned(),
        Ok("wayland") => "Wayland".to_owned(),
        Ok(s) if !s.is_empty() => s.to_owned(),
        _ if std::env::var_os("DISPLAY").is_some() => "X11".to_owned(),
        _ => "tty".to_owned(),
    };
    let term = if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        "kitty".to_owned()
    } else if let Ok(t) = std::env::var("TERM_PROGRAM") {
        t
    } else {
        std::env::var("TERM").unwrap_or_default()
    };
    if term.is_empty() {
        session
    } else {
        format!("{session} · {term}")
    }
}
