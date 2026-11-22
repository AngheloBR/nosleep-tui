use color_eyre::eyre::{WrapErr, bail, eyre};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::PathBuf,
    process::Command,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XsetState {
    pub timeout: u32,
    pub cycle: u32,
    pub blank: bool,
    pub dpms: bool,
    pub standby: u32,
    pub suspend: u32,
    pub off: u32,
}

fn xset(args: &[&str]) -> color_eyre::Result<String> {
    let out = Command::new("xset")
        .args(args)
        .output()
        .wrap_err("no pude ejecutar `xset` · sudo dnf install xset")?;
    if !out.status.success() {
        bail!(
            "xset {} falló: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Busca la ÚLTIMA aparición: `xset q` repite claves (los LEDs del teclado
/// incluyen `Suspend:`), y la sección DPMS/Screen Saver va al final.
fn after<'a>(toks: &[&'a str], key: &str) -> Option<&'a str> {
    toks.iter()
        .rposition(|t| *t == key)
        .and_then(|i| toks.get(i + 1))
        .copied()
}

impl XsetState {
    pub fn read() -> color_eyre::Result<Self> {
        Self::parse(&xset(&["q"])?)
    }

    fn parse(q: &str) -> color_eyre::Result<Self> {
        let t: Vec<&str> = q.split_whitespace().collect();
        // Si falta un campo preferimos fallar antes que "restaurar" valores inventados.
        let num = |k: &str| -> color_eyre::Result<u32> {
            after(&t, k)
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| eyre!("no encontré {k:?} en la salida de `xset q`"))
        };
        Ok(Self {
            timeout: num("timeout:")?,
            cycle: num("cycle:")?,
            blank: after(&t, "blanking:") == Some("yes"),
            dpms: q.contains("DPMS is Enabled"),
            standby: num("Standby:")?,
            suspend: num("Suspend:")?,
            off: num("Off:")?,
        })
    }

    pub fn disable(&self) -> color_eyre::Result<()> {
        xset(&["s", "off"])?;
        xset(&["s", "noblank"])?;
        xset(&["-dpms"])?;
        Ok(())
    }

    pub fn restore(&self) -> color_eyre::Result<()> {
        let (a, b) = (self.timeout.to_string(), self.cycle.to_string());
        xset(&["s", &a, &b])?;
        xset(&["s", if self.blank { "blank" } else { "noblank" }])?;
        let (s, u, o) = (
            self.standby.to_string(),
            self.suspend.to_string(),
            self.off.to_string(),
        );
        xset(&["dpms", &s, &u, &o])?;
        xset(&[if self.dpms { "+dpms" } else { "-dpms" }])?;
        Ok(())
    }
}

fn state_path() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("nosleep").join("xset.json")
}

pub struct ScreenGuard {
    saved: XsetState,
    path: PathBuf,
}

impl ScreenGuard {
    pub fn acquire() -> color_eyre::Result<Self> {
        if std::env::var_os("DISPLAY").is_none() {
            bail!("no hay $DISPLAY · esto es solo para X11");
        }
        if std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland") {
            bail!("--screen usa xset y solo funciona en sesiones X11, no en Wayland");
        }
        let path = state_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        // create_new: si ya existe, o hay otro `nosleep --screen` activo o quedó
        // estado de una muerte sucia. Sobrescribirlo perdería la config original.
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => bail!(
                "ya existe {} · ¿otro `nosleep --screen` activo? si no, ejecuta `nosleep --restore`",
                path.display()
            ),
            Err(e) => return Err(e.into()),
        };

        let saved = match XsetState::read() {
            Ok(s) => s,
            Err(e) => {
                let _ = fs::remove_file(&path);
                return Err(e);
            }
        };

        // 1º PERSISTIR, 2º MUTAR. Si mueres en medio, --restore lo arregla.
        // A partir de aquí el guard existe: cualquier error lo suelta y Drop restaura.
        let guard = Self { saved, path };
        file.write_all(&serde_json::to_vec_pretty(&guard.saved)?)?;
        file.sync_all()?;
        drop(file);
        guard.saved.disable()?;
        Ok(guard)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let _ = self.saved.restore();
        let _ = fs::remove_file(&self.path);
    }
}

/// Idempotente. `true` si había algo que restaurar.
pub fn restore_leftover() -> color_eyre::Result<bool> {
    let p = state_path();
    if !p.exists() {
        return Ok(false);
    }
    let saved: XsetState = serde_json::from_slice(&fs::read(&p)?)?;
    saved.restore()?;
    fs::remove_file(&p)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_xset_q_real() {
        let st = XsetState::parse(include_str!("xset_q.fixture.txt")).unwrap();
        assert_eq!((st.timeout, st.cycle), (1800, 1800));
        assert_eq!((st.standby, st.suspend, st.off), (1800, 1800, 1800));
        assert!(st.dpms);
        assert!(!st.blank);
    }

    #[test]
    fn falla_si_falta_un_campo() {
        assert!(XsetState::parse("DPMS is Enabled").is_err());
    }
}
