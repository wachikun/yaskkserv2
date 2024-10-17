use regex::Regex;
use rustc_hash::FxHashSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::sync::RwLock;

use crate::skk::test_unix::{Path, INIT_MUTEX_LOCK};
use crate::skk::yaskkserv2_make_dictionary::Yaskkserv2MakeDictionary;
use crate::skk::{encoding_simple, once_init_encoding_table, Config, Encoding};

static ONCE_INIT: std::sync::Once = std::sync::Once::new();

static RE_OPENSSL_MD5: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"= ([0-9a-f]{32})").unwrap());
static RE_DOT_MD5_FILE: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"^([0-9a-f]{32})").unwrap());
static RE_URL_FILENAME: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"/([^/]+)$").unwrap());
static EUC_JISYO_FULL_PATHS: once_cell::sync::Lazy<RwLock<Vec<String>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));
static UTF8_JISYO_FULL_PATHS: once_cell::sync::Lazy<RwLock<Vec<String>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));
static TEST_RUNNING_COUNT: once_cell::sync::Lazy<RwLock<usize>> =
    once_cell::sync::Lazy::new(|| RwLock::new(0));
type PanicDefaultHook = once_cell::sync::Lazy<
    RwLock<Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + 'static + Sync + Send>>,
>;
static PANIC_DEFAULT_HOOK: PanicDefaultHook =
    once_cell::sync::Lazy::new(|| RwLock::new(std::panic::take_hook()));
static PANIC_THREAD_NAME_SET: once_cell::sync::Lazy<RwLock<FxHashSet<String>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(FxHashSet::default()));

pub(in crate::skk) struct JisyoDownloader;

