use rand::Rng;
use std::sync::{Arc, Mutex, RwLock};

use crate::skk::test_unix::{Path, MANY_THREAD_MUTEX_LOCK};
use crate::skk::yaskkserv2::{
    BufRead, DictionaryFile, File, GoogleCache, Server, ServerDebug, Shutdown, Yaskkserv2,
    INITIAL_DICTIONARY_FILE_READ_BUFFER_LENGTH,
};

pub(in crate::skk) trait Yaskkserv2Debug {
    fn run_test(&mut self, take_count_for_test: usize);
    fn run_test_simple_std_net_tcp(&mut self, take_count_for_test: usize);
}

impl Yaskkserv2Debug for Yaskkserv2 {
    fn run_test(&mut self, take_count_for_test: usize) {
        self.run_loop(take_count_for_test).unwrap();
    }

    fn run_test_simple_std_net_tcp(&mut self, take_count_for_test: usize) {
        let listener = match std::net::TcpListener::bind(format!(
            "{}:{}",
            &self.server.config.listen_address, &self.server.config.port
        )) {
            Ok(ok) => ok,
            Err(e) => {
                let message = format!("bind failed {e}");
                Self::log_error(&message);
                Self::print_warning(&message);
                return;
            }
        };
        for stream in listener.incoming().take(take_count_for_test) {
            match stream {
                Ok(stream) => {
                    let mut buffer_stream = std::io::BufReader::new(stream);
                    let mut dictionary_file = DictionaryFile::new(
                        File::open(&self.server.config.dictionary_full_path).unwrap(),
                        INITIAL_DICTIONARY_FILE_READ_BUFFER_LENGTH,
                    );
                    let mut buffer = Vec::new();
                    while if let Ok(size) = buffer_stream.read_until(b' ', &mut buffer) {
                        if size == 0 {
                            false
                        } else {
                            let mut loop_result = true;
                            let skip = Self::get_buffer_skip_count(&buffer, size);
                            if size - skip > 0 {
                                match buffer.get(skip) {
                                    Some(b'0') => loop_result = false,
                                    Some(b'1') => {
                                        self.server.handle_client_protocol_1_simple_std_net_tcp(
                                            buffer_stream.get_mut(),
                                            &mut dictionary_file,
                                            &mut buffer[skip..],
                                        );
                                    }
                                    _ => panic!("error"),
                                }
                            } else {
                                loop_result = false;
                            }
                            buffer.clear();
                            loop_result
                        }
                    } else {
                        match buffer_stream.get_ref().peer_addr() {
                            Ok(peer_addr) => {
                                Self::log_error(&format!("read_line() error={peer_addr}"));
                            }
                            Err(e) => {
                                Self::log_error(&format!("peer_address() get failed error={e}"));
                            }
                        }
                        if let Err(e) = buffer_stream.get_mut().shutdown(Shutdown::Both) {
                            Self::log_error(&format!("shutdown error={e}"));
                        }
                        false
                    } {}
                }
                Err(e) => {
                    Self::log_error(&format!("{e}"));
                }
            }
        }
        drop(listener);
    }
}

fn setup_google_cache() -> Arc<RwLock<Server>> {
    let core = Arc::new(RwLock::new(Server::new()));
    // cache は google_cache_entries を越えると expire されるので
    // test では大きめに取っておくことに注意
    core.write().unwrap().config.google_cache_entries = 10000 * 16 * 2;
    {
        core.write().unwrap().config.google_cache_full_path =
            Path::get_full_path("yaskkserv2.google_cache");
        GoogleCache::setup_use_rwlock_internally(
            &core.read().unwrap().config.google_cache_full_path,
        )
        .unwrap();
    }
    core
}

#[test]
fn yaskkserv2_google_cache_mutex_read_write_compare_test() {
    let name = "yaskkserv2_google_cache_mutex_read_write_compare";
    println!("wait lock {name}");
    let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
    println!("start {name}");
    let mutex = Arc::new(Mutex::new(()));
    let core = setup_google_cache();
    for _ in 0..1 {
        let mut join_handles = Vec::new();
        for _thread_index in 0..16 {
            let core = core.clone();
            let mutex = mutex.clone();
            join_handles.push(
                std::thread::Builder::new()
                    .name(String::from(std::thread::current().name().unwrap()))
                    .spawn(move || {
                        let mut rng = rand::thread_rng();
                        for midashi in 0..1000 {
                            let _lock = mutex.lock();
                            let candidates =
                                [
                                    format!(
                                        "candidates{}",
                                        midashi + rng.gen_range(0..999_999_999)
                                    )
                                    .as_bytes()
                                    .to_vec(),
                                ];
                            let config = &core.read().unwrap().config;
                            GoogleCache::write_candidates(
                                format!("{midashi}").as_bytes(),
                                &candidates,
                                &config.google_cache_full_path,
                                config.google_cache_entries,
                                config.google_cache_expire_seconds,
                            )
                            .unwrap();
                            let get_candidates =
                                GoogleCache::get_candidates(format!("{midashi}").as_bytes());
                            assert_eq!(get_candidates, candidates);
                        }
                    })
                    .unwrap(),
            );
        }
        for join in join_handles {
            join.join().unwrap();
        }
    }
}

#[test]
fn yaskkserv2_google_cache_multithread_read_write_test() {
    let name = "yaskkserv2_google_cache_multithread_read_write";
    println!("wait lock {name}");
    let _many_thread_lock = MANY_THREAD_MUTEX_LOCK.lock();
    println!("start {name}");
    let core = setup_google_cache();
    for _ in 0..1 {
        let mut join_handles = Vec::new();
        for _thread_index in 0..16 {
            let core = core.clone();
            join_handles.push(
                std::thread::Builder::new()
                    .name(String::from(std::thread::current().name().unwrap()))
                    .spawn(move || {
                        let mut rng = rand::thread_rng();
                        for midashi in 0..1000 {
                            let candidates =
                                [
                                    format!(
                                        "candidates{}",
                                        midashi + rng.gen_range(0..999_999_999)
                                    )
                                    .as_bytes()
                                    .to_vec(),
                                ];
                            let config = &core.read().unwrap().config;
                            GoogleCache::write_candidates(
                                format!("{midashi}").as_bytes(),
                                &candidates,
                                &config.google_cache_full_path,
                                config.google_cache_entries,
                                config.google_cache_expire_seconds,
                            )
                            .unwrap();
                            GoogleCache::get_candidates(format!("{midashi}").as_bytes());
                        }
                    })
                    .unwrap(),
            );
        }
        for join in join_handles {
            join.join().unwrap();
        }
    }
}
