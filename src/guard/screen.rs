//! Pantalla encendida en X11 hablando el protocolo directamente (x11rb, Rust puro):
//! sin depender de `xset` ni de libX11.

use color_eyre::eyre::{WrapErr, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::PathBuf,
};
use x11rb::{
    connection::RequestConnection,
    protocol::{
        dpms::ConnectionExt as _,
        xproto::{Blanking, ConnectionExt as _, Exposures},
    },
    rust_connection::RustConnection,
};

/// Lo que `xset q` mostraría en "Screen Saver" y "DPMS".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenState {
    pub timeout: u16,
    pub interval: u16,
    pub blanking: u8,
    pub exposures: u8,
    /// `None` si el servidor no tiene la extensión DPMS.
    pub dpms: Option<Dpms>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dpms {
    pub enabled: bool,
    pub standby: u16,
    pub suspend: u16,
    pub off: u16,
}

fn connect() -> color_eyre::Result<RustConnection> {
    let (conn, _) = x11rb::connect(None).wrap_err("no pude conectar con el servidor X11")?;
    Ok(conn)
}

fn has_dpms(conn: &RustConnection) -> bool {
    conn.extension_information(x11rb::protocol::dpms::X11_EXTENSION_NAME)
        .ok()
        .flatten()
        .is_some()
        && conn
            .dpms_capable()
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.capable)
}

/// X11 guarda los tiempos como INT16 en las peticiones y CARD16 en las respuestas.
fn as_i16(v: u16) -> i16 {
    i16::try_from(v).unwrap_or(i16::MAX)
}

impl ScreenState {
    pub fn read() -> color_eyre::Result<Self> {
        let conn = connect()?;
        let ss = conn.get_screen_saver()?.reply()?;
        let dpms = if has_dpms(&conn) {
            let t = conn.dpms_get_timeouts()?.reply()?;
            let info = conn.dpms_info()?.reply()?;
            Some(Dpms {
                enabled: info.state,
                standby: t.standby_timeout,
                suspend: t.suspend_timeout,
                off: t.off_timeout,
            })
        } else {
            None
        };
        Ok(Self {
            timeout: ss.timeout,
            interval: ss.interval,
            blanking: ss.prefer_blanking.into(),
            exposures: ss.allow_exposures.into(),
            dpms,
        })
    }

    /// Equivale a `xset s off s noblank -dpms`.
    pub fn disable(&self) -> color_eyre::Result<()> {
        let conn = connect()?;
        conn.set_screen_saver(
            0,
            as_i16(self.interval),
            Blanking::NOT_PREFERRED,
            Exposures::from(self.exposures),
        )?
        .check()?;
        if self.dpms.is_some() {
            conn.dpms_disable()?.check()?;
        }
        Ok(())
    }

    pub fn restore(&self) -> color_eyre::Result<()> {
        let conn = connect()?;
        conn.set_screen_saver(
            as_i16(self.timeout),
            as_i16(self.interval),
            Blanking::from(self.blanking),
            Exposures::from(self.exposures),
        )?
        .check()?;
        if let Some(d) = &self.dpms {
            conn.dpms_set_timeouts(d.standby, d.suspend, d.off)?
                .check()?;
            if d.enabled {
                conn.dpms_enable()?.check()?;
            } else {
                conn.dpms_disable()?.check()?;
            }
        }
        Ok(())
    }
}

fn state_path() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("nosleep").join("screen.json")
}

pub struct ScreenGuard {
    saved: ScreenState,
    path: PathBuf,
}

impl ScreenGuard {
    pub fn acquire() -> color_eyre::Result<Self> {
        if std::env::var_os("DISPLAY").is_none() {
            bail!("no hay $DISPLAY · esto es solo para X11");
        }
        if std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland") {
            bail!("--screen solo funciona en sesiones X11, no en Wayland");
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

        let saved = match ScreenState::read() {
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
    let saved: ScreenState = serde_json::from_slice(&fs::read(&p)?)?;
    saved.restore()?;
    fs::remove_file(&p)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_estado_sobrevive_al_json() {
        let st = ScreenState {
            timeout: 1800,
            interval: 1800,
            blanking: 0,
            exposures: 1,
            dpms: Some(Dpms {
                enabled: true,
                standby: 1800,
                suspend: 1800,
                off: 1800,
            }),
        };
        let back: ScreenState = serde_json::from_slice(&serde_json::to_vec(&st).unwrap()).unwrap();
        assert_eq!(st, back);
    }

    #[test]
    fn tiempos_enormes_no_desbordan() {
        assert_eq!(as_i16(40_000), i16::MAX);
        assert_eq!(as_i16(600), 600);
    }
}
