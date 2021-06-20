mod ddskk;
mod dictionary;
mod echo_server;
mod encoding;
mod jisyo;
mod miscellaneous;
pub(in crate::skk) mod setup;
mod yaskkserv;
mod yaskkserv2;
mod yaskkserv2_benchmark;

use rand::seq::SliceRandom;
use rand::Rng;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, RwLock};

use crate::skk::yaskkserv2::TcpStreamSkk;
use crate::skk::yaskkserv2_make_dictionary::JisyoReader;
use crate::skk::{
    encoding_simple, Candidates, Dictionary, DictionaryMidashiKey, Encoding, GoogleTiming,
    Yaskkserv2MakeDictionary, DEFAULT_MAX_SERVER_COMPLETIONS,
};

pub(in crate::skk) static INIT_MUTEX_LOCK: once_cell::sync::Lazy<Mutex<()>> =
    once_cell::sync::Lazy::new(|| Mutex::new(()));
pub(in crate::skk) static TEST_MUTEX_LOCK: once_cell::sync::Lazy<Mutex<()>> =
    once_cell::sync::Lazy::new(|| Mutex::new(()));
pub(in crate::skk) static MANY_THREAD_MUTEX_LOCK: once_cell::sync::Lazy<Mutex<()>> =
    once_cell::sync::Lazy::new(|| Mutex::new(()));

pub(in crate::skk) const DEBUG_FORCE_EXIT_DIRECTORY: &str = "DEBUG_FORCE_EXIT";
const MANY_THREADS: usize = 8;

const fn get_take_count(threads: usize) -> usize {
    const WAIT_SERVER: usize = 1;
    WAIT_SERVER + threads
}

pub(in crate::skk) fn read_jisyo_entries_no_encoding_conversion(
    jisyo_full_path: &str,
) -> Vec<Vec<u8>> {
    let map = JisyoReader::get_merged_jisyo_entries_map_no_encoding_conversion(&[String::from(
        jisyo_full_path,
    )])
    .unwrap();
    map.iter()
        .map(|kv| {
            let mut vec = Vec::new();
            vec.extend_from_slice(kv.0);
            vec.push(b' ');
            vec.extend_from_slice(kv.1);
            vec
        })
        .collect()
}

fn wait_server(port: &str) {
    while if TcpStream::connect(format!("localhost:{}", port)).is_ok() {
        println!("wait_server connect port={}", port);
        false
    } else {
        println!("wait_server wait port={}", port);
        std::thread::sleep(std::time::Duration::from_millis(500));
        true
    } {}
    // std::thread::sleep(std::time::Duration::from_millis(1000));
}

pub(in crate::skk) struct Path {}

impl Path {
    const TEST_DICTIONARY: &'static str = "yaskkserv2_test_unix.dictionary";
    const TEST_JISYO: &'static str = "yaskkserv2_test_unix.dictionary.jisyo";
    const ECHO_SERVER: &'static str = "echo_server";
    const ECHO_SERVER_SOURCE: &'static str = "echo_server.c";

    pub(in crate::skk) fn get_full_path(full_path: &str) -> String {
        if std::env::var("YASKKSERV2_TEST_DIRECTORY").is_err() {
            panic!("\n\n\"YASKKSERV2_TEST_DIRECTORY\" environment variable not set\n\n");
        }
        String::from(
            std::path::Path::new(&std::env::var("YASKKSERV2_TEST_DIRECTORY").unwrap())
                .join(full_path)
                .to_str()
                .unwrap(),
        )
    }

    pub(in crate::skk) fn get_full_path_yaskkserv2_jisyo(encoding: Encoding) -> String {
        format!(
            "{}{}",
            Self::get_full_path(Self::TEST_JISYO),
            &Self::get_encoding_suffix(encoding)
        )
    }

    fn get_full_path_test_base() -> String {
        Self::get_full_path("")
    }

    fn get_full_path_echo_server() -> String {
        Self::get_full_path(Self::ECHO_SERVER)
    }

    fn get_full_path_echo_server_source() -> String {
        Self::get_full_path(Self::ECHO_SERVER_SOURCE)
    }

    fn get_encoding_suffix(encoding: Encoding) -> String {
        format!(".{}", &encoding.to_string().to_lowercase())
    }

