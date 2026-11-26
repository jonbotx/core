use std::io;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::{Directive, LevelFilter};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

use crate::cli::api::tauri_bindings::{TauriEmitter, TauriHandle, TauriLogEvent};

/// Creates a tracing layer that writes to a rolling file appender.
macro_rules! json_rolling_layer {
    ($dir:expr, $prefix:expr, $env_filter:expr, $max_files:expr) => {
        json_rolling_layer!($dir, $prefix, $env_filter, $max_files, Rotation::HOURLY)
    };
    ($dir:expr, $prefix:expr, $env_filter:expr, $max_files:expr, $rotation:expr) => {{
        let appender: RollingFileAppender = RollingFileAppender::builder()
            .rotation($rotation)
            .filename_prefix($prefix)
            .filename_suffix("log")
            .max_log_files($max_files)
            .build($dir)
            .expect("initializing rolling file appender failed");

        fmt::layer()
            .with_writer(appender)
            .with_ansi(false)
            .with_timer(UtcTime::rfc_3339())
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
            .json()
            .with_filter($env_filter?)
    }};
}

/// Output formats for logging messages.
pub enum Format {
    /// Standard, human readable format.
    Raw,
    /// JSON, machine readable format.
    Json,
}

/// Initialize tracing and enable logging messages according to these options.
/// Besides printing to `stdout`, this will append to a log file.
/// Said file will contain JSON-formatted logs of all levels,
/// disregarding the arguments to this function. When `trace_stdout` is `true`,
/// all tracing logs are also emitted to stdout.
pub fn init(
    format: Format,
    dir: impl AsRef<Path>,
    tauri_handle: Option<TauriHandle>,
    trace_stdout: bool,
) -> Result<()> {
    // Write our crates to the general log file at DEBUG level.
    //
    // This file used to be opened with `Rotation::never`, i.e. it never rotated
    // and grew without bound for the entire lifetime of the installation. On a
    // busy node that is easily tens of GB (see upstream #893). Rotate daily and
    // keep a week so the log directory stays bounded.
    let file_layer = {
        let file_appender: RollingFileAppender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("swap-all")
            .filename_suffix("log")
            .max_log_files(7)
            .build(&dir)
            .expect("initializing rolling file appender failed");

        fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)
            .with_timer(UtcTime::rfc_3339())
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
            .json()
            .with_filter(env_filter_with_all_crates(vec![(
                crates::OUR_CRATES.to_vec(),
                LevelFilter::DEBUG,
            )])?)
    };

    // Write our crates to a verbose log file (tracing*.log)
    let tracing_file_layer = json_rolling_layer!(
        &dir,
        "tracing",
        env_filter_with_all_crates(vec![(crates::OUR_CRATES.to_vec(), verbose_level())]),
        12
    );

    // Write Tor/arti to a verbose log file (tracing-tor*.log).
    // Rotates every minute and keeps the last 5 files: TRACE-level tor logs are
    // extremely voluminous (`tor_proto` alone can produce >10 MB/min during
    // HS failures), so we deliberately keep only a short rolling window.
    let tor_file_layer = json_rolling_layer!(
        &dir,
        "tracing-tor",
        env_filter_with_all_crates(vec![(crates::TOR_CRATES.to_vec(), verbose_level())]),
        5,
        Rotation::MINUTELY
    );

    // Write libp2p to a verbose log file (tracing-libp2p*.log)
    let libp2p_file_layer = json_rolling_layer!(
        &dir,
        "tracing-libp2p",
        env_filter_with_all_crates(vec![(crates::LIBP2P_CRATES.to_vec(), verbose_level())]),
        6
    );

    // Write monero wallet crates to a verbose log file (tracing-monero-wallet*.log)
    let monero_wallet_file_layer = json_rolling_layer!(
        &dir,
        "tracing-monero-wallet",
        env_filter_with_all_crates(vec![(
            crates::MONERO_WALLET_CRATES.to_vec(),
            verbose_level()
        )]),
        6
    );

    // Layer for writing to the terminal
    let terminal_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_file(true)
        .with_line_number(true);

    // Layer for writing to the Tauri guest. This will be displayed in the GUI.
    // Crates: All crates with libp2p at INFO+ level
    // Level: Passed in for our crates, INFO for libp2p
    let tauri_layer = fmt::layer()
        .with_writer(TauriWriter::new(tauri_handle))
        .with_ansi(false)
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .with_filter(env_filter_with_all_crates(vec![
            (crates::OUR_CRATES.to_vec(), LevelFilter::TRACE),
            (crates::MONERO_WALLET_CRATES.to_vec(), LevelFilter::INFO),
            (crates::LIBP2P_CRATES.to_vec(), LevelFilter::INFO),
            (crates::TOR_CRATES.to_vec(), LevelFilter::INFO),
        ])?);

    // If trace_stdout is true, we log our crates at TRACE level, others at INFO level
    // Otherwise, we only log our crates at INFO level
    let terminal_layer_env_filter = match trace_stdout {
        true => env_filter_with_all_crates(vec![
            (crates::OUR_CRATES.to_vec(), LevelFilter::TRACE),
            (crates::MONERO_WALLET_CRATES.to_vec(), LevelFilter::INFO),
            (crates::LIBP2P_CRATES.to_vec(), LevelFilter::INFO),
            (crates::TOR_CRATES.to_vec(), LevelFilter::INFO),
        ])?,
        false => {
            env_filter_with_all_crates(vec![(crates::OUR_CRATES.to_vec(), LevelFilter::INFO)])?
        }
    };

    let final_terminal_layer = match format {
        Format::Json => terminal_layer
            .json()
            .with_filter(terminal_layer_env_filter)
            .boxed(),
        Format::Raw => terminal_layer
            .with_filter(terminal_layer_env_filter)
            .boxed(),
    };

    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(tracing_file_layer)
        .with(tor_file_layer)
        .with(libp2p_file_layer)
        .with(monero_wallet_file_layer)
        .with(final_terminal_layer)
        .with(tauri_layer);

    subscriber.try_init()?;

    // Now we can use the tracing macros to log messages
    tracing::info!(
        logs_dir = %dir.as_ref().display(),
        "Initialized tracing. General logs go to swap-all.log*; verbose logs: tracing*.log (ours), tracing-tor*.log (tor), tracing-libp2p*.log (libp2p), tracing-monero-wallet*.log. Verbose level is DEBUG by default; set EIGEN_LOG_LEVEL=TRACE for maximum detail"
    );

    Ok(())
}

