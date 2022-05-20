use regex::Regex;

use crate::skk::yaskkserv2::MAX_CONNECTION;
use crate::skk::{
    Config, GoogleTiming, SkkError, DEFAULT_CONFIG_FULL_PATH, DEFAULT_GOOGLE_CACHE_ENTRIES,
    DEFAULT_GOOGLE_CACHE_EXPIRE_SECONDS, DEFAULT_GOOGLE_MAX_CANDIDATES_LENGTH,
    DEFAULT_GOOGLE_TIMEOUT_MILLISECONDS, DEFAULT_HOSTNAME_AND_IP_ADDRESS_FOR_PROTOCOL_3,
    DEFAULT_LISTEN_ADDRESS, DEFAULT_MAX_CONNECTIONS, DEFAULT_MAX_SERVER_COMPLETIONS, DEFAULT_PORT,
    PKG_VERSION,
};

pub(in crate::skk) struct Yaskkserv2CommandLine {
    config: Config,
}

impl Yaskkserv2CommandLine {
    pub(in crate::skk) fn new() -> Self {
        Self {
            config: Config::new(),
        }
    }

    pub(in crate::skk) fn get_config(&self) -> Config {
        self.config.clone()
    }

    pub(in crate::skk) fn start(&mut self) -> Result<bool, SkkError> {
        let mut result_is_help_exit = false;
        let mut result_is_exit = false;
        let config_arg = format!(
            "--config-filename=[FILENAME] 'config filename (default: {})'",
            DEFAULT_CONFIG_FULL_PATH
        );
        let default_port = &DEFAULT_PORT.to_string();
        let default_max_connections = &DEFAULT_MAX_CONNECTIONS.to_string();
        let default_google_timeout_milliseconds = &DEFAULT_GOOGLE_TIMEOUT_MILLISECONDS.to_string();
        let default_google_cache_entries = &DEFAULT_GOOGLE_CACHE_ENTRIES.to_string();
        let default_google_cache_expire_seconds = &DEFAULT_GOOGLE_CACHE_EXPIRE_SECONDS.to_string();
        let default_google_max_candidates_length =
            &DEFAULT_GOOGLE_MAX_CANDIDATES_LENGTH.to_string();
        let default_max_server_completions = &DEFAULT_MAX_SERVER_COMPLETIONS.to_string();
        let mut app = app_from_crate!()
            .setting(clap::AppSettings::DeriveDisplayOrder)
            .arg(clap::Arg::from_usage("[dictionary] 'dictionary'")
                 .validator(|v| Self::dictionary_validator(&v)))
            .arg(clap::Arg::from_usage(&config_arg))
            .arg(clap::Arg::from_usage("--no-daemonize 'do not daemonize'"))
            .arg(clap::Arg::from_usage("--port=[PORT] 'port number'")
                 .validator(|v| Self::port_validator(&v))
                 .default_value(default_port))
            .arg(clap::Arg::from_usage("--max-connections=[MAX-CONNECTIONS] 'max connections'")
                 .validator(|v| Self::max_connections_validator(&v))
                 .default_value(default_max_connections))
            .arg(clap::Arg::from_usage("--listen-address=[LISTEN-ADDRESS] 'listen address'")
                 .validator(|v| Self::listen_address_validator(&v))
                 .default_value(DEFAULT_LISTEN_ADDRESS))
            .arg(clap::Arg::from_usage("--hostname-and-ip-address-for-protocol-3=[HOSTNAME:ADDR] 'hostname and ip address for protocol 3'")
                 .validator(|v| Self::hostname_and_ip_address_address_validator(&v))
                 .default_value(DEFAULT_HOSTNAME_AND_IP_ADDRESS_FOR_PROTOCOL_3))
            .arg(clap::Arg::from_usage("--google-timeout-milliseconds=[MILLISECONDS] 'google timeout milliseconds'")
                 .validator(|v| Self::google_timeout_milliseconds_validator(&v))
                 .default_value(default_google_timeout_milliseconds))
            .arg(clap::Arg::from_usage("--google-cache-filename=[FILENAME] 'google cache filename (default: disable)'"))
            .arg(clap::Arg::from_usage("--google-cache-entries=[ENTRIES] 'google cache entries'")
                 .validator(|v| Self::google_cache_entries_validator(&v))
                 .default_value(default_google_cache_entries))
            .arg(clap::Arg::from_usage("--google-cache-expire-seconds=[SECONDS] 'google cache expire seconds'")
                 .validator(|v| Self::google_cache_expire_seconds_validator(&v))
                 .default_value(default_google_cache_expire_seconds))
            .arg(clap::Arg::from_usage("--google-max-candidates-length=[LENGTH] 'google max candidates length'")
                 .validator(|v| Self::google_max_candidates_length_validator(&v))
                 .default_value(default_google_max_candidates_length))
            .arg(clap::Arg::from_usage("--max-server-completions=[MAX] 'max server completions'")
                 .validator(|v| Self::max_server_completions_validator(&v))
                 .default_value(default_max_server_completions))
            .arg(clap::Arg::from_usage("--google-japanese-input=[TIMING] 'enable google japanese input (default: notfound)'")
                 .possible_values(&["notfound", "disable", "last", "first"]))
            .arg(clap::Arg::from_usage("--google-suggest 'enable google suggest'"))
            .arg(clap::Arg::from_usage("--google-use-http 'use http (default: https)'"))
            .arg(clap::Arg::from_usage("--midashi-utf8 'use utf8 (default: euc)'"));
        let matches = app
            .get_matches_from_safe_borrow(std::env::args_os())
            .unwrap_or_else(|e| e.exit());
        self.setup(&matches, &mut result_is_help_exit, &mut result_is_exit);
        if result_is_help_exit {
            if app.print_help().is_err() {
                return Err(SkkError::CommandLine);
            }
            println!();
        }
        Ok(result_is_help_exit || result_is_exit)
    }

