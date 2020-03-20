use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::mpsc;

use crate::skk::test_unix::*;
use crate::skk::yaskkserv2::DictionaryReader;
use crate::skk::yaskkserv2::Yaskkserv2;
use crate::skk::Config;

#[cfg(test)]
use crate::skk::yaskkserv2::test_unix::Yaskkserv2Debug;

fn run_and_wait_simple_server(config: &Config, take_count: usize) -> std::thread::JoinHandle<()> {
    let thread_handle = {
        let thread_config = config.clone().is_no_daemonize(true);
        std::thread::Builder::new()
            .name(String::from(std::thread::current().name().unwrap()))
            .spawn(move || {
                let mut core = Yaskkserv2::new();
                core.setup(&thread_config).unwrap();
                core.run_test(take_count);
            })
            .unwrap()
    };
    wait_server(&config.port);
    thread_handle
}

fn get_top_character_map(encoding: Encoding) -> BTreeMap<DictionaryMidashiKey, Vec<u8>> {
    let mut top_character_map = BTreeMap::new();
    let mut tmp_character_map = FxHashMap::default();
    {
        // 見出しを取得する辞書は EUC であることに注意
        let jisyo_entries = read_jisyo_entries_no_encoding_conversion(
            &Path::get_full_path_yaskkserv2_jisyo(Encoding::Euc),
        );
        for entry in &jisyo_entries {
            if let Some(space_find) = twoway::find_bytes(entry, b" ") {
                let midashi = &entry[..space_find];
                if !DictionaryReader::is_okuri_ari(midashi) {
                    let dictionary_midashi_key =
                        Dictionary::get_dictionary_midashi_key(entry).unwrap();
                    tmp_character_map
                        .entry(dictionary_midashi_key)
                        .or_insert_with(|| Vec::new())
                        .push(midashi.to_vec());
                }
            } else {
                panic!("illegal entry");
            }
        }
    }
    for (key, value) in tmp_character_map.iter_mut() {
        value.sort();
        let quoted = value
            .iter()
            .map(|v| Candidates::quote_and_add_prefix(v, None))
            .collect::<Vec<Vec<u8>>>();
        let concat = if encoding == Encoding::Euc {
            quoted.join(&b'/')
        } else {
            encoding_simple::Euc::decode(&quoted.join(&b'/')).unwrap()
        };
        top_character_map.insert(*key, concat);
    }
    top_character_map
}

fn test_abbrev(port: &str, encoding: Encoding) {
    let config = Config::new()
        .port(String::from(port))
        .encoding(encoding)
        .dictionary_full_path(Path::get_full_path_yaskkserv2_dictionary(encoding))
        .max_server_completions(64 * 1024);
    let threads = 1;
    let thread_handle = run_and_wait_simple_server(&config, get_take_count(threads));
    let top_character_map = get_top_character_map(encoding);
    match TcpStream::connect(format!("localhost:{}", config.port)) {
        Ok(stream) => {
            let mut buffer_stream = BufReader::new(&stream);
            for (dictionary_midashi_key, concat) in top_character_map.iter() {
                let mut send_data = vec![b'4'];
                match dictionary_midashi_key[0] {
                    0xa1..=0xfe | 0x8e => send_data.extend_from_slice(&dictionary_midashi_key[..2]),
                    0x00..=0x7f => send_data.push(dictionary_midashi_key[0]),
                    0x8f => send_data.extend_from_slice(&dictionary_midashi_key[..3]),
                    _ => panic!("illegal dictionary_midashi_key"),
                }
                send_data.push(b' ');
                buffer_stream.get_mut().write_all_flush(&send_data).unwrap();
                let mut buffer = Vec::new();
                match buffer_stream.read_until(b'\n', &mut buffer) {
                    Ok(size) => {
                        let trimmed = Candidates::trim_one_slash(&buffer[1..size - 1]);
                        // assert_eq!() だと対象が大き過ぎて log が酷いことになるので自前で比較表示
                        if &concat[..] != trimmed {
                            const PRINT_LENGTH: usize = 10;
                            panic!(
                                "compare error left head={:x?} right head={:x?}",
                                &concat[..std::cmp::min(PRINT_LENGTH, concat.len())],
                                &trimmed[..std::cmp::min(PRINT_LENGTH, trimmed.len())]
                            );
                        }
                    }
                    Err(e) => panic!("error e={}", e),
                }
            }
            buffer_stream.get_mut().write_all_flush(b"0").unwrap();
        }
        Err(e) => panic!("error e={}", e),
    }
    thread_handle.join().unwrap();
}

