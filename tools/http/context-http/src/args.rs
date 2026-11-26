//! Command-line flags of the server, shared by the `context-http` binary
//! and `context-cli serve`.

use std::{
    net::{
        SocketAddr,
        ToSocketAddrs,
    },
    path::PathBuf,
    time::Duration,
};

use context_api::SaveMode;

use crate::{
    ServerConfig,
    DEFAULT_ADDR,
    DEFAULT_MAX_VISITED_CAP,
};

/// When the server saves graphs (`--save`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveArg {
    /// After every mutating request.
    OnRead,
    /// Dirty graphs periodically.
    Interval(Duration),
    /// Only on `save` (and shutdown).
    Manual,
}

/// Flags of `context-http` / `context-cli serve`.
#[derive(Debug, Clone, clap::Args)]
pub struct ServeArgs {
    /// Graphs to open at start (default: opened on first use).
    #[arg(value_name = "GRAPH")]
    pub graphs: Vec<String>,

    /// Listen address; port 0 binds an ephemeral port (printed on stdout).
    #[arg(long = "http", value_name = "ADDR", default_value = DEFAULT_ADDR, value_parser = parse_addr)]
    pub http: SocketAddr,

    /// Graph store directory.
    #[arg(
        long,
        value_name = "DIR",
        env = "CONTEXT_STORE",
        default_value = ".context-engine"
    )]
    pub store: PathBuf,

    /// on-read | interval=30s | manual. Every policy saves on shutdown.
    #[arg(long, value_name = "POLICY", default_value = "on-read", value_parser = parse_save)]
    pub save: SaveArg,

    /// Bearer token; required when ADDR is not loopback.
    #[arg(
        long,
        value_name = "TOKEN",
        env = "CONTEXT_TOKEN",
        hide_env_values = true
    )]
    pub token: Option<String>,

    /// Allow `file` inputs below DIR (default: refused).
    #[arg(long, value_name = "DIR")]
    pub allow_fs: Option<PathBuf>,

    /// Request body limit, e.g. 64MiB, 512KB, 1048576.
    #[arg(long, value_name = "SIZE", default_value = "64MiB", value_parser = parse_size)]
    pub max_body: usize,

    /// Hard vertex cap of render/export, not liftable by `force`.
    #[arg(long, value_name = "N", default_value_t = crate::DEFAULT_RENDER_CAP)]
    pub render_cap: usize,

    /// Allowed CORS origin (repeatable; default: no CORS).
    #[arg(long = "cors-origin", value_name = "ORIGIN")]
    pub cors_origins: Vec<String>,

    /// Serve a frontend from DIR (SPA fallback to index.html).
    #[arg(long = "static", value_name = "DIR")]
    pub static_dir: Option<PathBuf>,
}

impl ServeArgs {
    pub fn into_config(self) -> ServerConfig {
        let (save, save_interval) = match self.save {
            SaveArg::OnRead => (SaveMode::OnWrite, None),
            SaveArg::Interval(period) => (SaveMode::Manual, Some(period)),
            SaveArg::Manual => (SaveMode::Manual, None),
        };
        ServerConfig {
            store_dir: self.store,
            addr: self.http,
            preload: self.graphs,
            save,
            save_interval,
            token: self.token.filter(|token| !token.is_empty()),
            allow_fs: self.allow_fs,
            max_body: self.max_body,
            render_cap: Some(self.render_cap),
            max_visited_cap: Some(DEFAULT_MAX_VISITED_CAP),
            cors_origins: self.cors_origins,
            static_dir: self.static_dir,
        }
    }
}

/// `HOST:PORT` (names resolved), or a bare port on 127.0.0.1.
pub fn parse_addr(text: &str) -> Result<SocketAddr, String> {
    if let Ok(port) = text.parse::<u16>() {
        return Ok(SocketAddr::from(([127, 0, 0, 1], port)));
    }
    text.to_socket_addrs()
        .map_err(|err| format!("invalid address '{text}': {err}"))?
        .next()
        .ok_or_else(|| format!("address '{text}' resolves to nothing"))
}

/// `on-read`, `manual` or `interval=DURATION` (`30s`, `5m`, `500ms`, `1h`;
/// a bare number is seconds).
pub fn parse_save(text: &str) -> Result<SaveArg, String> {
    match text {
        "on-read" | "on_read" => Ok(SaveArg::OnRead),
        "manual" => Ok(SaveArg::Manual),
        "interval" => Ok(SaveArg::Interval(Duration::from_secs(30))),
        _ => match text.strip_prefix("interval=") {
            Some(duration) => {
                let duration = parse_duration(duration)?;
                if duration.is_zero() {
                    return Err("the save interval must be positive".into());
                }
                Ok(SaveArg::Interval(duration))
            },
            None => Err(format!(
                "invalid save policy '{text}': expected on-read, interval=30s or manual"
            )),
        },
    }
}

fn parse_duration(text: &str) -> Result<Duration, String> {
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: u64 = number
        .parse()
        .map_err(|_| format!("invalid duration '{text}'"))?;
    match unit {
        "ms" => Ok(Duration::from_millis(number)),
        "" | "s" => Ok(Duration::from_secs(number)),
        "m" => Ok(Duration::from_secs(number * 60)),
        "h" => Ok(Duration::from_secs(number * 3600)),
        _ => Err(format!("invalid duration '{text}': use ms, s, m or h")),
    }
}

/// A byte size: `1048576`, `512KB`/`512K` (×1000), `64MiB` (×1024²), ...
pub fn parse_size(text: &str) -> Result<usize, String> {
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: usize = number
        .parse()
        .map_err(|_| format!("invalid size '{text}'"))?;
    let factor: usize = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" => 1_000,
        "kib" => 1 << 10,
        "m" | "mb" => 1_000_000,
        "mib" => 1 << 20,
        "g" | "gb" => 1_000_000_000,
        "gib" => 1 << 30,
        _ =>
            return Err(format!(
                "invalid size '{text}': use B, KB, KiB, MB, MiB, GB or GiB"
            )),
    };
    number
        .checked_mul(factor)
        .ok_or_else(|| format!("size '{text}' is too large"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags() {
        assert_eq!(parse_size("64MiB"), Ok(64 << 20));
        assert_eq!(parse_size("512KB"), Ok(512_000));
        assert_eq!(parse_size("100"), Ok(100));
        assert!(parse_size("1XB").is_err());
        assert_eq!(parse_save("on-read"), Ok(SaveArg::OnRead));
        assert_eq!(parse_save("manual"), Ok(SaveArg::Manual));
        assert_eq!(
            parse_save("interval=30s"),
            Ok(SaveArg::Interval(Duration::from_secs(30)))
        );
        assert_eq!(
            parse_save("interval=250ms"),
            Ok(SaveArg::Interval(Duration::from_millis(250)))
        );
        assert!(parse_save("interval=0s").is_err());
        assert!(parse_save("sometimes").is_err());
        assert_eq!(parse_addr("0").unwrap().to_string(), "127.0.0.1:0");
        assert_eq!(
            parse_addr("127.0.0.1:7878").unwrap().to_string(),
            "127.0.0.1:7878"
        );
    }
}