    fn get_full_path_yaskkserv2_dictionary(encoding: Encoding) -> String {
        format!(
            "{}{}",
            Self::get_full_path(Self::TEST_DICTIONARY),
            &Self::get_encoding_suffix(encoding)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Protocol {
    Protocol1,
    Protocol4,
    Echo,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Protocol1
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default)]
struct ConnectSendCompareRunParameter {
    jisyo_full_path: String,
    name: String,
    port: String,
    protocol: Protocol,
    encoding: Encoding,
    threads: usize,
    max_server_completions: usize,
    is_compare: bool,
    is_yaskkserv: bool,
    is_send_lf: bool,
    is_random_lf_or_crlf: bool,
    is_sequential: bool,
    is_send_broken_binary: bool,
}

impl ConnectSendCompareRunParameter {
    fn new(jisyo_full_path: &str, name: &str, port: &str, protocol: Protocol) -> Self {
        Self {
            jisyo_full_path: String::from(jisyo_full_path),
            name: String::from(name),
            port: String::from(port),
            protocol,
            encoding: Encoding::Utf8,
            threads: 1,
            max_server_completions: DEFAULT_MAX_SERVER_COMPLETIONS as usize,
            is_compare: true,
            ..Self::default()
        }
    }

    crate::define_builder!(encoding, Encoding);
    crate::define_builder!(is_yaskkserv, bool);
    crate::define_builder!(is_compare, bool);
    crate::define_builder!(is_send_lf, bool);
    crate::define_builder!(is_random_lf_or_crlf, bool);
    crate::define_builder!(is_sequential, bool);
    crate::define_builder!(is_send_broken_binary, bool);
    crate::define_builder!(threads, usize);
    crate::define_builder!(max_server_completions, usize);
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct ConnectSendCompare {
    name: String,
    port: String,
    protocol: Protocol,
    encoding: Encoding,
    max_server_completions: usize,
    bench: Option<std::time::Instant>,
    bench_total: Option<std::time::Instant>,
    bench_recv_count: i32,
    bench_total_recv_count: i32,
    rng: rand::rngs::ThreadRng,
    is_report_rps: bool,
    is_compare: bool,
    is_yaskkserv: bool,
    is_send_lf: bool,
    is_random_lf_or_crlf: bool,
    is_send_broken_binary: bool,
}

struct GetMidashiSendCandidatesResult {
    cannot_euc_conversion_skip_count: usize,
    midashi_send_candidates: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)>,
}

impl ConnectSendCompare {
    fn new(name: &str, port: &str, protocol: Protocol) -> Self {
        Self {
            name: String::from(name),
            port: String::from(port),
            protocol,
            encoding: Encoding::Utf8,
            max_server_completions: DEFAULT_MAX_SERVER_COMPLETIONS as usize,
            rng: rand::thread_rng(),
            is_report_rps: true,
            is_compare: true,
            ..Self::default()
        }
    }

    fn receive_line_common(&mut self) {
        self.bench_recv_count += 1;
        self.bench_total_recv_count += 1;
        let bench_millis = self.bench.unwrap().elapsed().as_millis();
        let limit_millis = 2000;
        if bench_millis >= limit_millis {
            #[allow(clippy::cast_possible_truncation)]
            if self.is_report_rps {
                println!(
                    "{} : port={}  rps.={}",
                    self.name,
                    self.port,
                    self.bench_recv_count * 1000 / bench_millis as i32,
                );
            }
            self.bench_recv_count = 0;
            self.bench = Some(std::time::Instant::now());
        }
    }

    fn receive_line_protocol_1(&mut self, buffer: &[u8], midashi: &[u8], candidates: &[u8]) {
        let result = buffer.to_vec();
        let mut candidates_compare: Vec<u8> = Vec::new();
        candidates_compare.extend_from_slice(b"1");
        candidates_compare.extend_from_slice(candidates);
        candidates_compare.extend_from_slice(b"\n");
        let candidates_compare = candidates_compare;
        if self.is_compare && result != candidates_compare {
            let decoded_midashi = encoding_simple::Euc::decode(midashi).unwrap();
            if self.encoding == Encoding::Euc {
                println!(
                    r#"euc candidates compare error  midashi="{:?}"(0x{:x?})  jisyo="{:x?}"  result="{:x?}""#,
                    String::from_utf8(decoded_midashi.clone()),
                    decoded_midashi,
                    candidates_compare,
                    result
                );
                let decoded_jisyo = encoding_simple::Euc::decode(&candidates_compare).unwrap();
                let decoded_result = encoding_simple::Euc::decode(&result).unwrap();
                println!(
                    r#"jisyo="{:?}"  result="{:?}""#,
                    String::from_utf8(decoded_jisyo),
                    String::from_utf8(decoded_result)
                );
            } else {
                println!(
                    r#"utf8 candidates compare error  midashi="{:?}"(0x{:x?})  jisyo="{:x?}"  result="{:x?}""#,
                    String::from_utf8(decoded_midashi),
                    midashi,
                    candidates_compare,
                    result
                );
                println!(
                    r#"jisyo="{:?}"  result="{:?}""#,
                    String::from_utf8(candidates_compare),
                    String::from_utf8(result),
                );
            }
            panic!("compare error port={}", self.port);
        }
        self.receive_line_common();
    }

    fn receive_line_protocol_4(&mut self, buffer: &[u8], midashi: &[u8], size: usize) {
        const SKIP_HEAD_1: usize = 1;
        const SLASH_AND_LF_LENGTH: usize = 1 + 1;
        if buffer[0] == b'4' {
            return;
        }
        let min_length = b"1/a/\n".len();
        assert!(size >= min_length);
        assert!(buffer[0] == b'1');
        assert!(buffer[1] == b'/');
        assert!(buffer[size - 2] == b'/');
        assert!(buffer[size - 1] == b'\n');
        let candidates = buffer[..size - SLASH_AND_LF_LENGTH]
            .split(|v| *v == b'/')
            .skip(SKIP_HEAD_1)
            .collect::<Vec<&[u8]>>();
        assert!(!candidates.is_empty());
        assert!(self.is_yaskkserv || (candidates.len() <= self.max_server_completions));
        // abbrev は quote がかかるので、quote されてなさそうなもののみ
        // 比較することに注意。また、 server によっては / を含むものを
        // 返してしまう場合があることに注意。
        // (yaskkserv2 は返さないが yaskkserv などは / を含むものを返す)
        if self.is_compare
            && candidates[0].len() == midashi.len()
            && candidates[0][..midashi.len()] == *midashi
        {
            for u in candidates {
                if midashi.len() > u.len() {
                    println!(
                        "midashi={:?}  buffer={:?}",
                        String::from_utf8(midashi.to_vec()).unwrap(),
                        String::from_utf8(buffer.to_vec()).unwrap()
                    );
                    panic!("midashi={:x?}  u={:x?}", midashi, u);
                }
                assert_eq!(*midashi, u[..midashi.len()]);
            }
        }
        self.receive_line_common();
    }

    fn receive_line_protocol_echo(&mut self, buffer: &[u8], send: &[u8]) {
        assert_eq!(buffer, send);
        self.receive_line_common();
    }

    fn report_total(&self) {
        let bench_total_millis = self.bench_total.unwrap().elapsed().as_millis();
        #[allow(clippy::cast_possible_truncation)]
        if bench_total_millis == 0 {
            println!("bench error");
        } else if self.is_report_rps {
            println!(
                "{} : port={}  total  {}times  {}ms.  rps.={}",
                self.name,
                self.port,
                self.bench_total_recv_count,
                bench_total_millis,
                self.bench_total_recv_count * 1000 / bench_total_millis as i32,
            );
        }
    }

    fn get_midashi_send_candidates(
        &mut self,
        jisyo_full_path: &str,
        is_sequential: bool,
    ) -> GetMidashiSendCandidatesResult {
        let mut cannot_euc_conversion_skip_count = 0;
        let mut midashi_send_candidates = Vec::new();
        let mut jisyo_entries = if self.encoding == Encoding::Euc {
            read_jisyo_entries_no_encoding_conversion(jisyo_full_path)
                .iter()
                .filter(|v| {
                    let r = Self::cannot_euc_conversion(v);
                    if r {
                        cannot_euc_conversion_skip_count += 1;
                    }
                    !r
                })
                .cloned()
                .collect::<Vec<Vec<u8>>>()
        } else {
            read_jisyo_entries_no_encoding_conversion(jisyo_full_path)
        };
        if !is_sequential {
            jisyo_entries.shuffle(&mut rand::thread_rng());
        }
        let jisyo_entries = jisyo_entries;
        for entry in &jisyo_entries {
            if let Some(midashi_space_find) = twoway::find_bytes(entry, b" ") {
                let mut send: Vec<u8> = Vec::new();
                match self.protocol {
                    Protocol::Protocol1 => send.push(b'1'),
                    Protocol::Protocol4 => send.push(b'4'),
                    Protocol::Echo => {}
                }
                let midashi = if self.encoding == Encoding::Utf8 {
                    encoding_simple::Euc::encode(&entry[0..midashi_space_find]).unwrap()
                } else {
                    entry[0..midashi_space_find].to_vec()
                };
                send.extend_from_slice(&midashi);
                if self.is_random_lf_or_crlf {
                    if self.rng.gen_bool(0.5) {
                        send.extend_from_slice(b" \r\n");
                    } else {
                        send.extend_from_slice(b" \n");
                    }
                } else if self.is_send_lf {
                    send.extend_from_slice(b" \n");
                } else {
                    send.extend_from_slice(b" ");
                }
                let candidates = entry[midashi_space_find + 1..].to_vec();
                midashi_send_candidates.push((midashi, send, candidates));
            }
        }
        GetMidashiSendCandidatesResult {
            cannot_euc_conversion_skip_count,
            midashi_send_candidates,
        }
    }

    /// server を強制終了してその終了を待つ
    ///
    /// Yaskkserv2 の `is_debug_force_exit_mode` で directory を強制終了 flag として使っている
    /// ことに注意。
    fn force_exit_server_and_wait_exit_server_for_connect_send_broken_binary(
        buffer_stream: &mut BufReader<&TcpStream>,
    ) {
        const WAIT_EXIT_LOOP: usize = 100;
        const WAIT_EXIT_SLEEP_MILLIS: u64 = 100;
        let debug_force_exit_directory_full_path =
            std::path::Path::new(&std::env::var("YASKKSERV2_TEST_DIRECTORY").unwrap())
                .join(DEBUG_FORCE_EXIT_DIRECTORY);
        std::fs::create_dir_all(&debug_force_exit_directory_full_path).unwrap();
        // connect_send_broken_binary() で random data を送信していて send() と read() の対応が
        // 取れていないため、ここで b'0' を dummy send/read して、溜まった buffer を吐き出して
        // いることに注意。
        // 同時に、 debug_force_exit_directory_full_path が存在する間 dummy send/read を続けて
        // server の read_until_skk_server() を動作させ、その直後の remove_dir() を促している。
        for _ in 0..WAIT_EXIT_LOOP {
            buffer_stream.get_mut().write_disconnect_flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(WAIT_EXIT_SLEEP_MILLIS));
            if !debug_force_exit_directory_full_path.exists() {
                break;
            }
            let mut dummy_read_buffer = Vec::new();
            if let Ok(read_length) = buffer_stream.read(&mut dummy_read_buffer) {
                if read_length == 0 {
                    break;
                }
            } else {
                println!("read() error");
            }
        }
    }

    fn connect_send_broken_binary(&self) {
        match TcpStream::connect(format!("localhost:{}", self.port)) {
            Ok(stream) => {
                const READ_TIMEOUT_SECS: u64 = 1;
                // TEST_LOOP を大きな値にすると Resource temporarily unavailable が発生し、
                // stream が使えなくなるので控え目な値にしていることに注意。
                // 本来はテストサーバ側に timeout など、適切な対策を入れる必要がある。
                const TEST_LOOP: usize = 64;
                println!("connected!  {}  port={}", self.name, self.port);
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(READ_TIMEOUT_SECS)))
                    .unwrap();
                let mut buffer_stream = BufReader::new(&stream);
                for _ in 0..TEST_LOOP {
                    const RANDOM_LENGTH_MAX: usize = 1000;
                    let random_length = rand::thread_rng().gen_range(1, RANDOM_LENGTH_MAX + 1);
                    let mut random_binary_data = Vec::new();
                    for _ in 0..random_length {
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        random_binary_data.push(rand::thread_rng().gen_range(0x00, 0xff + 1) as u8);
                    }
                    random_binary_data = random_binary_data
                        .into_iter()
                        .map(|v| if v == b'0' { 0 } else { v })
                        .collect::<Vec<u8>>();
                    random_binary_data.push(b' ');
                    buffer_stream
                        .get_mut()
                        .write_all_flush(&random_binary_data)
                        .unwrap();
                    let mut buffer = Vec::new();
                    match buffer_stream.read_until(b'\n', &mut buffer) {
                        Ok(0) => return,
                        Ok(_size) => {}
                        Err(_e) => std::thread::sleep(std::time::Duration::from_millis(500)),
                    }
                }
                Self::force_exit_server_and_wait_exit_server_for_connect_send_broken_binary(
                    &mut buffer_stream,
                );
            }
            Err(e) => {
                println!("bin {:#?}", e);
            }
        }
    }

    fn connect(
        &mut self,
        jisyo_full_path: &str,
        thread_end_counter: &mut Arc<RwLock<usize>>,
        is_sequential: bool,
        threads: usize,
    ) {
        if self.is_send_broken_binary {
            self.connect_send_broken_binary();
            return;
        }
        let get_midashi_send_candidates_result =
            self.get_midashi_send_candidates(jisyo_full_path, is_sequential);
        match TcpStream::connect(format!("localhost:{}", self.port)) {
            Ok(stream) => {
                println!("connected!  {}  port={}", self.name, self.port);
                let mut buffer_stream = BufReader::new(&stream);
                self.bench = Some(std::time::Instant::now());
                self.bench_total = Some(std::time::Instant::now());
                for (midashi, send, candidates) in
                    get_midashi_send_candidates_result.midashi_send_candidates
                {
                    buffer_stream.get_mut().write_all_flush(&send).unwrap();
                    let mut buffer = Vec::new();
                    match buffer_stream.read_until(b'\n', &mut buffer) {
                        Ok(0) => return,
                        Ok(size) => match self.protocol {
                            Protocol::Protocol1 => {
                                self.receive_line_protocol_1(&buffer, &midashi, &candidates)
                            }
                            Protocol::Protocol4 => {
                                self.receive_line_protocol_4(&buffer, &midashi, size)
                            }
                            Protocol::Echo => self.receive_line_protocol_echo(&buffer, &send),
                        },
                        Err(e) => panic!("{:#?}", e),
                    }
                }
                if get_midashi_send_candidates_result.cannot_euc_conversion_skip_count > 0 {
                    println!(
                        r#"cannot_euc_conversion: {} lines SKIPPED"#,
                        get_midashi_send_candidates_result.cannot_euc_conversion_skip_count
                    );
                }
                self.report_total();
                buffer_stream.get_mut().flush().unwrap();
                {
                    *thread_end_counter.write().unwrap() += 1;
                }
                if *thread_end_counter.read().unwrap() >= threads {
                    buffer_stream.get_mut().write_disconnect_flush().unwrap();
                }
            }
            Err(e) => panic!("{:#?}", e),
        }
    }

    fn copy_parameter(parameter: &ConnectSendCompareRunParameter, connect_send_compare: &mut Self) {
        connect_send_compare.encoding = parameter.encoding;
        connect_send_compare.is_yaskkserv = parameter.is_yaskkserv;
        connect_send_compare.is_compare = parameter.is_compare;
        connect_send_compare.is_send_lf = parameter.is_send_lf;
        connect_send_compare.is_random_lf_or_crlf = parameter.is_random_lf_or_crlf;
        connect_send_compare.is_send_broken_binary = parameter.is_send_broken_binary;
        connect_send_compare.max_server_completions = parameter.max_server_completions;
    }

    fn run(parameter: ConnectSendCompareRunParameter) {
        let mut thread_end_counter = Arc::new(RwLock::new(0));
        if parameter.threads == 1 {
            let mut connect_send_compare =
                Self::new(&parameter.name, &parameter.port, parameter.protocol);
            Self::copy_parameter(&parameter, &mut connect_send_compare);
            connect_send_compare.connect(
                &parameter.jisyo_full_path,
                &mut thread_end_counter,
                parameter.is_sequential,
                parameter.threads,
            );
        } else {
            let threads = parameter.threads;
            let thread_parameter = Arc::new(RwLock::new(parameter));
            let mut thread_handles = Vec::new();
            for thread_index in 0..threads {
                let thread_parameter = thread_parameter.clone();
                let mut thread_end_counter = thread_end_counter.clone();
                thread_handles.push(
                    std::thread::Builder::new()
                        .name(String::from(std::thread::current().name().unwrap()))
                        .spawn(move || {
                            let mut connect_send_compare = Self::new(
                                &format!(
                                    "{} tid={:>2}",
                                    &thread_parameter.read().unwrap().name,
                                    thread_index
                                ),
                                &thread_parameter.read().unwrap().port,
                                thread_parameter.read().unwrap().protocol,
                            );
                            Self::copy_parameter(
                                &thread_parameter.read().unwrap(),
                                &mut connect_send_compare,
                            );
                            connect_send_compare.connect(
                                &thread_parameter.read().unwrap().jisyo_full_path,
                                &mut thread_end_counter,
                                thread_parameter.read().unwrap().is_sequential,
                                thread_parameter.read().unwrap().threads,
                            );
                        })
                        .unwrap(),
                );
            }
            for handle in thread_handles {
                let _ignore_error_and_continue = handle.join();
            }
        }
    }

    const fn cannot_euc_conversion(line: &[u8]) -> bool {
        line.len() > 4
            && line[0] == b'&'
            && line[1] == b'#'
            && line[2].is_ascii_digit()
            && line[3].is_ascii_digit()
    }
}