fn test_dictionary_notfound_google_found(name: &str, port: &str, is_google_suggest_enabled: bool) {
    fn is_consecutive_slashes(buffer: &[u8]) -> bool {
        let mut previous = 0;
        for current in buffer {
            if *current == b'/' && previous == b'/' {
                return true;
            }
            previous = *current;
        }
        false
    }
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from(port))
        .dictionary_full_path(Path::get_full_path_yaskkserv2_dictionary(Encoding::Utf8))
        .is_google_suggest_enabled(is_google_suggest_enabled);
    let threads = 1;
    let thread_handle = run_and_wait_simple_server(&config, get_take_count(threads));
    match TcpStream::connect(format!("localhost:{}", port)) {
        Ok(stream) => {
            let mut buffer_stream = BufReader::new(&stream);
            // test 実装時、「なすがまま」は dictionary に存在せず google API では返ってくる
            // 将来も google API で返ってくるかは不明であることに注意
            let send_data_euc_hiragana_nasugamama = vec![
                b'1', 0xa4, 0xca, 0xa4, 0xb9, 0xa4, 0xac, 0xa4, 0xde, 0xa4, 0xde, b' ',
            ];
            buffer_stream
                .get_mut()
                .write_all_flush(&send_data_euc_hiragana_nasugamama)
                .unwrap();
            let mut buffer = Vec::new();
            match buffer_stream.read_until(b'\n', &mut buffer) {
                Ok(size) => {
                    const MIMINUM_RESULT: &[u8] = b"1/c/\n";
                    assert!(size >= MIMINUM_RESULT.len());
                    assert!(!is_consecutive_slashes(&buffer));
                    assert!(buffer[0] == b'1' || buffer[0] == b'4');
                    if buffer[0] == b'1' {
                        assert!(buffer[1] == b'/');
                        assert!(buffer[buffer.len() - 2] == b'/');
                        println!(
                            "size={}  buffer={:?}",
                            size,
                            String::from_utf8(buffer.to_vec())
                        );
                    } else {
                        println!("size={}  buffer={:x?}", size, buffer);
                    }
                    assert!(buffer[buffer.len() - 1] == b'\n');
                }
                Err(e) => panic!("error e={}", e),
            }
            buffer_stream.get_mut().write_all_flush(b"0").unwrap();
        }
        Err(e) => panic!("error e={}", e),
    }
    thread_handle.join().unwrap();
    setup::exit();
}

#[test]
fn yaskkserv2_abbrev_euc_test() {
    let name = "yaskkserv2_abbrev_euc";
    setup::setup_and_wait(name);
    let port = "12505";
    test_abbrev(port, Encoding::Euc);
    setup::exit();
}

#[test]
fn yaskkserv2_abbrev_utf8_test() {
    let name = "yaskkserv2_abbrev_utf8";
    setup::setup_and_wait(name);
    let port = "12506";
    test_abbrev(port, Encoding::Utf8);
    setup::exit();
}

// candidates が dictionary に存在せず google API のみ返される場合の test
//
// 戻り値の candidates の形式がおかしくなければ成功。相手が google API なので、想定する値が
// 返ってこない場合があることに注意。
#[test]
fn yaskkserv2_dictionary_notfound_google_found_test() {
    let name = "yaskkserv2_dictionary_notfound_google_found";
    let is_google_suggest_enabled = false;
    test_dictionary_notfound_google_found(name, "12600", is_google_suggest_enabled);
}

#[test]
fn yaskkserv2_dictionary_notfound_google_found_enable_google_suggest_test() {
    let name = "yaskkserv2_dictionary_notfound_google_found_enable_google_suggest";
    let is_google_suggest_enabled = true;
    test_dictionary_notfound_google_found(name, "12601", is_google_suggest_enabled);
}

struct MaxConnections {}