impl JisyoDownloader {
    pub(in crate::skk) fn get_jisyo_full_paths(encoding: Encoding) -> Vec<String> {
        match encoding {
            Encoding::Euc => EUC_JISYO_FULL_PATHS.read().unwrap().to_vec(),
            Encoding::Utf8 => UTF8_JISYO_FULL_PATHS.read().unwrap().to_vec(),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn download_and_extract() -> Vec<String> {
        let download_urls = [
            (Encoding::Euc, "https://raw.githubusercontent.com/skk-dev/dict/master/SKK-JISYO.hukugougo"),
            (Encoding::Euc, "https://raw.githubusercontent.com/skk-dev/dict/master/SKK-JISYO.notes"),
            (Encoding::Utf8, "https://raw.githubusercontent.com/uasi/skk-emoji-jisyo/master/SKK-JISYO.emoji.utf8"),
            (Encoding::Utf8, "https://raw.githubusercontent.com/dyama/skk-dict-kancolle/master/src/SKK-JISYO.kancolle"),
        ];
        let download_md5_infos = [
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.L.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.jinmei.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.fullname.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.geo.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.propernoun.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.station.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.law.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.okinawa.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.china_taiwan.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.assoc.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.JIS2.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.JIS3_4.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.JIS2004.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.itaiji.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.itaiji.JIS3_4.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.mazegaki.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.lisp.gz.md5",
                vec![],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/SKK-JISYO.edict.tar.gz.md5",
                vec!["SKK-JISYO.edict"],
            ),
            (
                Encoding::Euc,
                "https://skk-dev.github.io/dict/zipcode.tar.gz.md5",
                vec![
                    "zipcode/SKK-JISYO.office.zipcode",
                    "zipcode/SKK-JISYO.zipcode",
                ],
            ),
        ];
        let mut jisyo_full_paths = Vec::new();
        for info in &download_md5_infos {
            let full_path = Self::download_md5_and_archive(info.1, &info.2);
            jisyo_full_paths.extend(full_path.clone());
            match info.0 {
                Encoding::Euc => EUC_JISYO_FULL_PATHS.write().unwrap().extend(full_path),
                Encoding::Utf8 => UTF8_JISYO_FULL_PATHS.write().unwrap().extend(full_path),
            }
        }
        for download_url in &download_urls {
            let full_path = Self::download_simple(download_url.1);
            jisyo_full_paths.push(full_path.clone());
            match download_url.0 {
                Encoding::Euc => EUC_JISYO_FULL_PATHS.write().unwrap().push(full_path),
                Encoding::Utf8 => UTF8_JISYO_FULL_PATHS.write().unwrap().push(full_path),
            }
        }
        for jisyo in &jisyo_full_paths {
            assert!(std::path::Path::new(jisyo).exists());
        }
        jisyo_full_paths
    }

    fn get_extract_flag_directory_full_path(full_path: &str) -> String {
        format!("{full_path}.extracted")
    }

    fn is_extracted(full_path: &str) -> bool {
        std::path::Path::new(&Self::get_extract_flag_directory_full_path(full_path)).exists()
    }

    fn create_extract_flag_directory(full_path: &str) {
        std::fs::create_dir_all(Self::get_extract_flag_directory_full_path(full_path)).unwrap();
    }

    fn get_convert_dictionary_flag_directory_full_path() -> String {
        Path::get_full_path("convert_dictionary.converted")
    }

    fn is_converted() -> bool {
        std::path::Path::new(&Self::get_convert_dictionary_flag_directory_full_path()).exists()
    }

    fn create_convert_dictionary_flag_directory() {
        std::fs::create_dir_all(Self::get_convert_dictionary_flag_directory_full_path()).unwrap();
    }

    fn download_simple(download_url: &str) -> String {
        let base_filename = RE_URL_FILENAME
            .captures(download_url)
            .map_or_else(|| panic!("url format error"), |m| String::from(&m[1]));
        let full_path = Path::get_full_path(&base_filename);
        if Self::is_extracted(&full_path) {
            println!("extracted url={download_url}");
        } else {
            println!("download url={download_url}");
            Self::download(download_url, &full_path);
            Self::correct(&full_path);
            Self::create_extract_flag_directory(&full_path);
        }
        full_path
    }

    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    fn download_md5_and_archive(download_md5_url: &str, jisyo_filenames: &[&str]) -> Vec<String> {
        let mut jisyo_full_paths = Vec::new();
        assert!(download_md5_url.ends_with(".md5"));
        let base_md5_filename = RE_URL_FILENAME
            .captures(download_md5_url)
            .map_or_else(|| panic!("url format error"), |m| String::from(&m[1]));
        let md5_full_path = &Path::get_full_path(&base_md5_filename);
        let archive_full_path = md5_full_path.trim_end_matches(".md5");
        if jisyo_filenames.is_empty() {
            jisyo_full_paths.push(String::from(archive_full_path.trim_end_matches(".gz")));
        } else {
            jisyo_full_paths.extend(jisyo_filenames.iter().map(|v| Path::get_full_path(v)));
        }
        if Self::is_extracted(md5_full_path) {
            println!("extracted url={download_md5_url}");
        } else {
            let download_archive_url = download_md5_url.trim_end_matches(".md5");
            println!("curl download md5 url={download_md5_url}");
            Self::download(download_md5_url, md5_full_path);
            println!("curl download archive url={download_archive_url}");
            Self::download(download_archive_url, archive_full_path);
            Self::compare_openssl_md5(md5_full_path);
            Self::extract(archive_full_path);
            for full_path in &jisyo_full_paths {
                Self::correct(full_path);
            }
            Self::create_extract_flag_directory(md5_full_path);
        }
        jisyo_full_paths
    }

    fn correct_terminate_lf(full_path: &str) {
        let mut buffer = vec![0; 1];
        {
            let mut reader = File::open(full_path).unwrap();
            reader.seek(std::io::SeekFrom::End(-1)).unwrap();
            reader.read_exact(&mut buffer).unwrap();
        }
        if buffer[0] != b'\n' {
            let mut writer = OpenOptions::new().write(true).open(full_path).unwrap();
            writer.seek(std::io::SeekFrom::End(0)).unwrap();
            writer.write_all(b"\n").unwrap();
            writer.flush().unwrap();
        }
    }

    fn correct(full_path: &str) {
        Self::correct_terminate_lf(full_path);
    }

    fn download(download_url: &str, full_path: &str) {
        let mut child = std::process::Command::new("curl")
            .arg("--silent")
            .arg("-o")
            .arg(full_path)
            .arg(download_url)
            .spawn()
            .unwrap();
        child.wait().unwrap();
    }

    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    fn extract(archive_full_path: &str) {
        if archive_full_path.ends_with(".tar.gz") {
            println!("extract tar.gz full_path={archive_full_path}");
            let mut child = std::process::Command::new("tar")
                .arg("-C")
                .arg(Path::get_full_path(""))
                .arg("-xf")
                .arg(archive_full_path)
                .spawn()
                .unwrap();
            child.wait().unwrap();
        } else if archive_full_path.ends_with(".gz") {
            println!("extract gz full_path={archive_full_path}");
            let mut child = std::process::Command::new("gzip")
                .arg("-d")
                .arg(archive_full_path)
                .spawn()
                .unwrap();
            child.wait().unwrap();
        } else {
            panic!("unknown archive full_path={archive_full_path}");
        }
    }

    fn compare_openssl_md5(dot_md5_full_path: &str) {
        let dot_md5_file_string;
        {
            let mut read_string = String::new();
            let mut reader = File::open(dot_md5_full_path).unwrap();
            reader.read_to_string(&mut read_string).unwrap();
            dot_md5_file_string = RE_DOT_MD5_FILE
                .captures(&read_string)
                .map_or_else(|| panic!("md5 file format error"), |m| String::from(&m[1]));
        }
        let openssl_md5_string;
        {
            let openssl_arg_full_path = dot_md5_full_path.trim_end_matches(".md5");
            let output = std::process::Command::new("openssl")
                .arg("md5")
                .arg(openssl_arg_full_path)
                .output()
                .unwrap();
            openssl_md5_string = RE_OPENSSL_MD5
                .captures(&String::from_utf8(output.stdout).unwrap())
                .map_or_else(
                    || panic!("openssl md5 format error"),
                    |m| String::from(&m[1]),
                );
        }
        assert_eq!(dot_md5_file_string, openssl_md5_string);
    }
}

struct OnceInit;

impl OnceInit {
    fn create_dictionary(config: &Config, jisyo_full_paths: &[String]) {
        Yaskkserv2MakeDictionary::run_create_dictionary(
            config,
            encoding_simple::EncodingTable::get(),
            jisyo_full_paths,
        )
        .unwrap();
    }

