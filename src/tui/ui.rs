use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Cell, Clear, Paragraph, Row, Table},
};

use super::{
    app::{App, Choice, InputKind, Level, Modal, PRESETS, Status},
    digits,
    theme::{
        BORDER, DARK, DIM, ERR, FAINT, KEY_BG, MUTED, OK, OK_DIM, TEXT, WARN, accent, bold, fg,
        pill,
    },
};
use crate::time;

pub const MIN_W: u16 = 64;
pub const MIN_H: u16 = 20;
const MAX_W: u16 = 104;

pub fn draw(f: &mut Frame, app: &App, st: &Status) {
    let full = f.area();
    if full.width < MIN_W || full.height < MIN_H {
        too_small(f, full);
        return;
    }
    // En terminales muy anchas el contenido se centra en vez de estirarse.
    let w = full.width.min(MAX_W);
    let area = Rect::new(full.x + (full.width - w) / 2, full.y, w, full.height);

    let inhib_h = u16::try_from(app.inhibitors.len().max(1))
        .unwrap_or(8)
        .min(8)
        + 3;
    let hero_h = if area.height >= 24 { 10 } else { 7 };
    let [header, _, hero, mid, inh, _, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(u16::from(area.height >= 26)),
        Constraint::Length(hero_h),
        Constraint::Length(7),
        Constraint::Length(inhib_h),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, header, app, st);
    draw_hero(f, hero, app, st);
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(54), Constraint::Percentage(46)]).areas(mid);
    draw_duration(f, left, app, st);
    draw_options(f, right, app, st);
    draw_inhibitors(f, inh, app);
    draw_footer(f, footer, st);

    if let Some(m) = &app.modal {
        draw_modal(f, area, app, m);
    }
}

// ───────────────────────────── piezas ─────────────────────────────

fn panel<'a>(title: &'a str, hot: bool) -> Block<'a> {
    let border = if hot { accent(0.5) } else { BORDER };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(border))
        .title(Line::from(vec![
            Span::styled(" ", fg(border)),
            Span::styled(title, bold(if hot { TEXT } else { MUTED })),
            Span::styled(" ", fg(border)),
        ]))
}

fn pulse(tick: u64) -> bool {
    (tick / 5).is_multiple_of(2)
}

fn gradient(text: &str) -> Vec<Span<'static>> {
    let n = text.chars().count().max(2) - 1;
    text.chars()
        .enumerate()
        .map(|(i, c)| Span::styled(c.to_string(), bold(accent(i as f64 / n as f64))))
        .collect()
}

fn key(k: &str) -> Span<'static> {
    Span::styled(format!(" {k} "), Style::new().fg(TEXT).bg(KEY_BG))
}

fn clock_at(d: Duration) -> Option<String> {
    let sd = jiff::SignedDuration::try_from(d).ok()?;
    let now = jiff::ZonedDateTime::now();
    let at = now.checked_add(sd).ok()?;
    let fmt = if at.date() == now.date() {
        "%H:%M"
    } else {
        "%d/%m %H:%M"
    };
    Some(at.strftime(fmt).to_string())
}

fn width(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

fn too_small(f: &mut Frame, area: Rect) {
    let [_, mid, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Fill(1),
    ])
    .areas(area);
    let p = Paragraph::new(vec![
        Line::styled("terminal demasiado pequeña", bold(WARN)),
        Line::styled(
            format!("{}×{} · necesito {MIN_W}×{MIN_H}", area.width, area.height),
            fg(DIM),
        ),
    ])
    .alignment(Alignment::Center);
    f.render_widget(p, mid);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App, st: &Status) {
    let area = area.inner(Margin::new(1, 0));
    let dot = if !st.active {
        DIM
    } else if pulse(app.tick) {
        OK
    } else {
        OK_DIM
    };
    let mut left = vec![Span::styled("◉ ", fg(dot))];
    left.extend(gradient("nosleep"));
    left.push(Span::styled(
        format!("  v{}", env!("CARGO_PKG_VERSION")),
        fg(FAINT),
    ));

    let right = vec![
        Span::styled("● ", fg(if st.active { OK } else { DIM })),
        Span::styled("logind", fg(MUTED)),
        Span::styled("  ·  ", fg(FAINT)),
        Span::styled(app.env.clone(), fg(MUTED)),
    ];

    let free = usize::from(area.width).saturating_sub(width(&left) + width(&right) + 4);
    let mid = app.toast.as_ref().map(|(msg, lvl, _)| {
        let (icon, c) = match lvl {
            Level::Ok => ("✓ ", OK),
            Level::Warn => ("! ", WARN),
            Level::Err => ("✗ ", ERR),
            Level::Info => ("· ", accent(0.6)),
        };
        let mut s: String = format!("{icon}{msg}");
        if s.chars().count() > free {
            s = s.chars().take(free.saturating_sub(1)).collect::<String>() + "…";
        }
        Line::styled(s, fg(c))
    });

    f.render_widget(Paragraph::new(Line::from(left)), area);
    f.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        area,
    );
    if let Some(m) = mid {
        f.render_widget(Paragraph::new(m).alignment(Alignment::Center), area);
    }
}

