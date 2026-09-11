use std::{ffi::OsString, path::PathBuf, time::Duration};

pub const MAX_REQUEST_LINE_SIZE: usize = 8 * 1024;
pub const MAX_HEADER_SIZE: usize = 32 * 1024;
pub const DEFAULT_MAX_BODY_SIZE: usize = 1024 * 1024;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhpFpmEndpoint {
    Tcp(String),
    #[cfg(unix)]
    Unix(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub address: String,
    pub document_root: PathBuf,
    pub php_fpm: PhpFpmEndpoint,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub max_body_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:8080".to_string(),
            document_root: PathBuf::from("./www"),
            php_fpm: PhpFpmEndpoint::Tcp("127.0.0.1:9000".to_string()),
            read_timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
            write_timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
            max_body_size: DEFAULT_MAX_BODY_SIZE,
        }
    }
}

impl Config {
    pub fn from_args<I, T>(arguments: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut config = Self::default();
        let mut arguments = arguments.into_iter().map(Into::into);
        let _program_name = arguments.next();

        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| "command-line arguments must be valid UTF-8".to_string())?;

            if argument == "--help" || argument == "-h" {
                return Err(Self::usage());
            }

            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}\n\n{}", Self::usage()))?;
            let value = value
                .into_string()
                .map_err(|_| format!("value for {argument} must be valid UTF-8"))?;

            match argument.as_str() {
                "--address" => config.address = value,
                "--document-root" => config.document_root = PathBuf::from(value),
                "--php-fpm" => config.php_fpm = PhpFpmEndpoint::Tcp(value),
                #[cfg(unix)]
                "--php-fpm-unix" => {
                    config.php_fpm = PhpFpmEndpoint::Unix(PathBuf::from(value));
                }
                "--read-timeout-seconds" => {
                    config.read_timeout = Duration::from_secs(parse_positive(&argument, &value)?);
                }
                "--write-timeout-seconds" => {
                    config.write_timeout = Duration::from_secs(parse_positive(&argument, &value)?);
                }
                "--max-body-bytes" => {
                    config.max_body_size = value
                        .parse::<usize>()
                        .ok()
                        .filter(|number| *number > 0)
                        .ok_or_else(|| format!("{argument} must be a positive integer"))?;
                }
                _ => return Err(format!("unknown argument: {argument}\n\n{}", Self::usage())),
            }
        }

        Ok(config)
    }

    pub fn usage() -> String {
        format!(
            "Usage: thesis-rust-server [options]\n\
             \n\
             Options:\n\
               --address ADDRESS                 HTTP listen address (default 127.0.0.1:8080)\n\
               --document-root PATH              document root (default ./www)\n\
               --php-fpm ADDRESS                 PHP-FPM TCP endpoint (default 127.0.0.1:9000)\n\
               --php-fpm-unix PATH               PHP-FPM Unix socket (Linux/Unix only)\n\
               --read-timeout-seconds NUMBER     read timeout (default {DEFAULT_TIMEOUT_SECONDS})\n\
               --write-timeout-seconds NUMBER    write timeout (default {DEFAULT_TIMEOUT_SECONDS})\n\
               --max-body-bytes NUMBER           request body limit (default {DEFAULT_MAX_BODY_SIZE})"
        )
    }
}

fn parse_positive(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|number| *number > 0)
        .ok_or_else(|| format!("{name} must be a positive integer"))
}