    fn create_jisyo(config: &Config, jisyo_full_paths: &str) {
        Yaskkserv2MakeDictionary::run_create_jisyo(config, jisyo_full_paths).unwrap();
    }

    fn convert_dictionary_and_jisyo(config: &Config, jisyo_full_paths: &[String]) {
        for jisyo_full_path in jisyo_full_paths {
            println!("    {jisyo_full_path}");
            {
                let mut cloned_config = config.clone();
                cloned_config.encoding = Encoding::Euc;
                cloned_config.dictionary_full_path = format!("{jisyo_full_path}.dictionary");
                Self::create_dictionary(&cloned_config, &[String::from(jisyo_full_path)]);
            }
            {
                let mut cloned_config = config.clone();
                cloned_config.encoding = Encoding::Utf8;
                cloned_config.dictionary_full_path = format!("{jisyo_full_path}.dictionary.utf8");
                Self::create_dictionary(&cloned_config, &[String::from(jisyo_full_path)]);
            }
        }
        {
            let mut cloned_config = config.clone();
            cloned_config.encoding = Encoding::Euc;
            cloned_config.dictionary_full_path =
                Path::get_full_path_yaskkserv2_dictionary(cloned_config.encoding);
            Self::create_dictionary(&cloned_config, jisyo_full_paths);
        }
        {
            let mut cloned_config = config.clone();
            cloned_config.encoding = Encoding::Utf8;
            cloned_config.dictionary_full_path =
                Path::get_full_path_yaskkserv2_dictionary(cloned_config.encoding);
            Self::create_dictionary(&cloned_config, jisyo_full_paths);
        }
        {
            let mut cloned_config = config.clone();
            let encoding = Encoding::Euc;
            cloned_config.encoding = encoding;
            cloned_config.dictionary_full_path =
                Path::get_full_path_yaskkserv2_dictionary(cloned_config.encoding);
            Self::create_jisyo(
                &cloned_config,
                &Path::get_full_path_yaskkserv2_jisyo(encoding),
            );
        }
        {
            let mut cloned_config = config.clone();
            let encoding = Encoding::Utf8;
            cloned_config.encoding = encoding;
            cloned_config.dictionary_full_path =
                Path::get_full_path_yaskkserv2_dictionary(cloned_config.encoding);
            Self::create_jisyo(
                &cloned_config,
                &Path::get_full_path_yaskkserv2_jisyo(encoding),
            );
        }
    }

    fn cargo_build() {
        let mut child = std::process::Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--bin=yaskkserv2")
            .spawn()
            .unwrap();
        child.wait().unwrap();
        let mut child = std::process::Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--bin=yaskkserv2_make_dictionary")
            .spawn()
            .unwrap();
        child.wait().unwrap();
    }

