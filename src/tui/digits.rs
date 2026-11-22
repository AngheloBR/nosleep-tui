//! Reloj grande: fuente de 3×5 "píxeles". En modo ancho cada píxel ocupa
//! dos columnas, lo que compensa que las celdas de la terminal son altas.

const ROWS: usize = 5;

fn glyph(c: char) -> [&'static str; ROWS] {
    match c {
        '0' => ["###", "# #", "# #", "# #", "###"],
        '1' => [" # ", "## ", " # ", " # ", "###"],
        '2' => ["###", "  #", "###", "#  ", "###"],
        '3' => ["###", "  #", "###", "  #", "###"],
        '4' => ["# #", "# #", "###", "  #", "  #"],
        '5' => ["###", "#  ", "###", "  #", "###"],
        '6' => ["###", "#  ", "###", "# #", "###"],
        '7' => ["###", "  #", "  #", "  #", "  #"],
        '8' => ["###", "# #", "###", "# #", "###"],
        '9' => ["###", "# #", "###", "  #", "###"],
        ':' => [" ", "#", " ", "#", " "],
        '-' => ["   ", "   ", "###", "   ", "   "],
        _ => ["   "; ROWS],
    }
}

/// Columnas que ocupa `text`.
pub fn width(text: &str, wide: bool) -> usize {
    let px: usize = text.chars().map(|c| glyph(c)[0].len()).sum();
    let gaps = text.chars().count().saturating_sub(1);
    px * if wide { 2 } else { 1 } + gaps
}

/// Las 5 filas de `text` ya renderizadas con bloques.
pub fn render(text: &str, wide: bool) -> [String; ROWS] {
    let on = if wide { "██" } else { "█" };
    let off = if wide { "  " } else { " " };
    std::array::from_fn(|row| {
        text.chars()
            .map(|c| {
                glyph(c)[row]
                    .chars()
                    .map(|p| if p == '#' { on } else { off })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancho_coincide_con_render() {
        for wide in [false, true] {
            let rows = render("01:29:57", wide);
            for r in &rows {
                assert_eq!(r.chars().count(), width("01:29:57", wide));
            }
        }
    }
}
