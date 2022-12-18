//! ddskk
//!
//! emacs を起動して ddskk から test する。 emacs が存在しない場合は何もせず成功する。
//!
//! ddskk 内部で lock されて止まる事があるので `TEST_MUTEX_LOCK.lock()` で lock し、 test を
//! 同時に走らせていないことに注意。

#![allow(clippy::non_ascii_literal)]

use std::fs::OpenOptions;
use std::io::Write;

use crate::skk::test_unix::{get_take_count, setup, wait_server, Path, TEST_MUTEX_LOCK};
use crate::skk::yaskkserv2::Yaskkserv2;
use crate::skk::{Config, Encoding, GoogleTiming};

use crate::skk::yaskkserv2::test_unix::Yaskkserv2Debug;

struct DaredevilSkk {
    name: String,
    config: Config,
}

impl DaredevilSkk {
    fn new(name: &str, config: &Config) -> Self {
        Self {
            name: String::from(name),
            config: config.clone(),
        }
    }

    fn run_elisp(identifier: &str, elisp_str: &str) -> Result<(), std::io::Error> {
        let elisp_full_path = Path::get_full_path(&format!("elisp.{identifier}.tmp.el"));
        let mut writer = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&elisp_full_path)
            .unwrap();
        writer.write_all(elisp_str.as_bytes()).unwrap();
        writer.flush().unwrap();
        let mut child = match std::process::Command::new("emacs")
            .arg("--script")
            .arg(elisp_full_path)
            .spawn()
        {
            Ok(ok) => ok,
            Err(e) => return Err(e),
        };
        child.wait().unwrap();
        Ok(())
    }

    fn run(&mut self, dictionary_full_path: &str, elisp_str: &str) {
        {
            let mut child = match std::process::Command::new("emacs").arg("--version").spawn() {
                Ok(ok) => ok,
                Err(e) => {
                    println!("Error(test success): emacs  error={e:?}");
                    return;
                }
            };
            child.wait().unwrap();
        }
        self.config.dictionary_full_path = String::from(dictionary_full_path);
        self.config.is_no_daemonize = true;
        let thread_config = self.config.clone();
        let thread_handle = std::thread::Builder::new()
            .name(String::from(std::thread::current().name().unwrap()))
            .spawn(move || {
                let mut core = Yaskkserv2::new();
                core.setup(&thread_config).unwrap();
                let threads = 1;
                core.run_test(get_take_count(threads));
            })
            .unwrap();
        wait_server(&self.config.port);
        Self::run_elisp(&format!("{}.{}", self.config.port, self.name), elisp_str).unwrap();
        thread_handle.join().unwrap();
    }
}

#[test]
fn ddskk_server_version_test() {
    let name = "ddskk_server_version_test";
    println!("wait lock {name}");
    let _test_lock = TEST_MUTEX_LOCK.lock();
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("13000"));
    let mut ddskk = DaredevilSkk::new(name, &config);
    ddskk.run(
        &Path::get_full_path_yaskkserv2_dictionary(config.encoding),
        &format!(
            r#"#!/usr/bin/emacs --script
(require 'skk-autoloads)
(setq skk-server-portnum {})
(skk-mode)
(message "%s" (skk-server-version))
(skk-disconnect-server)
"#,
            config.port
        ),
    );
    setup::exit();
}

// euc で受信する test
//
// 受信できれば良いので candidates が一致するかどうかは調べていないことに注意。
#[test]
fn ddskk_euc_test() {
    let name = "ddskk_euc_test";
    println!("wait lock {name}");
    let _test_lock = TEST_MUTEX_LOCK.lock();
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("13001"))
        .encoding(Encoding::Euc)
        .google_timing(GoogleTiming::Last);
    let mut ddskk = DaredevilSkk::new(name, &config);
    ddskk.run(
        &Path::get_full_path_yaskkserv2_dictionary(config.encoding),
        &format!(
            r#"#!/usr/bin/emacs --script
(require 'skk-autoloads)
(setq skk-server-portnum {})
(skk-mode)
(let ((skk-henkan-key "かんこれ"))
  (message "skk-search-server=%s"
           (skk-search-server nil nil)))
(skk-disconnect-server)
"#,
            config.port
        ),
    );
    setup::exit();
}