    fn compile_echo_server() {
        let echo_server_c_source = r#"/* This file was automatically generated by echo_server_c_server()@echo_server.rs */
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char *argv[]) {
        if (argc != 3) {
                printf("illegal command line\n");
                printf("$ echo_server PORT LOOP\n");
                exit(EXIT_FAILURE);
        }
        const char address[] = "127.0.0.1";
        unsigned short port = atoi(argv[1]);
        int loop = atoi(argv[2]);
        struct sockaddr_in server_address;
        struct sockaddr_in client_address;
        memset(&server_address, 0, sizeof(server_address));
        int socket_fd = socket(AF_INET, SOCK_STREAM, 0);
        if (socket_fd < 0) {
                printf("socket() failed  port=%d\n", port);
                exit(EXIT_FAILURE);
        }
        server_address.sin_family = AF_INET;
        if (!inet_aton(address, &server_address.sin_addr)) {
                printf("inet_aton() failed  port=%d\n", port);
                exit(EXIT_FAILURE);
        }
        server_address.sin_port = htons(port);
        socklen_t length = sizeof(struct sockaddr_in);
        if (bind(socket_fd, (struct sockaddr*)&server_address, length) < 0) {
                printf("bind() failed  port=%d\n", port);
                exit(EXIT_FAILURE);
        }
        if (listen(socket_fd, SOMAXCONN) < 0) {
                printf("listen() failed  port=%d\n", port);
                exit(EXIT_FAILURE);
        }
        for (int i = 0; i < loop; ++i) {
                int fd = accept(socket_fd, (struct sockaddr*)&client_address, &length);
                if (fd < 0) {
                        printf("accept() failed  port=%d\n", port);
                        exit(EXIT_FAILURE);
                }
                /* close(socket_fd); */
                int buffer_size = 256 * 1024;
                char buffer[buffer_size];
                for (;;) {
                        ssize_t recv_size = recv(fd, buffer, buffer_size, 0);
                        if (recv_size == 0) {
                                break;
                        } else if (recv_size == -1) {
                                printf("recv() failed  port=%d\n", port);
                                exit(EXIT_FAILURE);
                        } else {
                                if (write(fd, buffer, recv_size) < 0) {
                                        printf("write() failed  port=%d\n", port);
                                        exit(EXIT_FAILURE);
                                }
                        }
                }
        }
        return EXIT_SUCCESS;
}
"#;
        let source_full_path = &Path::get_full_path_echo_server_source();
        let mut writer = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(source_full_path)
            .unwrap();
        writer.write_all(echo_server_c_source.as_bytes()).unwrap();
        let echo_server_full_path = &Path::get_full_path_echo_server();
        let mut child = match std::process::Command::new("gcc")
            .arg("-Ofast")
            .arg(source_full_path)
            .arg("-o")
            .arg(echo_server_full_path)
            .spawn()
        {
            Ok(ok) => ok,
            Err(e) => {
                println!("Error(test success): gcc/compile  error={e:?}");
                return;
            }
        };
        child.wait().unwrap();
    }

    /// `std::panic::set_hook()` 直前の `std::panic::take_hook()` は必須であることに注意。
    /// (`lazy_static` 定義部の `std::panic::take_hook()` だけでは `std::panic::set_hook()` 後に
    ///  設定されてしまう)
    fn setup_panic_hook() {
        *PANIC_DEFAULT_HOOK.write().unwrap() = std::panic::take_hook();
        std::panic::set_hook(Box::new(|panic_info| {
            let handle = std::thread::current();
            let name = String::from(handle.name().unwrap());
            if !PANIC_THREAD_NAME_SET.read().unwrap().contains(&name) {
                PANIC_THREAD_NAME_SET.write().unwrap().insert(name);
                exit();
            }
            PANIC_DEFAULT_HOOK.read().unwrap()(panic_info);
        }));
    }

    fn once_init() {
        ONCE_INIT.call_once(|| {
            Self::setup_panic_hook();
            std::fs::create_dir_all(Path::get_full_path_test_base()).unwrap();
            once_init_encoding_table(encoding_simple::EncodingTable::get());
            let jisyo_full_paths = JisyoDownloader::download_and_extract();
            let config = Config::new();
            if JisyoDownloader::is_converted() {
                println!("converted already");
            } else {
                Self::convert_dictionary_and_jisyo(&config, &jisyo_full_paths);
                JisyoDownloader::create_convert_dictionary_flag_directory();
            }
            Self::compile_echo_server();
            Self::cargo_build();
        });
    }
}

pub(in crate::skk) fn setup_and_wait(test_name: &str) {
    println!("wait setup {test_name}");
    let _init_lock = INIT_MUTEX_LOCK.lock().unwrap();
    OnceInit::once_init();
    {
        let mut count = TEST_RUNNING_COUNT.write().unwrap();
        *count += 1;
    }
    println!("start {test_name}");
}

pub(in crate::skk) fn exit() {
    let mut count = TEST_RUNNING_COUNT.write().unwrap();
    *count -= 1;
}

pub(in crate::skk) fn get_test_running_count() -> usize {
    let count = TEST_RUNNING_COUNT.read().unwrap();
    *count
}
