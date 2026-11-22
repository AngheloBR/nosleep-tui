use zbus::{blocking::Connection, zvariant::OwnedFd};

/// (what, who, why, mode, uid, pid)
pub type InhibitorRow = (String, String, String, String, u32, u32);

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait LoginManager {
    /// Devuelve un fd. Mientras siga abierto, logind respeta el bloqueo.
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;

    fn list_inhibitors(&self) -> zbus::Result<Vec<InhibitorRow>>;
}

/// RAII puro: al soltar el fd, logind libera. Incluso con SIGKILL.
pub struct Inhibitor {
    _fd: std::os::fd::OwnedFd,
    _conn: Connection,
}

impl Inhibitor {
    pub fn acquire(what: &str, why: &str) -> color_eyre::Result<Self> {
        let conn = Connection::system()?;
        let mgr = LoginManagerProxyBlocking::new(&conn)?;
        let fd = mgr.inhibit(what, "nosleep", why, "block")?;
        Ok(Self {
            _fd: fd.into(),
            _conn: conn,
        })
    }
}

pub fn list() -> color_eyre::Result<Vec<InhibitorRow>> {
    let conn = Connection::system()?;
    Ok(LoginManagerProxyBlocking::new(&conn)?.list_inhibitors()?)
}
