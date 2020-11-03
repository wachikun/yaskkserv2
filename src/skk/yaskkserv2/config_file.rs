use regex::Regex;
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::skk::{yaskkserv2, Config, GoogleTiming, SkkError};

#[derive(Default)]
pub(in crate::skk) struct Yaskkserv2ConfigFile {
    config: Config,
    default_config: Config,
}

impl Yaskkserv2ConfigFile {
    pub(in crate::skk) fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            default_config: Config::new(),
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub(in crate::skk) fn get_config(self) -> Config {
        self.config
    }

    pub(in crate::skk) fn read(&mut self) -> Result<(), SkkError> {
        let file = File::open(&self.config.config_full_path);
        if file.is_err() {
            return Ok(());
        }
        let re_comment = Regex::new(r"^\s*[;#]").unwrap();
        let mut candidates = FxHashMap::default();
        for line_result in BufReader::new(file.unwrap()).lines() {
            let line = line_result?;
            if line.len() < 2 || re_comment.is_match(&line) {
                continue;
            }
            if let Some(m) = Regex::new(r"^\s*([^=\s]+)\s*=\s*(.+)")
                .unwrap()
                .captures(&line)
            {
                candidates.insert(String::from(&m[1]), String::from(&m[2]));
            }
        }
        if let Err(e) = self.validate_and_set_config(&candidates) {
            println!("{}", e);
            return Err(SkkError::CommandLine);
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn validate_and_set_config(
        &mut self,
        candidates: &FxHashMap<String, String>,
    ) -> Result<(), String> {
        macro_rules! validate_and_set_config_google_bool {
            ($key: tt, $field: ident) => {
                if candidates.contains_key($key) && self.config.$field == self.default_config.$field
                {
                    let tmp = candidates[$key].to_owned();
                    let is_enabled = Regex::new(r"^(?i)\s*enable\s*$").unwrap().is_match(&tmp);
                    if is_enabled && self.config.google_timing == GoogleTiming::Disable {
                        return Err("illegal combination".to_owned());
                    }
                    self.config.$field = is_enabled;
                }
            };
        }

        let mut parse_integer_result = false;

        macro_rules! validate_and_set_config_integer {
            ($key: tt, $field: ident, $validator: ident) => {
                if candidates.contains_key($key) && self.config.$field == self.default_config.$field
                {
                    let tmp = candidates[$key].to_owned();
                    yaskkserv2::command_line::Yaskkserv2CommandLine::$validator(tmp.to_owned())?;
                    self.config.$field =
                        yaskkserv2::command_line::Yaskkserv2CommandLine::parse_integer(
                            &tmp,
                            0,
                            &mut parse_integer_result,
                        );
                }
            };
        }

        {
            let key = "dictionary";
            if candidates.contains_key(key) && self.config.dictionary_full_path.is_empty() {
                let tmp = candidates[key].to_owned();
                yaskkserv2::command_line::Yaskkserv2CommandLine::dictionary_validator(
                    tmp.to_owned(),
                )?;
                self.config.dictionary_full_path = tmp;
            }
        }
        {
            let key = "port";
            if candidates.contains_key(key) && self.config.port == self.default_config.port {
                let tmp = candidates[key].to_owned();
                yaskkserv2::command_line::Yaskkserv2CommandLine::port_validator(tmp.to_owned())?;
                self.config.port = tmp;
            }
        }
        validate_and_set_config_integer!(
            "max-connections",
            max_connections,
            max_connections_validator
        );
        {
            let key = "listen-address";
            if candidates.contains_key(key)
                && self.config.listen_address == self.default_config.listen_address
            {
                let tmp = candidates[key].to_owned();
                yaskkserv2::command_line::Yaskkserv2CommandLine::listen_address_validator(
                    tmp.to_owned(),
                )?;
                self.config.listen_address = tmp;
            }
        }
        {
            let key = "hostname-and-ip-address-for-protocol-3";
            if candidates.contains_key(key)
                && self.config.hostname_and_ip_address_for_protocol_3
                    == self.default_config.hostname_and_ip_address_for_protocol_3
            {
                let tmp = candidates[key].to_owned();
                yaskkserv2::command_line::Yaskkserv2CommandLine::hostname_and_ip_address_address_validator(
                tmp.to_owned(),
            )?;
                self.config.hostname_and_ip_address_for_protocol_3 = tmp;
            }
        }
        validate_and_set_config_integer!(
            "google-timeout-milliseconds",
            google_timeout_milliseconds,
            google_timeout_milliseconds_validator
        );
        {
            let key = "google-cache-filename";
            if candidates.contains_key(key)
                && self.config.google_cache_full_path == self.default_config.google_cache_full_path
            {
                let tmp = candidates[key].to_owned();
                self.config.google_cache_full_path = tmp;
            }
        }
        validate_and_set_config_integer!(
            "google-cache-entries",
            google_cache_entries,
            google_cache_entries_validator
        );
        validate_and_set_config_integer!(
            "google-cache-expire-seconds",
            google_cache_expire_seconds,
            google_cache_expire_seconds_validator
        );
        validate_and_set_config_integer!(
            "google-max-candidates-length",
            google_max_candidates_length,
            google_max_candidates_length_validator
        );
        validate_and_set_config_integer!(
            "max-server-completions",
            max_server_completions,
            max_server_completions_validator
        );
        {
            let key = "google-japanese-input";
            if candidates.contains_key(key)
                && self.config.google_timing == self.default_config.google_timing
            {
                let tmp = candidates[key].to_owned();
                #[allow(clippy::match_same_arms)]
                let timing = match &tmp[..] {
                    "notfound" => GoogleTiming::NotFound,
                    "disable" => GoogleTiming::Disable,
                    "last" => GoogleTiming::Last,
                    "first" => GoogleTiming::First,
                    _ => GoogleTiming::NotFound,
                };
                self.config.google_timing = timing;
            }
        }
        validate_and_set_config_google_bool!("google-use-http", is_http_enabled);
        validate_and_set_config_google_bool!("google-suggest", is_google_suggest_enabled);
        validate_and_set_config_google_bool!(
            "google-insert-hiragana-only-candidate",
            google_insert_hiragana_only_candidate
        );
        validate_and_set_config_google_bool!(
            "google-insert-katakana-only-candidate",
            google_insert_katakana_only_candidate
        );
        validate_and_set_config_google_bool!(
            "google-insert-hankaku-katakana-only-candidate",
            google_insert_hankaku_katakana_only_candidate
        );
        Ok(())
    }
}
