use crate::skk::test_unix::{
    encoding_simple, get_take_count, setup, wait_server, ConnectSendCompare,
    ConnectSendCompareRunParameter, Encoding, GoogleTiming, Path, Protocol,
    Yaskkserv2MakeDictionary, MANY_THREADS,
};
use crate::skk::yaskkserv2::Yaskkserv2;
use crate::skk::Config;

#[cfg(test)]
use crate::skk::yaskkserv2::test_unix::Yaskkserv2Debug;

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct Yaskkserv2Test {
    name: String,
    config: Config,
    threads: usize,
    is_random_lf_or_crlf: bool,
    is_sequential: bool,
    is_send_broken_binary: bool,
    is_simple_std_use_tcp_single_thread_test: bool,
}

impl Yaskkserv2Test {
    fn new(name: &str, config: &Config) -> Self {
        Self {
            name: String::from(name),
            config: config.clone(),
            threads: 1,
            ..Self::default()
        }
    }

    fn run(&mut self, protocol: Protocol) {
        self.config.dictionary_full_path =
            Path::get_full_path_yaskkserv2_dictionary(self.config.encoding);
        self.config.is_no_daemonize = true;
        self.config.google_timing = GoogleTiming::Disable;
        let thread_config = self.config.clone();
        let thread_take_count = get_take_count(self.threads);
        let thread_is_simple_std_use_tcp_single_thread_test =
            self.is_simple_std_use_tcp_single_thread_test;
        let thread_is_force_exit_mode = self.is_send_broken_binary;
        let thread_handle = std::thread::Builder::new()
            .name(String::from(std::thread::current().name().unwrap()))
            .spawn(move || {
                let mut core = Yaskkserv2::new();
                core.setup(&thread_config).unwrap();
                core.is_debug_force_exit_mode = thread_is_force_exit_mode;
                if thread_is_simple_std_use_tcp_single_thread_test {
                    core.run_test_simple_std_net_tcp(thread_take_count);
                } else {
                    core.run_test(thread_take_count);
                }
            })
            .unwrap();
        wait_server(&self.config.port);
        let parameter = ConnectSendCompareRunParameter::new(
            &Path::get_full_path_yaskkserv2_jisyo(self.config.encoding),
            &self.name,
            &self.config.port,
            protocol,
        )
        .encoding(self.config.encoding)
        .is_random_lf_or_crlf(self.is_random_lf_or_crlf)
        .is_sequential(self.is_sequential)
        .is_send_broken_binary(self.is_send_broken_binary)
        .threads(self.threads);
        ConnectSendCompare::run(parameter);
        thread_handle.join().unwrap();
    }

    crate::define_builder!(is_random_lf_or_crlf, bool);
    crate::define_builder!(is_sequential, bool);
    crate::define_builder!(is_send_broken_binary, bool);
    crate::define_builder!(threads, usize);
    crate::define_builder!(is_simple_std_use_tcp_single_thread_test, bool);
}

struct Yaskkserv2MakeDictionaryTest;

impl Yaskkserv2MakeDictionaryTest {
    const LOOP: usize = 10;
    fn create_dictionary(name: &str, jisyo_encoding: Encoding, output_encoding: Encoding) {
        let encoding_table = encoding_simple::EncodingTable::get();
        let jisyo_full_path = Path::get_full_path_yaskkserv2_jisyo(jisyo_encoding);
        let dictionary_full_path =
            &Path::get_full_path(&format!("{}.{}.dictionary", &jisyo_full_path, name));
        let config = Config::new()
            .dictionary_full_path(String::from(dictionary_full_path))
            .encoding(output_encoding);
        let bench = std::time::Instant::now();
        for _ in 0..Self::LOOP {
            Yaskkserv2MakeDictionary::run_create_dictionary(
                &config.clone(),
                encoding_table,
                &[jisyo_full_path.clone()],
            )
            .unwrap();
        }
        let millis = bench.elapsed().as_millis();
        println!("{}  total  ms.={}", name, millis);
    }

