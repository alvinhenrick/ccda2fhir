//! `ccda2fhir`: convert C-CDA documents to FHIR from the command line, or run the HTTP service.

mod server;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use ccda2fhir::{convert, Options};
use ccda_map::Profiles;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version, about = "Convert C-CDA documents to FHIR R4 bundles")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum ProfileChoice {
    /// Declare US Core 6.1.0 profiles where the data supports them.
    UsCore,
    /// Declare no profiles.
    None,
}

#[derive(Subcommand)]
enum Command {
    /// Convert one or more C-CDA files. With one file and no --out-dir, the bundle goes to stdout.
    Convert {
        /// C-CDA XML files.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Write `<name>.fhir.json` here instead of next to the input.
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "us-core")]
        profiles: ProfileChoice,
        /// A LOINC `Loinc.csv` (from your own LOINC download) to set result categories.
        #[arg(long, env = "LOINC_TABLE")]
        loinc_table: Option<PathBuf>,
    },
    /// Run the HTTP service (POST /convert, GET /healthz, GET /readyz).
    Serve {
        #[arg(long, env = "BIND_ADDR", default_value = "127.0.0.1:8080")]
        bind: SocketAddr,
        /// Largest accepted request body in bytes.
        #[arg(long, env = "MAX_BODY_BYTES", default_value_t = 10 * 1024 * 1024)]
        max_body_bytes: usize,
        /// Conversions that may run at once.
        #[arg(long, env = "MAX_CONCURRENCY", default_value_t = 64)]
        max_concurrency: usize,
        /// Seconds allowed per request.
        #[arg(long, env = "REQUEST_TIMEOUT_SECS", default_value_t = 30)]
        request_timeout_secs: u64,
        /// A LOINC `Loinc.csv` (from your own LOINC download) to set result categories.
        #[arg(long, env = "LOINC_TABLE")]
        loinc_table: Option<PathBuf>,
    },
    /// Exit 0 if a running service answers /healthz (for container health checks).
    Healthcheck {
        #[arg(long, env = "HEALTHCHECK_ADDR", default_value = "127.0.0.1:8080")]
        addr: SocketAddr,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Convert {
            files,
            out_dir,
            profiles,
            loinc_table,
        } => match load_loinc(loinc_table.as_deref()) {
            Ok(loinc) => run_convert(&files, out_dir.as_deref(), profiles, loinc),
            Err(code) => code,
        },
        Command::Serve {
            bind,
            max_body_bytes,
            max_concurrency,
            request_timeout_secs,
            loinc_table,
        } => match load_loinc(loinc_table.as_deref()) {
            Ok(loinc) => run_serve(
                bind,
                server::Config {
                    max_body_bytes,
                    max_concurrency,
                    request_timeout: Duration::from_secs(request_timeout_secs),
                    loinc,
                },
            ),
            Err(code) => code,
        },
        Command::Healthcheck { addr } => run_healthcheck(addr),
    }
}

/// Read the optional LOINC table. A table that cannot be read or has no class column is an error,
/// not a silent fallback: the caller asked for it.
fn load_loinc(
    path: Option<&Path>,
) -> Result<Option<std::sync::Arc<ccda_terminology::LoincTable>>, ExitCode> {
    let Some(path) = path else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path).map_err(|e| {
        eprintln!(
            "{}: cannot read the LOINC table: {}",
            path.display(),
            e.kind()
        );
        ExitCode::FAILURE
    })?;
    match ccda_terminology::LoincTable::from_csv(&text) {
        Some(table) if !table.is_empty() => Ok(Some(std::sync::Arc::new(table))),
        _ => {
            eprintln!(
                "{}: not a LOINC table (needs LOINC_NUM and CLASSTYPE columns)",
                path.display()
            );
            Err(ExitCode::FAILURE)
        }
    }
}

fn run_convert(
    files: &[PathBuf],
    out_dir: Option<&Path>,
    profiles: ProfileChoice,
    loinc: Option<std::sync::Arc<ccda_terminology::LoincTable>>,
) -> ExitCode {
    let options = Options {
        loinc,
        profiles: match profiles {
            ProfileChoice::UsCore => Profiles::UsCore610,
            ProfileChoice::None => Profiles::None,
        },
        ..Options::default()
    };
    let to_stdout = files.len() == 1 && out_dir.is_none();
    let mut failed = false;
    for file in files {
        // File names are the caller's own; document content is never printed.
        let xml = match std::fs::read(file) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("{}: cannot read: {}", file.display(), e.kind());
                failed = true;
                continue;
            }
        };
        let conversion = match convert(&xml, &options) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: {e}", file.display());
                failed = true;
                continue;
            }
        };
        let mut json = match serde_json::to_string_pretty(&conversion.bundle.to_json()) {
            Ok(j) => j,
            Err(_) => {
                eprintln!("{}: could not write the bundle", file.display());
                failed = true;
                continue;
            }
        };
        json.push('\n');
        if to_stdout {
            if std::io::stdout().write_all(json.as_bytes()).is_err() {
                failed = true;
            }
        } else {
            let name = file
                .file_stem()
                .map_or_else(|| "bundle".to_owned(), |s| s.to_string_lossy().into_owned());
            let dir = out_dir
                .map(Path::to_path_buf)
                .or_else(|| file.parent().map(Path::to_path_buf))
                .unwrap_or_default();
            let target = dir.join(format!("{name}.fhir.json"));
            if let Err(e) = std::fs::write(&target, json) {
                eprintln!("{}: cannot write: {}", target.display(), e.kind());
                failed = true;
                continue;
            }
            eprintln!(
                "{} -> {} ({} report entries)",
                file.display(),
                target.display(),
                conversion.report.issues.len()
            );
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_serve(bind: SocketAddr, config: server::Config) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("cannot start the runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(bind).await?;
        tracing::info!(%bind, "listening");
        axum::serve(listener, server::router(&config))
            .with_graceful_shutdown(shutdown_signal())
            .await
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("server error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutting down");
}

fn run_healthcheck(addr: SocketAddr) -> ExitCode {
    let ok = (|| -> std::io::Result<bool> {
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response.starts_with("HTTP/1.1 200"))
    })()
    .unwrap_or(false);
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
