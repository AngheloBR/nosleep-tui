//! Inhibidor de logind por D-Bus. zbus es asíncrono; este programa es síncrono,
//! así que cada llamada se ejecuta con `zbus::block_on`.

pub use zbus::Connection;
use zbus::OwnedFd;

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

/// RAII puro: el bloqueo vive en el fd, no en la conexión D-Bus.
/// Al cerrarse (drop, salida o SIGKILL), logind lo libera.
pub struct Inhibitor {
    _fd: std::os::fd::OwnedFd,
}

impl Inhibitor {
    pub fn acquire(what: &str, why: &str) -> color_eyre::Result<Self> {
        let fd = zbus::block_on(async {
            let conn = Connection::system().await?;
            LoginManagerProxy::new(&conn)
                .await?
                .inhibit(what, "nosleep", why, "block")
                .await
        })?;
        Ok(Self { _fd: fd.into() })
    }
}

pub fn connect() -> color_eyre::Result<Connection> {
    Ok(zbus::block_on(Connection::system())?)
}

pub fn list() -> color_eyre::Result<Vec<InhibitorRow>> {
    list_on(&connect()?)
}

/// Igual que [`list`] pero reutiliza una conexión (la TUI consulta cada pocos segundos).
pub fn list_on(conn: &Connection) -> color_eyre::Result<Vec<InhibitorRow>> {
    Ok(zbus::block_on(async {
        LoginManagerProxy::new(conn).await?.list_inhibitors().await
    })?)
}
