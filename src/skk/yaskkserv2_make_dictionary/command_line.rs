use crate::skk::*;

#[derive(Default)]
pub(in crate::skk) struct Yaskkserv2MakeDictionaryCommandLine {
    config: Config,
    jisyo_full_paths: Vec<String>,
    output_jisyo_full_path: String,
    input_cache_full_path: String,
}

impl Yaskkserv2MakeDictionaryCommandLine {
    pub(in crate::skk) fn new() -> Yaskkserv2MakeDictionaryCommandLine {
        Yaskkserv2MakeDictionaryCommandLine {
            config: Config::new(),
            ..Default::default()
        }
    }

    pub(in crate::skk) fn get_config(&self) -> Config {
        self.config.clone()
    }

    pub(in crate::skk) fn get_jisyo_full_paths(&self) -> Vec<String> {
        self.jisyo_full_paths.clone()
    }

    pub(in crate::skk) fn get_output_jisyo_full_path(&self) -> &str {
        &self.output_jisyo_full_path
    }

    pub(in crate::skk) fn get_input_cache_full_path(&self) -> &str {
        &self.input_cache_full_path
    }

    pub(in crate::skk) fn start(&mut self) -> Result<bool, SkkError> {
        let mut result_is_help_exit = false;
        let result_is_exit = false;
        let mut app = app_from_crate!()
            .setting(clap::AppSettings::DeriveDisplayOrder)
            .arg(
                clap::Arg::from_usage("[jisyo] 'SKK-JISYO (EUC or UTF8)'")
                    .validator(Self::jisyo_validator)
                    .multiple(true),
            )
            .arg(clap::Arg::from_usage(
                "--dictionary-filename=[FILENAME] 'dictionary filename'",
            ))
            .arg(clap::Arg::from_usage(
                "--cache-filename=[FILENAME] 'cache filename'",
            ))
            .arg(clap::Arg::from_usage("--utf8 'create utf8 dictionary'"))
            .arg(clap::Arg::from_usage(
                "--output-jisyo-filename=[FILENAME] 'output jisyo filename'",
            ))
            .arg(clap::Arg::from_usage("--verbose 'verbose mode'"))
            .group(
                clap::ArgGroup::with_name("source-filename")
                    .args(&["dictionary-filename", "cache-filename"]),
            );
        let matches = app
            .get_matches_from_safe_borrow(std::env::args_os())
            .unwrap_or_else(|e| e.exit());
        if !Self::is_unique_jisyo_args(&matches) {
            println!("Warning: SAME JISYO FOUND");
        }
        if let Some(jisyo_full_paths) = matches.values_of("jisyo") {
            self.jisyo_full_paths = jisyo_full_paths
                .map(|x| x.to_string())
                .collect::<Vec<String>>();
        }
        self.config.encoding = if matches.is_present("utf8") {
            Encoding::Utf8
        } else {
            Encoding::Euc
        };
        if let Some(full_path) = matches.value_of("dictionary-filename") {
            self.config.dictionary_full_path = String::from(full_path);
        }
        if let Some(full_path) = matches.value_of("cache-filename") {
            self.input_cache_full_path = String::from(full_path);
        }
        if let Some(full_path) = matches.value_of("output-jisyo-filename") {
            self.output_jisyo_full_path = String::from(full_path);
        }
        self.config.is_verbose = matches.is_present("verbose");
        if self.jisyo_full_paths.is_empty() && self.output_jisyo_full_path.is_empty() {
            result_is_help_exit = true;
        }
        if result_is_help_exit {
            if app.print_help().is_err() {
                return Err(SkkError::CommandLine);
            }
            println!();
        }
        Ok(result_is_help_exit || result_is_exit)
    }

    // ファイルの実体が同一かどうかは見ていないことに注意
    fn is_unique_jisyo_args(matches: &clap::ArgMatches<'_>) -> bool {
        if let Some(s) = matches.values_of("jisyo") {
            let hash_set: FxHashSet<&str> = s.clone().collect::<FxHashSet<&str>>();
            return s.len() == hash_set.len();
        }
        true
    }

    fn jisyo_validator(value: String) -> Result<(), String> {
        if !std::path::Path::new(&value).exists() {
            Err(format!(r#"jisyo "{}" not found"#, &value))
        } else {
            Ok(())
        }
    }
}
