//! Capturas del README generadas con el mismo código que dibuja la TUI.
//! `cargo test snapshots -- --ignored` reescribe `docs/tui-*.svg`.

use std::{fmt::Write as _, time::Duration};

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};

use super::{
    app::{App, Choice, InputKind, Level, Modal, Status},
    ui,
};
use crate::session::Options;

const BG: &str = "#0b1120";
const DEFAULT_FG: &str = "#e2e8f0";
const CW: f64 = 8.6;
const LH: f64 = 19.0;

fn hex(c: Color) -> Option<String> {
    match c {
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        Color::Reset => None,
        other => Some(format!("{other:?}").to_lowercase()),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn to_svg(buf: &Buffer, title: &str) -> String {
    let (w, h) = (buf.area.width, buf.area.height);
    let (pw, top) = (16.0, 40.0);
    let width = f64::from(w) * CW + pw * 2.0;
    let height = f64::from(h) * LH + top + 14.0;
    let mut s = String::new();
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">
<style>text{{font-family:"JetBrains Mono","Fira Code","DejaVu Sans Mono",Menlo,Consolas,monospace;font-size:14.3px;white-space:pre}}</style>
<rect width="100%" height="100%" rx="12" fill="{BG}" stroke="#1e293b"/>
<circle cx="22" cy="20" r="6" fill="#ff5f57"/><circle cx="42" cy="20" r="6" fill="#febc2e"/><circle cx="62" cy="20" r="6" fill="#28c840"/>
<text x="{:.0}" y="25" fill="#64748b" text-anchor="middle" style="font-size:12px">{}</text>
"##,
        width / 2.0,
        esc(title)
    );
    for y in 0..h {
        let ty = top + f64::from(y) * LH;
        // Fondos por celda.
        for x in 0..w {
            let cell = &buf[(x, y)];
            if cell.symbol() == "█" {
                let fill = hex(cell.fg).unwrap_or_else(|| DEFAULT_FG.to_owned());
                let _ = writeln!(
                    s,
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{fill}"/>"#,
                    pw + f64::from(x) * CW,
                    ty,
                    CW + 0.4,
                    LH + 0.4
                );
                continue;
            }
            if let Some(bg) = hex(cell.bg) {
                let _ = writeln!(
                    s,
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{LH}" fill="{bg}"/>"#,
                    pw + f64::from(x) * CW,
                    ty,
                    CW + 0.4
                );
            }
        }
        // Texto agrupado en tramos del mismo estilo.
        let mut x = 0;
        while x < w {
            let c0 = &buf[(x, y)];
            let style = (c0.fg, c0.modifier.contains(Modifier::BOLD));
            let start = x;
            let mut text = String::new();
            while x < w {
                let c = &buf[(x, y)];
                if (c.fg, c.modifier.contains(Modifier::BOLD)) != style {
                    break;
                }
                text.push_str(if c.symbol() == "█" { " " } else { c.symbol() });
                x += 1;
            }
            if text.trim().is_empty() {
                continue;
            }
            let fill = hex(style.0).unwrap_or_else(|| DEFAULT_FG.to_owned());
            let weight = if style.1 { r#" font-weight="700""# } else { "" };
            let n = x - start;
            let _ = writeln!(
                s,
                r#"<text x="{:.1}" y="{:.1}" fill="{fill}"{weight} textLength="{:.1}" lengthAdjust="spacingAndGlyphs">{}</text>"#,
                pw + f64::from(start) * CW,
                ty + LH * 0.74,
                f64::from(n) * CW,
                esc(&text)
            );
        }
    }
    s.push_str("</svg>\n");
    s
}

fn demo_app() -> App {
    let mut app = App::new(
        Options {
            screen: true,
            lid: true,
            reason: "Sesión de trabajo activa (nosleep)".into(),
        },
        None,
    );
    let me = std::process::id();
    let row = |what: &str, who: &str, why: &str, mode: &str, uid: u32, pid: u32| {
        (what.into(), who.into(), why.into(), mode.into(), uid, pid)
    };
    app.inhibitors = vec![
        row(
            "sleep",
            "NetworkManager",
            "NetworkManager needs to turn off networks",
            "delay",
            0,
            868,
        ),
        row(
            "idle:sleep:handle-lid-switch",
            "nosleep",
            "Sesión de trabajo activa (nosleep)",
            "block",
            1000,
            me,
        ),
        row("idle", "firefox", "Video playing", "block", 1000, 4127),
        row("sleep", "UPower", "Pause device polling", "delay", 0, 877),
        row(
            "sleep",
            "xss-lock",
            "Lock screen first",
            "delay",
            1000,
            1933,
        ),
    ];
    app.env = "X11 · kitty".into();
    app
}

fn render(app: &App, st: &Status, w: u16, h: u16) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app, st)).unwrap();
    term.backend().buffer().clone()
}

#[test]
fn dibuja_en_todos_los_tamanos_sin_panico() {
    let app = demo_app();
    for (w, h) in [(40, 10), (64, 20), (80, 24), (120, 40), (250, 70)] {
        render(&app, &Status::default(), w, h);
    }
}

#[test]
#[ignore = "regenera docs/tui-*.svg"]
fn snapshots() {
    let docs = concat!(env!("CARGO_MANIFEST_DIR"), "/docs");
    let mut app = demo_app();
    app.choice = Choice::Preset(3);
    app.tick = 2;
    app.toast = Some((
        "bloqueo activo · 2h".into(),
        Level::Ok,
        std::time::Instant::now(),
    ));
    let active = Status {
        active: true,
        remaining: Some(Duration::from_secs(3600 + 12 * 60 + 47)),
        elapsed: Duration::from_secs(47 * 60 + 13),
        span: Some(Duration::from_secs(7200)),
        screen: true,
    };
    let buf = render(&app, &active, 92, 28);
    std::fs::write(format!("{docs}/tui-active.svg"), to_svg(&buf, "nosleep")).unwrap();

    let mut idle = demo_app();
    idle.opts.screen = false;
    idle.opts.lid = false;
    idle.inhibitors.retain(|r| r.1 != "nosleep");
    idle.modal = Some(Modal::Input {
        kind: InputKind::Duration,
        buf: "1h30m".into(),
    });
    let buf = render(&idle, &Status::default(), 92, 28);
    std::fs::write(format!("{docs}/tui-custom.svg"), to_svg(&buf, "nosleep")).unwrap();
}
