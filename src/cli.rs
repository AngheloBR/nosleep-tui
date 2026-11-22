use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "nosleep",
    version,
    about = "Impide la suspensión de tu laptop · systemd-logind + X11",
    after_help = "Sin argumentos abre la interfaz interactiva (TUI)."
)]
pub struct Cli {
    /// Duración: 10m, 8h, 1h30m, 90 (=90 min), inf
    #[arg(short = 't', long, value_name = "DUR")]
    pub time: Option<String>,

    /// Mantener la pantalla encendida (X11 / DPMS)
    #[arg(short = 's', long)]
    pub screen: bool,

    /// Ignorar el cierre de la tapa
    #[arg(long)]
    pub lid: bool,

    /// Razón que registrará logind
    #[arg(long, default_value = "Sesión de trabajo activa (nosleep)")]
    pub reason: String,

    /// Inhibir solo mientras corre este comando
    #[arg(long = "while", value_name = "CMD", num_args = 1.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub while_cmd: Option<Vec<String>>,

    /// Restaurar la pantalla tras una muerte sucia
    #[arg(long)]
    pub restore: bool,

    /// Listar los inhibidores activos del sistema
    #[arg(long)]
    pub list: bool,

    /// Abrir la TUI usando -t, -s, --lid y --reason como valores iniciales
    #[arg(long, conflicts_with_all = ["while_cmd", "restore", "list"])]
    pub tui: bool,
}