fn draw_hero(f: &mut Frame, area: Rect, app: &App, st: &Status) {
    let block = panel("ahora", st.active);
    let inner = block.inner(area).inner(Margin::new(2, 0));
    f.render_widget(block, area);

    let clock = match (st.active, st.remaining) {
        (true, Some(r)) => time::fmt_hms(r),
        (true, None) => time::fmt_hms(st.elapsed),
        (false, _) => app
            .selected()
            .map_or_else(|| "--:--:--".to_owned(), time::fmt_hms),
    };
    let urgent = st.remaining.is_some_and(|r| r < Duration::from_secs(60));
    let info = info_lines(app, st);

    if inner.height >= 8 {
        let [_, top, _, bar] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        let [dig, _, side] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(2),
            Constraint::Length(30),
        ])
        .areas(top);
        let wide = digits::width(&clock, true) <= usize::from(dig.width);
        let rows = digits::render(&clock, wide);
        let lines: Vec<Line> = rows
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let c = if !st.active {
                    Color::Rgb(71, 85, 105)
                } else if urgent {
                    WARN
                } else {
                    accent(i as f64 / 4.0)
                };
                Line::styled(r, fg(c))
            })
            .collect();
        f.render_widget(Paragraph::new(lines), dig);
        f.render_widget(Paragraph::new(info), side);
        draw_bar(f, bar, app, st);
    } else {
        // Versión compacta para terminales bajas.
        let [_, line1, line2, bar] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        let mut first = vec![
            Span::styled(clock, bold(if st.active { accent(0.3) } else { DIM })),
            Span::raw("   "),
        ];
        first.extend(info.first().cloned().unwrap_or_default().spans);
        f.render_widget(Paragraph::new(Line::from(first)), line1);
        if let Some(l) = info.get(2) {
            f.render_widget(Paragraph::new(l.clone()), line2);
        }
        draw_bar(f, bar, app, st);
    }
}

fn info_lines(app: &App, st: &Status) -> Vec<Line<'static>> {
    let status = if st.active {
        let dot = if pulse(app.tick) { "●" } else { "○" };
        Line::from(vec![Span::styled(
            format!(" {dot} ACTIVO "),
            pill(DARK, OK),
        )])
    } else {
        Line::from(vec![Span::styled(" ○ EN ESPERA ", pill(MUTED, KEY_BG))])
    };
    let when = match (st.active, st.remaining, app.selected()) {
        (true, Some(r), _) => Line::from(vec![
            Span::styled("vence   ", fg(DIM)),
            Span::styled(clock_at(r).unwrap_or_default(), bold(TEXT)),
        ]),
        (true, None, _) => Line::from(vec![
            Span::styled("sin límite", bold(TEXT)),
            Span::styled(" · transcurrido", fg(DIM)),
        ]),
        (false, _, Some(d)) => Line::from(vec![
            Span::styled("vencería ", fg(DIM)),
            Span::styled(clock_at(d).unwrap_or_default(), fg(MUTED)),
        ]),
        (false, _, None) => Line::styled("sin límite · hasta detener", fg(DIM)),
    };
    let flag = |label: &'static str, on: bool, yes: &'static str, no: &'static str| {
        Line::from(vec![
            Span::styled(label, fg(DIM)),
            Span::styled(
                if on { "● " } else { "○ " },
                fg(if on { OK } else { FAINT }),
            ),
            Span::styled(if on { yes } else { no }, fg(if on { TEXT } else { MUTED })),
        ])
    };
    let screen_on = if st.active {
        st.screen
    } else {
        app.opts.screen
    };
    vec![
        status,
        Line::default(),
        when,
        flag("pantalla ", screen_on, "siempre encendida", "normal"),
        flag("tapa     ", app.opts.lid, "ignorada", "suspende"),
    ]
}

