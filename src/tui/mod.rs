mod app;
mod digits;
#[cfg(test)]
mod snapshot;
mod theme;
mod ui;

use std::{
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use crate::session::Options;
use app::App;

const FRAME: Duration = Duration::from_millis(100);

/// `initial`: `None` = duración por defecto; `Some(None)` = sin límite.
pub fn run(opts: Options, initial: Option<Option<Duration>>) -> color_eyre::Result<ExitCode> {
    // Raw mode convierte Ctrl+C en una tecla; SIGTERM/SIGHUP siguen llegando como señal.
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP] {
        signal_hook::flag::register(sig, Arc::clone(&stop))?;
    }

    let mut app = App::new(opts, initial);
    // `ratatui::init` instala un hook que restaura la terminal si hay panic;
    // el unwind suelta después los guards de `app` (inhibidor y pantalla).
    let mut terminal = ratatui::init();
    let res = event_loop(&mut terminal, &mut app, &stop);
    ratatui::restore();

    let was_active = app.session.active();
    app.session.stop();
    res?;
    if was_active {
        println!("✓ bloqueo liberado y configuración restaurada");
    }
    Ok(ExitCode::SUCCESS)
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    stop: &AtomicBool,
) -> color_eyre::Result<()> {
    while !app.quit && !stop.load(Ordering::Relaxed) {
        app.update();
        let st = app.status();
        terminal.draw(|f| ui::draw(f, app, &st))?;
        if event::poll(FRAME)?
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
        {
            app.on_key(k);
        }
        app.tick = app.tick.wrapping_add(1);
    }
    Ok(())
}
