mod cli;
mod guard;
mod time;

use std::{
    io::Write,
    process::{Command, ExitCode},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use cli::Cli;

const TICK: Duration = Duration::from_millis(200);

fn main() -> color_eyre::Result<ExitCode> {
    color_eyre::install()?;
    let cli = Cli::parse();

    if cli.restore {
        let did = guard::screen::restore_leftover()?;
        println!(
            "{}",
            if did {
                "✓ pantalla restaurada"
            } else {
                "· nada que restaurar"
            }
        );
        return Ok(ExitCode::SUCCESS);
    }

    if cli.list {
        let rows = guard::inhibit::list()?;
        if rows.is_empty() {
            println!("· no hay inhibidores activos");
        }
        for (what, who, why, mode, uid, pid) in rows {
            println!("{who:<14} {what:<28} {mode:<6} uid={uid:<5} pid={pid:<7} {why}");
        }
        return Ok(ExitCode::SUCCESS);
    }

    let dur = match cli.time.as_deref() {
        Some(t) => time::parse_duration(t)?,
        None => Some(Duration::from_secs(3600)),
    };

    let stop = Arc::new(AtomicBool::new(false));
    for sig in [
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(sig, Arc::clone(&stop))?;
    }

    let what = if cli.lid {
        "idle:sleep:handle-lid-switch"
    } else {
        "idle:sleep"
    };

    // El orden importa: al salir se suelta en orden inverso (pantalla → inhibidor).
    let inhib = guard::inhibit::Inhibitor::acquire(what, &cli.reason)?;
    let screen = if cli.screen {
        Some(guard::screen::ScreenGuard::acquire()?)
    } else {
        None
    };

    let mut code = ExitCode::SUCCESS;
    if let Some(cmd) = cli.while_cmd {
        println!(
            "◉ bloqueo activo · mientras corre `{}` · {what}",
            cmd.join(" ")
        );
        let mut child = Command::new(&cmd[0]).args(&cmd[1..]).spawn()?;
        let status = loop {
            if let Some(st) = child.try_wait()? {
                break Some(st);
            }
            if stop.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            thread::sleep(TICK);
        };
        // Propaga el código del comando (útil en scripts: `nosleep --while make && …`).
        code = match status.and_then(|s| s.code()) {
            Some(0) => ExitCode::SUCCESS,
            Some(c) => ExitCode::from(u8::try_from(c).unwrap_or(1)),
            None => ExitCode::from(130),
        };
    } else {
        println!("◉ bloqueo activo · {} · {what}", time::fmt_short(dur));
        let deadline = dur.map(|d| Instant::now() + d);
        while !stop.load(Ordering::Relaxed) {
            if let Some(dl) = deadline {
                let now = Instant::now();
                if now >= dl {
                    break;
                }
                print!("\r  restante {}   ", time::fmt_hms(dl - now));
                std::io::stdout().flush().ok();
            }
            thread::sleep(TICK);
        }
        println!();
    }

    drop(screen);
    drop(inhib);
    println!("✓ bloqueo liberado y configuración restaurada");
    Ok(code)
}
