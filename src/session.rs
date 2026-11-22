//! Estado de un bloqueo: guards (inhibidor + pantalla) y temporizador.
//! Lo usa la TUI; la CLI tiene su propio flujo lineal.

use std::time::{Duration, Instant};

use crate::guard::{inhibit::Inhibitor, screen::ScreenGuard};

const MAX_SPAN: Duration = Duration::from_secs(365 * 86400);

#[derive(Debug, Clone)]
pub struct Options {
    pub screen: bool,
    pub lid: bool,
    pub reason: String,
}

impl Options {
    pub fn what(&self) -> &'static str {
        if self.lid {
            "idle:sleep:handle-lid-switch"
        } else {
            "idle:sleep"
        }
    }
}

#[derive(Default)]
pub struct Session {
    // El orden de los campos es el orden de drop: pantalla antes que inhibidor.
    screen: Option<ScreenGuard>,
    inhib: Option<(Inhibitor, &'static str, String)>,
    started: Option<Instant>,
    deadline: Option<Instant>,
}

impl Session {
    pub fn active(&self) -> bool {
        self.inhib.is_some()
    }

    pub fn screen_on(&self) -> bool {
        self.screen.is_some()
    }

    /// Activa (o reactiva) el bloqueo y reinicia el temporizador.
    /// Si falla solo la pantalla, el inhibidor queda activo y se devuelve el error.
    pub fn start(&mut self, opts: &Options, dur: Option<Duration>) -> color_eyre::Result<()> {
        let res = self.apply(opts);
        if self.active() {
            let now = Instant::now();
            self.started = Some(now);
            self.deadline = dur.map(|d| now + d.min(MAX_SPAN));
        }
        res
    }

    /// Ajusta el bloqueo activo a `opts` sin tocar el temporizador.
    pub fn apply(&mut self, opts: &Options) -> color_eyre::Result<()> {
        let what = opts.what();
        let same = self
            .inhib
            .as_ref()
            .is_some_and(|(_, w, r)| *w == what && *r == opts.reason);
        if !same {
            // Primero el nuevo, luego se suelta el viejo: nunca hay hueco sin bloqueo.
            let new = Inhibitor::acquire(what, &opts.reason)?;
            self.inhib = Some((new, what, opts.reason.clone()));
        }
        match (opts.screen, self.screen.is_some()) {
            (true, false) => self.screen = Some(ScreenGuard::acquire()?),
            (false, true) => self.screen = None,
            _ => {}
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.screen = None;
        self.inhib = None;
        self.started = None;
        self.deadline = None;
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
    }

    pub fn elapsed(&self) -> Duration {
        self.started.map(|s| s.elapsed()).unwrap_or_default()
    }

    /// Duración total del tramo actual (cambia con `bump`).
    pub fn span(&self) -> Option<Duration> {
        Some(self.deadline?.saturating_duration_since(self.started?))
    }

    pub fn expired(&self) -> bool {
        self.active() && self.deadline.is_some_and(|d| Instant::now() >= d)
    }

    /// Suma o resta minutos al bloqueo con límite. `false` si no aplica (sin límite / inactivo).
    pub fn bump(&mut self, minutes: i64) -> bool {
        let (Some(start), Some(dl)) = (self.started, self.deadline) else {
            return false;
        };
        let delta = Duration::from_secs(minutes.unsigned_abs() * 60);
        let floor = Instant::now() + Duration::from_secs(30);
        let new = if minutes >= 0 {
            (dl + delta).min(start + MAX_SPAN)
        } else {
            dl.checked_sub(delta).unwrap_or(floor).max(floor)
        };
        self.deadline = Some(new);
        true
    }
}