fn draw_bar(f: &mut Frame, area: Rect, app: &App, st: &Status) {
    let w = usize::from(area.width);
    let mut spans = Vec::new();
    match (st.active, st.span) {
        (true, Some(span)) => {
            let frac = if span.is_zero() {
                1.0
            } else {
                (st.elapsed.as_secs_f64() / span.as_secs_f64()).clamp(0.0, 1.0)
            };
            let label = format!(" {:>3}%", (frac * 100.0).floor() as u32);
            let bw = w.saturating_sub(label.len());
            let filled = (frac * bw as f64).round() as usize;
            for i in 0..bw {
                let c = if i < filled {
                    accent(i as f64 / bw.max(1) as f64)
                } else {
                    FAINT
                };
                spans.push(Span::styled("━", fg(c)));
            }
            spans.push(Span::styled(label, fg(MUTED)));
        }
        (true, None) => {
            // Sin límite: un brillo que recorre la barra.
            let label = "   ∞ ";
            let bw = w.saturating_sub(label.chars().count());
            let pos = (app.tick as usize * 2) % (bw + 24);
            for i in 0..bw {
                let d = (i as isize - pos as isize + 12).unsigned_abs();
                let c = if d < 12 {
                    accent(1.0 - d as f64 / 12.0)
                } else {
                    FAINT
                };
                spans.push(Span::styled("━", fg(c)));
            }
            spans.push(Span::styled(label, fg(MUTED)));
        }
        (false, _) => {
            let hint = vec![
                Span::raw(" "),
                key("⏎"),
                Span::styled(" iniciar", fg(MUTED)),
            ];
            let bw = w.saturating_sub(width(&hint));
            spans.push(Span::styled("━".repeat(bw), fg(FAINT)));
            spans.extend(hint);
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_duration(f: &mut Frame, area: Rect, app: &App, st: &Status) {
    let block = panel("duración", false);
    let inner = block.inner(area).inner(Margin::new(2, 0));
    f.render_widget(block, area);

    let mut chips = Vec::new();
    let mut keys = Vec::new();
    for (i, (label, _)) in PRESETS.iter().enumerate() {
        let sel = app.choice == Choice::Preset(i);
        let text = format!(" {label} ");
        let w = text.chars().count();
        chips.push(if sel {
            Span::styled(text, pill(DARK, accent(0.35)))
        } else {
            Span::styled(text, fg(MUTED))
        });
        let num = (i + 1).to_string();
        let pad = w - 1;
        keys.push(Span::styled(
            format!("{}{num}{}", " ".repeat(pad / 2), " ".repeat(pad - pad / 2)),
            fg(if sel { accent(0.35) } else { FAINT }),
        ));
    }

    let custom = match app.choice {
        Choice::Custom(d) => Line::from(vec![
            Span::styled(" ✦ ", fg(accent(0.35))),
            Span::styled(time::fmt_short(Some(d)), pill(DARK, accent(0.35))),
            Span::styled("  personalizado", fg(DIM)),
        ]),
        Choice::Preset(_) => Line::from(vec![
            key("c"),
            Span::styled(" personalizado   ", fg(DIM)),
            key("±"),
            Span::styled(" 5 min", fg(DIM)),
        ]),
    };
    let summary = match app.selected() {
        Some(d) if !st.active => Line::from(vec![
            Span::styled(time::fmt_short(Some(d)), bold(TEXT)),
            Span::styled(format!(" · {} min", d.as_secs() / 60), fg(DIM)),
        ]),
        Some(_) => Line::styled("elegir otra reinicia el contador", fg(DIM)),
        None => Line::styled("sin límite · hasta pulsar x", fg(DIM)),
    };
    let lines = vec![
        Line::default(),
        Line::from(chips),
        Line::from(keys),
        custom,
        summary,
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_options(f: &mut Frame, area: Rect, app: &App, st: &Status) {
    let block = panel("opciones", false);
    let inner = block.inner(area).inner(Margin::new(2, 0));
    f.render_widget(block, area);
    let w = usize::from(inner.width);

    let row = |on: bool, label: &str, k: &str| {
        let switch = if on {
            Span::styled("  ON ", pill(DARK, OK))
        } else {
            Span::styled(" OFF ", Style::new().fg(DIM).bg(KEY_BG))
        };
        let left = vec![
            switch,
            Span::raw("  "),
            Span::styled(label.to_owned(), fg(if on { TEXT } else { MUTED })),
        ];
        let pad = w.saturating_sub(width(&left) + k.chars().count() + 2);
        let mut spans = left;
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(key(k));
        Line::from(spans)
    };
    let screen_on = if st.active {
        st.screen
    } else {
        app.opts.screen
    };

    let reason_w = w.saturating_sub(14);
    let mut reason: String = app.opts.reason.chars().take(reason_w).collect();
    if app.opts.reason.chars().count() > reason_w {
        reason.pop();
        reason.push('…');
    }
    let mut r = vec![
        Span::styled("razón  ", fg(DIM)),
        Span::styled(format!("«{reason}»"), fg(MUTED)),
    ];
    let pad = w.saturating_sub(width(&r) + 3);
    r.push(Span::raw(" ".repeat(pad)));
    r.push(key("r"));

    let lines = vec![
        Line::default(),
        row(screen_on, "Pantalla encendida", "p"),
        row(app.opts.lid, "Ignorar la tapa", "t"),
        Line::default(),
        Line::from(r),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_inhibitors(f: &mut Frame, area: Rect, app: &App) {
    let title = format!("inhibidores del sistema · {}", app.inhibitors.len());
    let block = panel(&title, false);
    let inner = block.inner(area).inner(Margin::new(1, 0));
    f.render_widget(block, area);

    if app.inhibitors.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(
                " · nada está bloqueando la suspensión",
                fg(DIM),
            )),
            inner,
        );
        return;
    }
    let me = std::process::id();
    let rows = app
        .inhibitors
        .iter()
        .map(|(what, who, why, mode, _uid, pid)| {
            let mine = *pid == me;
            let (c_who, c_rest) = if mine {
                (bold(accent(0.4)), fg(TEXT))
            } else {
                (fg(TEXT), fg(MUTED))
            };
            let mode_style = if mode == "block" { fg(WARN) } else { fg(DIM) };
            Row::new(vec![
                Cell::from(Span::styled(if mine { "◉" } else { " " }, fg(accent(0.4)))),
                Cell::from(Span::styled(who.clone(), c_who)),
                Cell::from(Span::styled(what.clone(), c_rest)),
                Cell::from(Span::styled(mode.clone(), mode_style)),
                Cell::from(Span::styled(pid.to_string(), fg(DIM))),
                Cell::from(Span::styled(why.clone(), c_rest)),
            ])
        });
    let col = |f: fn(&crate::guard::inhibit::InhibitorRow) -> usize, min: usize| {
        let w = app
            .inhibitors
            .iter()
            .map(f)
            .max()
            .unwrap_or(0)
            .clamp(min, 28);
        Constraint::Length(u16::try_from(w).unwrap_or(28))
    };
    let who_w = col(|r| r.1.chars().count(), 5);
    let what_w = col(|r| r.0.chars().count(), 3);
    let header = Row::new(["", "QUIÉN", "QUÉ", "MODO", "PID", "RAZÓN"]).style(fg(DIM));
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            who_w,
            what_w,
            Constraint::Length(5),
            Constraint::Length(7),
            Constraint::Fill(1),
        ],
    )
    .header(header)
    .column_spacing(2);
    f.render_widget(table, inner);
}

fn draw_footer(f: &mut Frame, area: Rect, st: &Status) {
    let area = area.inner(Margin::new(1, 0));
    let start = if st.active { "reiniciar" } else { "iniciar" };
    // (prioridad, tecla, texto): si no cabe, se quitan los de menor prioridad.
    let mut items = vec![(0, "⏎", start)];
    if st.active {
        items.push((0, "x", "detener"));
        items.push((2, "±", "5 min"));
    }
    items.extend([
        (1, "1-7", "duración"),
        (3, "c", "personalizar"),
        (3, "p t", "opciones"),
        (1, "?", "ayuda"),
        (0, "q", "salir"),
    ]);
    let max = usize::from(area.width);
    let cost = |it: &(i32, &str, &str)| it.1.chars().count() + it.2.chars().count() + 5;
    let mut lvl = 3;
    while items.iter().map(cost).sum::<usize>() > max && lvl > 0 {
        items.retain(|it| it.0 < lvl);
        lvl -= 1;
    }
    let mut spans = Vec::new();
    for (_, k, label) in items {
        spans.push(key(k));
        spans.push(Span::styled(format!(" {label}  "), fg(DIM)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ───────────────────────────── modales ─────────────────────────────

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(4));
    let h = h.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn modal_block(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(accent(0.2)))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title, bold(TEXT)),
            Span::raw(" "),
        ]))
}

