//! yaskkserv
//!
//! yaskkserv を起動して test する。 yaskkserv が存在しない場合は何もせず成功する。

use crate::skk::test_unix::{
    setup, wait_server, ConnectSendCompare, ConnectSendCompareRunParameter, Encoding, Path,
    Protocol, MANY_THREADS,
};

fn yaskkserv_core(
    name: &'static str,
    port: &'static str,
    is_sequential: bool,
    threads: usize,
    protocol: Protocol,
) {
    let jisyo_full_path = &Path::get_full_path_yaskkserv2_jisyo(Encoding::Euc);
    let yaskkserv_dictionary_full_path = &format!(
        "{}.yaskkserv.{}.{}",
        jisyo_full_path,
        threads,
        if is_sequential {
            "sequential"
        } else {
            "random"
        }
    );
    let mut child = match std::process::Command::new("yaskkserv_make_dictionary")
        .arg(jisyo_full_path)
        .arg(yaskkserv_dictionary_full_path)
        .spawn()
    {
        Ok(ok) => ok,
        Err(e) => {
            println!(
                "Error(test success): yaskkserv_make_dictionary  error={:?}",
                e
            );
            return;
        }
    };
    child.wait().unwrap();
    let child = match std::process::Command::new("yaskkserv")
        .arg("--no-daemonize")
        .arg(format!("--port={}", port))
        .arg(format!("--max-connection={}", threads))
        .arg(yaskkserv_dictionary_full_path)
        .spawn()
    {
        Ok(ok) => ok,
        Err(e) => {
            println!("Error(test success): yaskkserv  error={:?}", e);
            return;
        }
    };
    wait_server(port);
    let parameter = ConnectSendCompareRunParameter::new(jisyo_full_path, name, port, protocol)
        .encoding(Encoding::Euc)
        .is_yaskkserv(true)
        .is_compare(false)
        .is_sequential(is_sequential)
        .threads(threads);
    ConnectSendCompare::run(parameter);
    // FIXME! この kill だけでは処理できないケースが多々ある
    let _ = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(format!("{}", child.id()))
        .spawn()
        .unwrap();
}

//
// yaskkserv と yaskkserv_make_dictionary への path が通っている必要があることに注意。
//
// #[cfg(debug_ignore)]
#[test]
fn yaskkserv_benchmark_000_sequential_test() {
    let name = "yaskkserv_benchmark_000_sequential";
    setup::setup_and_wait(name);
    let port = "10100";
    yaskkserv_core(name, port, true, 1, Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv_benchmark_001_random_test() {
    let name = "yaskkserv_benchmark_001_random";
    setup::setup_and_wait(name);
    let port = "10101";
    yaskkserv_core(name, port, false, 1, Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv_benchmark_002_abbrev_sequential_test() {
    let name = "yaskkserv_benchmark_002_abbrev_sequential";
    setup::setup_and_wait(name);
    let port = "10102";
    yaskkserv_core(name, port, true, 1, Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv_benchmark_003_abbrev_random_test() {
    let name = "yaskkserv_benchmark_003_abbrev_random";
    setup::setup_and_wait(name);
    let port = "10103";
    yaskkserv_core(name, port, false, 1, Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv_benchmark_100_sequential_multithreads_test() {
    let name = "yaskkserv_benchmark_100_sequential_multithreads";
    setup::setup_and_wait(name);
    let port = "10110";
    yaskkserv_core(name, port, true, MANY_THREADS, Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv_benchmark_101_random_multithreads_test() {
    let name = "yaskkserv_benchmark_101_random_multithreads";
    setup::setup_and_wait(name);
    let port = "10111";
    yaskkserv_core(name, port, false, MANY_THREADS, Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv_benchmark_102_abbrev_sequential_multithreads_test() {
    let name = "yaskkserv_benchmark_102_abbrev_sequential_multithreads";
    setup::setup_and_wait(name);
    let port = "10112";
    yaskkserv_core(name, port, true, MANY_THREADS, Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv_benchmark_103_abbrev_random_multithreads_test() {
    let name = "yaskkserv_benchmark_103_abbrev_random_multithreads";
    setup::setup_and_wait(name);
    let port = "10113";
    yaskkserv_core(name, port, false, MANY_THREADS, Protocol::Protocol4);
    setup::exit();
}