    pub(in crate::skk) fn dictionary_validator(value: &str) -> Result<(), String> {
        if std::path::Path::new(value).exists() {
            Ok(())
        } else {
            Err(format!(r#"dictionary "{}" not found"#, value))
        }
    }

    pub(in crate::skk) fn port_validator(value: &str) -> Result<(), String> {
        Self::range_validator::<i32>(value, "illegal port number", 0, 65535)
    }

    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub(in crate::skk) fn max_connections_validator(value: &str) -> Result<(), String> {
        Self::range_validator::<i32>(
            value,
            "illegal max connection range",
            1,
            MAX_CONNECTION as i32,
        )
    }

    pub(in crate::skk) fn listen_address_validator(value: &str) -> Result<(), String> {
        if value.parse::<std::net::IpAddr>().is_ok() {
            Ok(())
        } else {
            Err(String::from("illegal listen address"))
        }
    }

    pub(in crate::skk) fn hostname_and_ip_address_address_validator(
        value: &str,
    ) -> Result<(), String> {
        let re_ascii = Regex::new(r"^[\x21-\x7e]+$").unwrap();
        if re_ascii.is_match(value) {
            Ok(())
        } else {
            Err(String::from("illegal hostname/IP"))
        }
    }

    pub(in crate::skk) fn google_timeout_milliseconds_validator(value: &str) -> Result<(), String> {
        Self::range_validator::<u64>(value, "illegal timeout milliseconds", 0, 5 * 60 * 1000)
    }

    pub(in crate::skk) fn google_cache_entries_validator(value: &str) -> Result<(), String> {
        Self::range_validator::<usize>(value, "illegal cache entries", 1, 1024 * 1024)
    }

    pub(in crate::skk) fn google_cache_expire_seconds_validator(value: &str) -> Result<(), String> {
        Self::range_validator::<u64>(value, "illegal expire seconds", 1, 100 * 365 * 24 * 60 * 60)
    }

    pub(in crate::skk) fn google_max_candidates_length_validator(
        value: &str,
    ) -> Result<(), String> {
        Self::range_validator::<u64>(value, "illegal candidates length", 1, 1024)
    }

    pub(in crate::skk) fn max_server_completions_validator(value: &str) -> Result<(), String> {
        Self::range_validator::<i32>(value, "illegal max server completions", 1, 64 * 1024)
    }

    pub(in crate::skk) fn parse_integer<T: std::str::FromStr>(
        value: &str,
        fail_value: T,
        is_help_exit: &mut bool,
    ) -> T {
        value.parse::<T>().unwrap_or_else(|_| {
            *is_help_exit = true;
            fail_value
        })
    }

    pub(in crate::skk) fn range_validator<T: std::str::FromStr + std::cmp::PartialOrd + Copy>(
        value: &str,
        message: &str,
        min: T,
        max: T,
    ) -> Result<(), String> {
        value
            .parse::<T>()
            .map_err(|_e| String::from(message))
            .and_then(|ok| {
                if ok < min || ok > max {
                    Err(String::from(message))
                } else {
                    Ok(())
                }
            })
    }

    fn setup(
        &mut self,
        matches: &clap::ArgMatches<'_>,
        result_is_help_exit: &mut bool,
        result_is_exit: &mut bool,
    ) {
        if let Some(dictionary_full_path) = matches.value_of("dictionary") {
            self.config.dictionary_full_path = String::from(dictionary_full_path);
        }
        if matches.is_present("version") {
            println!("{}", PKG_VERSION);
            *result_is_exit = true;
        }
        if let Some(full_path) = matches.value_of("config-filename") {
            self.config.config_full_path = String::from(full_path);
        }
        if matches.is_present("no-daemonize") {
            self.config.is_no_daemonize = true;
        }
        if let Some(port) = matches.value_of("port") {
            self.config.port = String::from(port);
        }
        if let Some(max_connections) = matches.value_of("max-connections") {
            self.config.max_connections =
                Self::parse_integer(max_connections, 0, result_is_help_exit);
        }
        if let Some(listen_address) = matches.value_of("listen-address") {
            self.config.listen_address = String::from(listen_address);
        }
        if let Some(hostname_and_ip_address_for_protocol_3) =
            matches.value_of("hostname-and-ip-address-for-protocol-3")
        {
            self.config.hostname_and_ip_address_for_protocol_3 =
                String::from(hostname_and_ip_address_for_protocol_3);
        }
        if let Some(milliseconds) = matches.value_of("google-timeout-milliseconds") {
            self.config.google_timeout_milliseconds =
                Self::parse_integer(milliseconds, 0, result_is_help_exit);
        }
        if let Some(full_path) = matches.value_of("google-cache-filename") {
            self.config.is_google_cache_enabled = true;
            self.config.google_cache_full_path = String::from(full_path);
        }
        if let Some(entries) = matches.value_of("google-cache-entries") {
            self.config.google_cache_entries = Self::parse_integer(entries, 0, result_is_help_exit);
        }
        if let Some(seconds) = matches.value_of("google-cache-expire-seconds") {
            self.config.google_cache_expire_seconds =
                Self::parse_integer(seconds, 0, result_is_help_exit);
        }
        if let Some(completions) = matches.value_of("max-server-completions") {
            self.config.max_server_completions =
                Self::parse_integer(completions, 0, result_is_help_exit);
        }
        if matches.value_of("google-japanese-input").is_some() {
            match matches.value_of("google-japanese-input") {
                Some("notfound") => self.config.google_timing = GoogleTiming::NotFound,
                Some("disable") => self.config.google_timing = GoogleTiming::Disable,
                Some("last") => self.config.google_timing = GoogleTiming::Last,
                Some("first") => self.config.google_timing = GoogleTiming::First,
                _ => *result_is_help_exit = true,
            }
        }
        if matches.is_present("google-use-http") {
            if self.config.google_timing == GoogleTiming::Disable {
                *result_is_help_exit = true;
            } else {
                self.config.is_http_enabled = true;
            }
        }
        if matches.is_present("midashi-utf8") {
            self.config.is_midashi_utf8 = true;
        }
        if matches.is_present("google-suggest") {
            if self.config.google_timing == GoogleTiming::Disable {
                *result_is_help_exit = true;
            } else {
                self.config.is_google_suggest_enabled = true;
            }
        }
    }
}