// utf8 で受信する test
//
// 受信できれば良いので candidates が一致するかどうかは調べていないことに注意。
#[test]
fn ddskk_utf8_test() {
    let name = "ddskk_utf8_test";
    println!("wait lock {name}");
    let _test_lock = TEST_MUTEX_LOCK.lock();
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("13002"))
        .encoding(Encoding::Utf8)
        .google_timing(GoogleTiming::Last);
    let mut ddskk = DaredevilSkk::new(name, &config);
    ddskk.run(&Path::get_full_path_yaskkserv2_dictionary(config.encoding),
            &format!(
                r#"#!/usr/bin/emacs --script
(require 'skk-autoloads)
(setq skk-server-portnum {})
(defun skk-open-server-decoding-utf-8 ()
  "辞書サーバと接続する。サーバープロセスを返す。 decoding coding-system が euc ではなく utf8 となる。"
  (unless (skk-server-live-p)
    (setq skkserv-process (skk-open-server-1))
    (when (skk-server-live-p)
      (let ((code (cdr (assoc "euc" skk-coding-system-alist))))
	(set-process-coding-system skkserv-process 'utf-8 code))))
  skkserv-process)
(setq skk-mode-hook
      '(lambda()
         (advice-add 'skk-open-server :override 'skk-open-server-decoding-utf-8)))
(skk-mode)
(let ((skk-henkan-key "かんこれ"))
  (message "skk-search-server=%s"
           (skk-search-server nil nil)))
(skk-disconnect-server)
"#,
                config.port
            ),
    );
    setup::exit();
}

// 指定した SKK 辞書全ての midashi で変換して candidates が一致することを調べる test
//
// ddskk 経由での test は遅いので、小さな辞書を使用していることに注意。
#[test]
fn ddskk_skk_jisyo_test() {
    let name = "ddskk_skk_jisyo";
    println!("wait lock {name}");
    let _test_lock = TEST_MUTEX_LOCK.lock();
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("13003"))
        .encoding(Encoding::Euc);
    let base_filename = "SKK-JISYO.JIS2004";
    let jisyo_full_path = Path::get_full_path(base_filename);
    let dictionary_full_path = Path::get_full_path(&format!(
        "{}.dictionary{}",
        base_filename,
        if config.encoding == Encoding::Euc {
            ""
        } else {
            ".utf8"
        }
    ));
    let mut ddskk = DaredevilSkk::new(name, &config);
    ddskk.run(
        &dictionary_full_path,
        &format!(
            r#"#!/usr/bin/emacs --script
(require 'skk-autoloads)
(setq skk-server-portnum {})
(skk-mode)
(setq skk-server-report-response nil)
(with-temp-buffer
  (insert-file-contents "{jisyo_full_path}")
  (goto-char (point-min))
  (let ((count 0))
    (catch 'loop
      (while (< (point) (point-max))
        (when (looking-at "^\\([^ ]+\\) \\(.+\\)")
          (let ((skk-henkan-key (buffer-substring-no-properties (match-beginning 1) (match-end 1)))
	        (candidates (buffer-substring-no-properties (match-beginning 2) (match-end 2))))
	    (unless (or (string-prefix-p ";" skk-henkan-key)
		        (string-match "[0-9]\\|[０-９]" skk-henkan-key))
              (let ((result (concat "/" (mapconcat #'identity (skk-search-server nil nil) "/") "/")))
	        (unless (string= result candidates)
		  (message "differ skk-henkan-key=%s candidates=%s result=%s" skk-henkan-key candidates result)))
              (setq count (1- count))
              (when (< count 0)
                (message "skk-henkan-key=%s" skk-henkan-key)
                (setq count 200))))
	  (forward-line 1))))))
(skk-disconnect-server)
"#,
            config.port
        ),
    );
    setup::exit();
}