/// This function controls which crate's logs actually get logged and from which level, including all crate categories.
fn env_filter_with_all_crates(crates: Vec<(Vec<&str>, LevelFilter)>) -> Result<EnvFilter> {
    let mut filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::OFF.into())
        .from_env_lossy();

    // Add directives for each group of crates with their specified level filter
    for (crate_names, level_filter) in crates {
        for crate_name in crate_names {
            filter = filter.add_directive(Directive::from_str(&format!(
                "{}={}",
                crate_name, &level_filter
            ))?);
        }
    }

    Ok(filter)
}

/// Level used for the per-subsystem verbose log files (`tracing*.log`).
///
/// These layers used to capture third-party crates (libp2p, tor, the Monero
/// wallet) at TRACE, which produced several GB of logs per hour and, together
/// with the never-rotating `swap-all.log`, was the main reason users' log
/// directories grew into the tens of GB (upstream #893). Default to `DEBUG`,
/// which is still enough to diagnose connection/swap issues, and let the
/// operator raise or lower it via the `EIGEN_LOG_LEVEL` environment variable
/// (e.g. `EIGEN_LOG_LEVEL=TRACE`) when deep debugging is needed.
fn verbose_level() -> LevelFilter {
    std::env::var("EIGEN_LOG_LEVEL")
        .ok()
        .and_then(|v| LevelFilter::from_str(v.trim()).ok())
        .unwrap_or(LevelFilter::DEBUG)
}

mod crates {
    pub const TOR_CRATES: &[&str] = &[
        "arti",
        "arti_client",
        "tor_guardmgr",
        "tor_circmgr",
        "tor_chanmgr",
        "tor_hsclient",
        "tor_hscrypto",
        "tor_hsservice",
        "tor_dirmgr",
        "tor_dirclient",
        "tor_dircommon",
        "tor_netdir",
        "tor_netdoc",
        "tor_linkspec",
        "tor_ptmgr",
        "tor_rtcompat",
        "tor_persist",
        "tor_consdiff",
        "tor_error",
        "tor_log_ratelim",
    ];

    pub const LIBP2P_CRATES: &[&str] = &[
        "libp2p",
        "libp2p_swarm",
        "libp2p_core",
        // Protocols
        "libp2p_identify",
        "libp2p_ping",
        "libp2p_request_response",
        "libp2p_rendezvous",
        // Transports
        "libp2p_dns",
        "libp2p_yamux",
        "libp2p_tor",
        "libp2p_tcp",
        // TODO: Maybe add "swap_p2p" here too?
    ];

    pub const OUR_CRATES: &[&str] = &[
        // Library crates
        "bitcoin_wallet",
        "monero_wallet",
        "monero_wallet_ng",
        "swap_p2p",
        "swap_env",
        "swap_core",
        "swap_fs",
        "swap_serde",
        "swap_feed",
        "monero_sys",
        "tracing_ext",
        // Binary crates
        "swap",
        "asb",
        "unstoppableswap_gui_rs",
    ];

    pub const MONERO_WALLET_CRATES: &[&str] = &["monero_cpp", "monero_rpc_pool"];
}

/// A writer that forwards tracing log messages to the tauri guest.
#[derive(Clone)]
pub struct TauriWriter {
    tauri_handle: Option<TauriHandle>,
}

impl TauriWriter {
    /// Create a new Tauri writer that sends log messages to the tauri guest.
    pub fn new(tauri_handle: Option<TauriHandle>) -> Self {
        Self { tauri_handle }
    }
}

/// This is needed for tracing to accept this as a writer.
impl<'a> MakeWriter<'a> for TauriWriter {
    type Writer = TauriWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// For every write issued by tracing we simply pass the string on as an event to the tauri guest.
impl std::io::Write for TauriWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Since this function accepts bytes, we need to pass to utf8 first
        let owned_buf = buf.to_owned();
        let utf8_string = String::from_utf8(owned_buf)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        // Then send to tauri
        self.tauri_handle.emit_cli_log_event(TauriLogEvent {
            buffer: utf8_string,
        });

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // No-op, we don't need to flush anything
        Ok(())
    }
}