impl MaxConnections {
    fn spawn(
        config: &Config,
        tx: &mpsc::Sender<&'static str>,
        send_data: &'static [u8],
        sleep_millis: u64,
    ) -> std::thread::JoinHandle<()> {
        let thread_config = config.clone();
        let tx = tx.clone();
        std::thread::Builder::new()
            .name(String::from(std::thread::current().name().unwrap()))
            .spawn(move || {
                // max_connections を越えて send するので error を握り潰す必要があることに注意
                match TcpStream::connect(format!("localhost:{}", thread_config.port)) {
                    Ok(mut stream) => {
                        stream.write_all_flush_ignore_error(send_data);
                        let mut buffer = vec![0; 8 * 1024];
                        match stream.read(&mut buffer) {
                            Ok(size) => {
                                if size != 0 {
                                    tx.send("send").unwrap();
                                }
                            }
                            Err(_) => {}
                        }
                        std::thread::sleep(std::time::Duration::from_millis(sleep_millis));
                        let _ignore_error = stream.write_all(b"0");
                        let _ignore_error = stream.flush();
                    }
                    Err(_) => {}
                }
            })
            .unwrap()
    }

    #[allow(dead_code)]
    fn run(port: &str, max_connections: usize) {
        let lifetime_static_table = [
            b"10 ", b"11 ", b"12 ", b"13 ", b"14 ", b"15 ", b"16 ", b"17 ", b"18 ", b"19 ", b"1a ",
            b"1b ", b"1c ", b"1d ", b"1e ", b"1f ", b"1g ", b"1h ", b"1i ", b"1j ", b"1k ", b"1l ",
            b"1m ", b"1n ", b"1o ", b"1p ", b"1q ", b"1r ", b"1s ", b"1t ", b"1u ", b"1v ", b"1w ",
            b"1x ", b"1y ", b"1z ", b"1A ", b"1B ", b"1C ", b"1D ", b"1E ", b"1F ", b"1G ", b"1H ",
            b"1I ", b"1J ", b"1K ", b"1L ", b"1M ", b"1N ", b"1O ", b"1P ", b"1Q ", b"1R ", b"1S ",
            b"1T ", b"1U ", b"1V ", b"1W ", b"1X ", b"1Y ", b"1Z ", b"1! ", b"1@ ", b"10 ", b"11 ",
            b"12 ", b"13 ", b"14 ", b"15 ", b"16 ", b"17 ", b"18 ", b"19 ", b"1a ", b"1b ", b"1c ",
            b"1d ", b"1e ", b"1f ", b"1g ", b"1h ", b"1i ", b"1j ", b"1k ", b"1l ", b"1m ", b"1n ",
            b"1o ", b"1p ", b"1q ", b"1r ", b"1s ", b"1t ", b"1u ", b"1v ", b"1w ", b"1x ", b"1y ",
            b"1z ", b"1A ", b"1B ", b"1C ", b"1D ", b"1E ", b"1F ", b"1G ", b"1H ", b"1I ", b"1J ",
            b"1K ", b"1L ", b"1M ", b"1N ", b"1O ", b"1P ", b"1Q ", b"1R ", b"1S ", b"1T ", b"1U ",
            b"1V ", b"1W ", b"1X ", b"1Y ", b"1Z ", b"1! ", b"1@ ",
        ];
        assert!(max_connections <= lifetime_static_table.len());
        let config = Config::new()
            .port(String::from(port))
            .dictionary_full_path(Path::get_full_path_yaskkserv2_dictionary(Encoding::Utf8))
            .max_connections(max_connections as i32);
        // ここで take_count を与えているが、それ以上の send() をするため、
        // spawn() 内で send() などが失敗することに注意。
        let thread_handle = run_and_wait_simple_server(&config, get_take_count(max_connections));
        {
            const THREAD_SLEEP_MILLIS: u64 = 10 * 1000;
            const TRY_RECV_SLEEP_MILLIS: u128 = 5 * 1000;
            let (tx, rx) = mpsc::channel();
            let mut thread_handles = Vec::new();
            for u in lifetime_static_table.iter() {
                thread_handles.push(Self::spawn(&config, &tx, *u, THREAD_SLEEP_MILLIS));
            }
            let start_time = std::time::Instant::now();
            let mut receive_count = 0;
            while match rx.try_recv() {
                Ok(_data) => {
                    receive_count += 1;
                    true
                }
                Err(e) => match e {
                    mpsc::TryRecvError::Empty => {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        start_time.elapsed().as_millis() < TRY_RECV_SLEEP_MILLIS
                    }
                    mpsc::TryRecvError::Disconnected => {
                        println!("Disconnected");
                        true
                    }
                },
            } {}
            assert_eq!(receive_count, config.max_connections);
            for handle in thread_handles {
                let _ignore_error_and_continue = handle.join();
            }
        }
        thread_handle.join().unwrap();
    }
}