    fn create_jisyo(name: &str, jisyo_encoding: Encoding, output_encoding: Encoding) {
        let dictionary_full_path = Path::get_full_path_yaskkserv2_dictionary(jisyo_encoding);
        let jisyo_full_path =
            &Path::get_full_path(&format!("{}.{}.jisyo", &dictionary_full_path, name));
        let config = Config::new()
            .dictionary_full_path(dictionary_full_path)
            .encoding(output_encoding);
        let bench = std::time::Instant::now();
        for _ in 0..Self::LOOP {
            Yaskkserv2MakeDictionary::run_create_jisyo(&config.clone(), jisyo_full_path).unwrap();
        }
        let millis = bench.elapsed().as_millis();
        println!("{}  total  ms.={}", name, millis);
    }
}

#[test]
fn yaskkserv2_benchmark_000_normal_send_sequential_euc_test() {
    let name = "yaskkserv2_benchmark_000_normal_send_sequential_euc";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12000"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_001_normal_send_random_euc_test() {
    let name = "yaskkserv2_benchmark_001_normal_send_random_euc";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12001"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config).run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_002_abbrev_sequential_euc_test() {
    let name = "yaskkserv2_benchmark_002_abbrev_sequential_euc";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12002"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .run(Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_003_abbrev_random_euc_test() {
    let name = "yaskkserv2_benchmark_003_abbrev_random_euc";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12003"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config).run(Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_010_normal_send_sequential_utf8_test() {
    let name = "yaskkserv2_benchmark_010_normal_send_sequential_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12010"));
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_011_normal_send_random_utf8_test() {
    let name = "yaskkserv2_benchmark_011_normal_send_random_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12011"));
    Yaskkserv2Test::new(name, &config).run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_012_abbrev_sequential_utf8_test() {
    let name = "yaskkserv2_benchmark_012_abbrev_sequential_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12012"));
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .run(Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_013_abbrev_random_utf8_test() {
    let name = "yaskkserv2_benchmark_013_abbrev_random_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12013"));
    Yaskkserv2Test::new(name, &config).run(Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_100_normal_send_sequential_multithreads_euc_test() {
    let name = "yaskkserv2_benchmark_100_normal_send_sequential_multithreads_euc";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12100"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_101_normal_send_random_multithreads_euc_test() {
    let name = "yaskkserv2_benchmark_101_normal_send_random_multithreads_euc";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12101"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_102_abbrev_sequential_euc_multithreads_test() {
    let name = "yaskkserv2_benchmark_102_abbrev_sequential_euc_multithreads";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12102"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_103_abbrev_random_euc_multithreads_test() {
    let name = "yaskkserv2_benchmark_103_abbrev_random_euc_multithreads";
    setup::setup_and_wait(name);
    let config = Config::new()
        .port(String::from("12103"))
        .encoding(Encoding::Euc);
    Yaskkserv2Test::new(name, &config)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_110_normal_send_sequential_multithreads_utf8_test() {
    let name = "yaskkserv2_benchmark_110_normal_send_sequential_multithreads_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12110"));
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_111_normal_send_random_multithreads_utf8_test() {
    let name = "yaskkserv2_benchmark_111_normal_send_random_multithreads_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12111"));
    Yaskkserv2Test::new(name, &config)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_112_abbrev_sequential_multithreads_utf8_test() {
    let name = "yaskkserv2_benchmark_112_abbrev_sequential_multithreads_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12112"));
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol4);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_113_abbrev_random_multithreads_utf8_test() {
    let name = "yaskkserv2_benchmark_113_abbrev_random_multithreads_utf8";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12113"));
    Yaskkserv2Test::new(name, &config)
        .threads(MANY_THREADS)
        .run(Protocol::Protocol4);
    setup::exit();
}