fn draw_modal(f: &mut Frame, area: Rect, app: &App, m: &Modal) {
    match m {
        Modal::Help => {
            let rows: [(&str, &str); 11] = [
                ("⏎ / s", "iniciar · reiniciar el contador"),
                ("x", "detener y restaurar"),
                ("+ / −", "sumar o restar 5 minutos"),
                ("1 … 7", "10m 30m 1h 2h 4h 8h ∞"),
                ("c", "duración personalizada"),
                ("p", "pantalla siempre encendida (X11)"),
                ("t", "ignorar el cierre de la tapa"),
                ("r", "cambiar la razón"),
                ("?", "esta ayuda"),
                ("q", "salir (pide confirmación)"),
                ("ctrl+c", "salir ya, soltando todo"),
            ];
            let mut lines = vec![Line::default()];
            for (k, d) in rows {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{k:<8}"), bold(accent(0.4))),
                    Span::styled(d, fg(TEXT)),
                ]));
            }
            lines.push(Line::default());
            lines.push(Line::styled("  cualquier tecla para cerrar", fg(DIM)));
            let r = centered(area, 52, 16);
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(lines).block(modal_block("atajos")), r);
        }
        Modal::ConfirmQuit => {
            let lines = vec![
                Line::default(),
                Line::styled("  El bloqueo sigue activo.", fg(TEXT)),
                Line::styled("  Al salir se suelta y se restaura todo.", fg(MUTED)),
                Line::default(),
                Line::from(vec![
                    Span::raw("  "),
                    key("s"),
                    Span::styled(" salir    ", fg(MUTED)),
                    key("cualquier otra"),
                    Span::styled(" seguir", fg(MUTED)),
                ]),
            ];
            let r = centered(area, 50, 7);
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(lines).block(modal_block("¿salir?")), r);
        }
        Modal::Input { kind, buf } => {
            let (title, hint) = match kind {
                InputKind::Duration => ("duración personalizada", "ej. 45m · 1h30m · 2.5h · inf"),
                InputKind::Reason => ("razón para logind", "se ve en `nosleep --list`"),
            };
            let cursor = if pulse(app.tick) { "▏" } else { " " };
            let preview = match kind {
                InputKind::Duration if buf.trim().is_empty() => Line::styled(hint, fg(DIM)),
                InputKind::Duration => match time::parse_duration(buf) {
                    Ok(Some(d)) => Line::from(vec![
                        Span::styled("✓ ", fg(OK)),
                        Span::styled(time::fmt_short(Some(d)), bold(TEXT)),
                        Span::styled(
                            format!(" · vencería {}", clock_at(d).unwrap_or_default()),
                            fg(DIM),
                        ),
                    ]),
                    Ok(None) => Line::from(vec![
                        Span::styled("✓ ", fg(OK)),
                        Span::styled("sin límite", bold(TEXT)),
                    ]),
                    Err(e) => Line::styled(format!("✗ {e}"), fg(ERR)),
                },
                InputKind::Reason => Line::styled(hint, fg(DIM)),
            };
            let mut out = vec![
                Line::default(),
                Line::from(vec![
                    Span::styled("  › ", bold(accent(0.3))),
                    Span::styled(buf.clone(), bold(TEXT)),
                    Span::styled(cursor, fg(accent(0.6))),
                ]),
            ];
            let mut p = vec![Span::raw("    ")];
            p.extend(preview.spans);
            out.push(Line::from(p));
            out.push(Line::default());
            out.push(Line::from(vec![
                Span::raw("  "),
                key("⏎"),
                Span::styled(" aplicar   ", fg(MUTED)),
                key("esc"),
                Span::styled(" cancelar", fg(MUTED)),
            ]));
            let r = centered(area, 56, 7);
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(out).block(modal_block(title)), r);
        }
    }
}