#[test]
fn yaskkserv2_max_connections_1_test() {
    let name = "yaskkserv2_max_connections_1";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10400", 1);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_2_test() {
    let name = "yaskkserv2_max_connections_2";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10401", 2);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_4_test() {
    let name = "yaskkserv2_max_connections_4";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10402", 4);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_8_test() {
    let name = "yaskkserv2_max_connections_8";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10403", 8);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_16_test() {
    let name = "yaskkserv2_max_connections_16";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10404", 16);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_24_test() {
    let name = "yaskkserv2_max_connections_24";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10405", 24);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_32_test() {
    let name = "yaskkserv2_max_connections_32";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10406", 32);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_48_test() {
    let name = "yaskkserv2_max_connections_48";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10407", 48);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_64_test() {
    let name = "yaskkserv2_max_connections_64";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10408", 64);
    }
    setup::exit();
}

#[test]
fn yaskkserv2_max_connections_128_test() {
    let name = "yaskkserv2_max_connections_128";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        MaxConnections::run("10409", 128);
    }
    setup::exit();
}

struct TestConnections {}

// 大量に connection して正常終了するかの test
impl TestConnections {
    fn spawn(config: &Config) -> std::thread::JoinHandle<()> {
        let thread_config = config.clone();
        std::thread::Builder::new()
            .name(String::from(std::thread::current().name().unwrap()))
            .spawn(
                move || match TcpStream::connect(format!("localhost:{}", thread_config.port)) {
                    Ok(mut stream) => {
                        let mut buffer = vec![0; 8 * 1024];
                        let loop_times = rand::thread_rng().gen_range(1000, 5000);
                        for _ in 0..loop_times {
                            stream.write_all_flush(b"1a ").unwrap();
                            std::thread::sleep(std::time::Duration::from_millis(
                                rand::thread_rng().gen_range(1, 10),
                            ));
                            match stream.read(&mut buffer) {
                                Ok(_) => {}
                                Err(_) => {}
                            }
                        }
                        stream.write_disconnect_flush().unwrap();
                    }
                    Err(_) => {}
                },
            )
            .unwrap()
    }

    #[allow(dead_code)]
    fn run(port: &str, max_connections: usize) {
        // 大量の thread を起動するので、他の test 開始を少し待ち、他の test が落ち着いてから開始
        std::thread::sleep(std::time::Duration::from_millis(3 * 1000));
        loop {
            const ACTIVE_RUNNING_COUNT_LIMIT: usize = 1;
            if setup::get_test_running_count() <= ACTIVE_RUNNING_COUNT_LIMIT {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
        let config = Config::new()
            .port(String::from(port))
            .dictionary_full_path(Path::get_full_path_yaskkserv2_dictionary(Encoding::Utf8))
            .max_connections(max_connections as i32);
        let thread_handle = run_and_wait_simple_server(&config, get_take_count(max_connections));
        let mut thread_handles = Vec::new();
        loop {
            if thread_handles.len() < max_connections as usize {
                thread_handles.push(Self::spawn(&config));
            } else {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(
                rand::thread_rng().gen_range(1, 50),
            ));
        }
        for handle in thread_handles {
            let _ignore_error_and_continue = handle.join();
        }
        thread_handle.join().unwrap();
    }
}

#[test]
fn yaskkserv2_test_connections_test() {
    let name = "yaskkserv2_test_connections_test";
    setup::setup_and_wait(name);
    if std::env::var("YASKKSERV2_TEST_HEAVY").is_ok() {
        let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
        TestConnections::run("10500", 512);
    }
    setup::exit();
}