//
// debug_send は意図的に送信を半端な状態にしたり、過剰に送信するもの。
//
#[test]
fn yaskkserv2_benchmark_debug_send_test() {
    let name = "yaskkserv2_benchmark_debug_send";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12500"));
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_normal_send_random_lf_or_crlf_test() {
    let name = "yaskkserv2_benchmark_normal_send_random_lf_or_crlf";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12501"));
    Yaskkserv2Test::new(name, &config)
        .is_random_lf_or_crlf(true)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_normal_send_broken_binary_test() {
    let name = "yaskkserv2_benchmark_normal_send_broken_binary_test";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12502"));
    Yaskkserv2Test::new(name, &config)
        .is_send_broken_binary(true)
        .run(Protocol::Protocol1);
    setup::exit();
}

//
// FIXME! 部の kill だけでは処理できないケースが多々あることに注意。
//
#[test]
fn yaskkserv2_benchmark_binary_normal_send_sequential_test() {
    let name = "yaskkserv2_benchmark_binary_normal_send_sequential";
    setup::setup_and_wait(name);
    let port = "12503";
    let child = match std::process::Command::new("target/release/yaskkserv2")
        .arg("--no-daemonize")
        .arg(format!("--port={}", port))
        .arg(&Path::get_full_path_yaskkserv2_dictionary(Encoding::Utf8))
        .spawn()
    {
        Ok(ok) => ok,
        Err(e) => {
            println!("Error(test success): yaskkserv2  error={:?}", e);
            return;
        }
    };
    wait_server(port);
    let parameter = ConnectSendCompareRunParameter::new(
        &Path::get_full_path_yaskkserv2_jisyo(Encoding::Utf8),
        name,
        port,
        Protocol::Protocol1,
    )
    .is_compare(false)
    .encoding(Encoding::Utf8)
    .is_sequential(true);
    ConnectSendCompare::run(parameter);
    // FIXME!
    let _droppable = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(format!("{}", child.id()))
        .spawn()
        .unwrap();
    setup::exit();
}

#[test]
fn yaskkserv2_benchmark_send_std_net_tcp_test() {
    let name = "yaskkserv2_benchmark_send_std_net_tcp";
    setup::setup_and_wait(name);
    let config = Config::new().port(String::from("12504"));
    Yaskkserv2Test::new(name, &config)
        .is_sequential(true)
        .is_simple_std_use_tcp_single_thread_test(true)
        .run(Protocol::Protocol1);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_dictionary_euc_euc_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_dictionary_euc_euc";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_dictionary(name, Encoding::Euc, Encoding::Euc);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_dictionary_euc_utf8_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_dictionary_euc_utf8";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_dictionary(name, Encoding::Euc, Encoding::Utf8);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_dictionary_utf8_euc_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_dictionary_utf8_euc";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_dictionary(name, Encoding::Utf8, Encoding::Euc);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_dictionary_utf8_utf8_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_dictionary_utf8_utf8";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_dictionary(name, Encoding::Utf8, Encoding::Utf8);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_jisyo_euc_euc_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_jisyo_euc_euc";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_jisyo(name, Encoding::Euc, Encoding::Euc);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_jisyo_euc_utf8_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_jisyo_euc_utf8";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_jisyo(name, Encoding::Euc, Encoding::Utf8);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_jisyo_utf8_euc_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_jisyo_utf8_euc";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_jisyo(name, Encoding::Utf8, Encoding::Euc);
    setup::exit();
}

#[test]
fn yaskkserv2_make_dictionary_benchmark_create_jisyo_utf8_utf8_test() {
    let name = "yaskkserv2_make_dictionary_benchmark_create_jisyo_utf8_utf8";
    setup::setup_and_wait(name);
    Yaskkserv2MakeDictionaryTest::create_jisyo(name, Encoding::Utf8, Encoding::Utf8);
    setup::exit();
}
