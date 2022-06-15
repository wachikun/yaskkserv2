//! yaskkserv2
//!
//! # はじめに
//!
//! SKK server 本体。 `yaskkserv2_make_dictionary` で作成した dictionary を使用する。
//!
//! `yaskkserv2_make_dictionary` に比べるとメモリなどのリソースを抑えて使用する。ファイルは
//! 一気読みせず、メモリに保持するデータも限定的。実行速度も重要となるため、ヒープや map の
//! 使用も最低限に抑えてある (現代的な Rust が動作がするような環境に対して、いささか神経質に
//! なり過ぎかもしれない)。

mod dictionary_reader;
mod google_cache;
mod request;
mod server;

pub(in crate::skk) mod command_line;
pub(in crate::skk) mod config_file;

#[cfg(test)]
pub(in crate::skk) mod test_unix;

#[cfg(all(not(test), not(unix)))]
use log::*;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::net::Shutdown;
use std::sync::RwLock;
#[cfg(all(not(test), unix))]
use syslog::{Facility, Formatter3164};

#[cfg(not(test))]
use crate::skk::PKG_NAME;
use crate::skk::{
    encoding_simple, Candidates, Config, Dictionary, DictionaryBlockInformation,
    DictionaryMidashiKey, Encoding, GoogleTiming, OnMemory, SkkError, GOOGLE_JAPANESE_INPUT_URL,
    GOOGLE_SUGGEST_URL, PKG_VERSION, PROTOCOL_RESULT_ERROR, SHA1SUM_LENGTH,
};

#[cfg(feature = "assert_paranoia")]
use crate::{const_assert, const_panic};

#[cfg(test)]
use crate::skk::test_unix::DEBUG_FORCE_EXIT_DIRECTORY;

#[cfg(test)]
use crate::skk::yaskkserv2::server::test_unix::ServerDebug;

const MAX_CONNECTION: usize = 1024;

const PROTOCOL_MAXIMUM_LENGTH: usize = {
    const SKKSERV_MAXIMUM_MIDASHI_LENGTH: usize = 510;
    const PROTOCOL_MAXIMUM_LENGTH_UTF8_SCALE: usize = 2;
    const PROTOCOL_MAXIMUM_LENGTH_MARGIN: usize = 1024;
    SKKSERV_MAXIMUM_MIDASHI_LENGTH * PROTOCOL_MAXIMUM_LENGTH_UTF8_SCALE
        + PROTOCOL_MAXIMUM_LENGTH_MARGIN
};
const PROTOCOL_MINIMUM_LENGTH: usize = 3; // "1a ".len()

const SHA1_READ_BUFFER_LENGTH: usize = 64 * 1024;
const RESULT_VEC_CAPACITY: usize = 2 * 1024;
const MIDASHI_VEC_CAPACITY: usize = 1024;

const INITIAL_DICTIONARY_FILE_READ_BUFFER_LENGTH: usize = 8 * 1024;

static GOOGLE_CACHE_OBJECT: once_cell::sync::Lazy<RwLock<GoogleCacheObject>> =
    once_cell::sync::Lazy::new(|| RwLock::new(GoogleCacheObject::new()));

pub(in crate::skk) enum HandleClientResult {
    Continue,
    Exit,
}

enum RunLoopListenerResult {
    Nop,
    Break,
}

enum RunLoopTokenResult {
    Nop,
    Return,
}

pub(in crate::skk) struct DictionaryFile {
    file: File,
    seek_position: u64,
    read_length: usize,
    buffer: Vec<u8>,
}

impl DictionaryFile {
    pub(in crate::skk) fn new(file: File, buffer_length: usize) -> Self {
        Self {
            file,
            seek_position: 0,
            read_length: 0,
            buffer: vec![0; buffer_length],
        }
    }

    fn read(&mut self, seek_position: u64, read_length: usize) -> Result<&[u8], SkkError> {
        if self.seek_position != seek_position || self.read_length != read_length {
            self.seek_position = seek_position;
            self.read_length = read_length;
            if read_length > self.buffer.len() {
                self.buffer = vec![0; read_length];
            }
            self.file.seek(std::io::SeekFrom::Start(seek_position))?;
            self.file.read_exact(&mut self.buffer[..read_length])?;
        }
        Ok(&self.buffer[..read_length])
    }
}

struct MioSocket {
    buffer_stream: BufReader<TcpStream>,
}

impl MioSocket {
    fn new(stream: TcpStream) -> Self {
        Self {
            buffer_stream: BufReader::new(stream),
        }
    }
}

pub(in crate::skk) trait TcpStreamSkk: Write {
    fn write_all_flush_ignore_error(&mut self, data: &[u8]) {
        if let Err(e) = self.write_all(data) {
            Yaskkserv2::log_error(&format!("write_all() failed {}", e));
            return;
        }
        if let Err(e) = self.flush() {
            Yaskkserv2::log_error(&format!("flush() failed {}", e));
        }
    }

    fn write_all_flush(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        if let Err(e) = self.write_all(data) {
            Yaskkserv2::log_error(&format!("write_all() failed {}", e));
            return Err(e);
        }
        if let Err(e) = self.flush() {
            Yaskkserv2::log_error(&format!("flush() failed {}", e));
            return Err(e);
        }
        Ok(())
    }

    fn write_disconnect_flush(&mut self) -> Result<(), std::io::Error> {
        self.write_all_flush(b"0")
    }

    fn write_error_flush(&mut self) -> Result<(), std::io::Error> {
        self.write_all_flush(PROTOCOL_RESULT_ERROR)
    }
}

impl TcpStreamSkk for TcpStream {}
impl TcpStreamSkk for &TcpStream {}
impl TcpStreamSkk for std::net::TcpStream {}
impl TcpStreamSkk for &std::net::TcpStream {}

trait BufReaderSkk {
    fn read_until_skk_server(&mut self, buffer: &mut Vec<u8>) -> Result<usize, std::io::Error>;
}

impl BufReaderSkk for BufReader<TcpStream> {
    fn read_until_skk_server(&mut self, buffer: &mut Vec<u8>) -> Result<usize, std::io::Error> {
        fn find_one_character_protocol(available: &[u8]) -> Option<usize> {
            for (i, c) in available.iter().enumerate() {
                if *c == b'0' || *c == b'2' || *c == b'3' {
                    return Some(i);
                }
                if *c != b'\n' && *c != b'\r' {
                    break;
                }
            }
            None
        }
        let mut read = 0;
        loop {
            let (done, used) = {
                let available = match self.fill_buf() {
                    Ok(n) => n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
                match twoway::find_bytes(available, b" ") {
                    Some(i) => {
                        buffer.extend_from_slice(&available[..=i]);
                        (true, i + 1)
                    }
                    None =>
                    {
                        #[allow(clippy::option_if_let_else)]
                        if let Some(i) = find_one_character_protocol(available) {
                            buffer.extend_from_slice(&available[..=i]);
                            (true, i + 1)
                        } else {
                            buffer.extend_from_slice(available);
                            (false, available.len())
                        }
                    }
                }
            };
            self.consume(used);
            read += used;
            if done || used == 0 {
                return Ok(read);
            }
        }
    }
}

// is_debug_force_exit_mode が true のとき、
// "std::env::var("YASKKSERV2_TEST_DIRECTORY")/DEBUG_FORCE_EXIT_DIRECTORY" を server の強制
// 終了 flag として使う。このディレクトリが存在した場合、ディレクトリを削除してから server loop
// を強制終了する。ディレクトリの存在チェックタイミングは read_until_skk_server() の直後なので
// 何らかの yaskkserv2 的に正しい (read_until_skk_server() を抜ける) 通信が発生しないと強制
// 終了しないことに注意。
// (本来は channel で綺麗に実装したいところだが、通常は強制終了の必要が無く、どうしても必要な
//  場面が server を想定外の叩き方をする test のみで、 channel よりもシンプルに実装できるため、
//  このようなかたちとなっている)
pub(in crate::skk) struct Yaskkserv2 {
    server: Server,
    #[cfg(test)]
    pub(in crate::skk) is_debug_force_exit_mode: bool,
}

impl Yaskkserv2 {
    pub(in crate::skk) fn new() -> Self {
        Self {
            server: Server::new(),
            #[cfg(test)]
            is_debug_force_exit_mode: false,
        }
    }

    pub(in crate::skk) fn setup(&mut self, config: &Config) -> Result<(), SkkError> {
        if config.is_google_cache_enabled {
            GoogleCache::setup_use_rwlock_internally(&config.google_cache_full_path)?;
        }
        self.server.setup(
            config.clone(),
            Dictionary::setup(SHA1_READ_BUFFER_LENGTH, &config.dictionary_full_path)?,
        );
        Ok(())
    }

    pub(in crate::skk) fn run(&mut self) {
        Self::log_info(&format!(
            "version {} (port={})",
            PKG_VERSION, self.server.config.port
        ));
        #[cfg(test)]
        if let Err(e) = self.run_loop(0) {
            let message = format!("run_loop() failed {}", e);
            Self::log_error(&message);
            Self::print_warning(&message);
        }
        #[cfg(not(test))]
        if let Err(e) = self.run_loop() {
            let message = format!("run_loop() failed {}", e);
            Self::log_error(&message);
            Self::print_warning(&message);
        }
    }

    /// `read_candidates()` や `read_abbrev()` など、 `b'1'` や `b'4'` からはじまる candidates を返す
    /// ものは空の場合でも `len() == 0` とならないので、本関数で空かどうか判定する。
    const fn is_empty_candidates(candidates: &[u8]) -> bool {
        #[cfg(feature = "assert_paranoia")]
        {
            const_assert!(candidates[0] == b'1' || candidates[0] == b'4');
        }
        candidates.len() == 1
    }

    /// empty な index を取得する
    ///
    /// # Panics
    ///
    /// index が見付からない場合、 panic!() することに注意。
    fn get_empty_sockets_index(
        sockets: &[Option<MioSocket>],
        sockets_length: usize,
        next_socket_index: usize,
    ) -> usize {
        let mut index = next_socket_index;
        index += 1;
        if index >= sockets_length {
            index = 0;
        }
        if sockets[index].is_none() {
            return index;
        }
        for _ in 0..sockets_length {
            index += 1;
            if index >= sockets_length {
                index = 0;
            }
            if sockets[index].is_none() {
                return index;
            }
        }
        panic!("illegal sockets slice");
    }

    /// sockets に `HashMap` ではなく `Vec` を使用する理由は、常に実行される `sockets.get_mut()`
    /// の速度を重視するため。
    /// なお、 `Vec` は `HashMap` に比べて empty index を探す必要がある分だけ `insert()` 相当の
    /// 処理が少しだけ高くつくが、最悪のケースでもそもそも `insert()` 相当処理の実行頻度は
    /// 低いので問題にならない。
    fn run_loop(&mut self, #[cfg(test)] take_count_for_test: usize) -> Result<(), SkkError> {
        const LISTENER: Token = Token(MAX_CONNECTION);
        #[cfg(test)]
        let mut take_index_for_test = 0;
        let mut sockets: Vec<Option<MioSocket>> = Vec::new();
        for _ in 0..self.server.config.max_connections {
            sockets.push(None);
        }
        let sockets_length = sockets.len();
        let mut sockets_some_count = 0;
        let mut next_socket_index = 0;
        let mut poll = Poll::new()?;
        let mut listener = TcpListener::bind(
            format!(
                "{}:{}",
                &self.server.config.listen_address, &self.server.config.port
            )
            .parse()
            .unwrap(),
        )?;
        poll.registry()
            .register(&mut listener, LISTENER, Interest::READABLE)?;
        let mut events = Events::with_capacity(MAX_CONNECTION);
        let mut dictionary_file = DictionaryFile::new(
            File::open(&self.server.config.dictionary_full_path)?,
            INITIAL_DICTIONARY_FILE_READ_BUFFER_LENGTH,
        );
        let mut buffer: Vec<u8> = Vec::new();
        loop {
            if let Err(e) = poll.poll(&mut events, None) {
                let message = &format!("poll failed {}", e);
                Self::log_error(message);
                Self::print_warning(message);
            }
            for event in &events {
                match event.token() {
                    LISTENER => loop {
                        match self.run_loop_listener(
                            &mut next_socket_index,
                            &mut sockets,
                            &mut sockets_some_count,
                            &mut poll,
                            &listener,
                            sockets_length,
                            #[cfg(test)]
                            &mut take_index_for_test,
                        ) {
                            Ok(RunLoopListenerResult::Break) => break,
                            Ok(RunLoopListenerResult::Nop) => {}
                            Err(e) => return Err(e),
                        }
                    },
                    token => {
                        match self.run_loop_token(
                            &mut next_socket_index,
                            &mut sockets,
                            &mut sockets_some_count,
                            &mut buffer,
                            &mut dictionary_file,
                            &poll,
                            token,
                            #[cfg(test)]
                            &mut take_index_for_test,
                            #[cfg(test)]
                            take_count_for_test,
                        ) {
                            Ok(RunLoopTokenResult::Return) => return Ok(()),
                            Ok(RunLoopTokenResult::Nop) => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_loop_listener(
        &self,
        next_socket_index: &mut usize,
        sockets: &mut [Option<MioSocket>],
        sockets_some_count: &mut usize,
        poll: &mut Poll,
        listener: &TcpListener,
        sockets_length: usize,
        #[cfg(test)] take_index_for_test: &mut usize,
    ) -> Result<RunLoopListenerResult, SkkError> {
        match listener.accept() {
            Ok((mut socket, _)) => {
                #[allow(clippy::cast_sign_loss)]
                if *sockets_some_count >= self.server.config.max_connections as usize {
                    return Ok(RunLoopListenerResult::Break);
                }
                #[cfg(test)]
                {
                    *take_index_for_test += 1;
                }
                let token = Token(*next_socket_index);
                poll.registry()
                    .register(&mut socket, token, Interest::READABLE)?;
                sockets[usize::from(token)] = Some(MioSocket::new(socket));
                *sockets_some_count += 1;
                #[allow(clippy::cast_sign_loss)]
                if *sockets_some_count < self.server.config.max_connections as usize {
                    *next_socket_index =
                        Self::get_empty_sockets_index(sockets, sockets_length, *next_socket_index);
                }
                Ok(RunLoopListenerResult::Nop)
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(RunLoopListenerResult::Break)
            }
            Err(e) => Err(SkkError::Io(e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_loop_token(
        &mut self,
        next_socket_index: &mut usize,
        sockets: &mut [Option<MioSocket>],
        sockets_some_count: &mut usize,
        buffer: &mut Vec<u8>,
        dictionary_file: &mut DictionaryFile,
        poll: &Poll,
        token: Token,
        #[cfg(test)] take_index_for_test: &mut usize,
        #[cfg(test)] take_count_for_test: usize,
    ) -> Result<RunLoopTokenResult, SkkError> {
        #[allow(clippy::get_unwrap)]
        let socket = match sockets.get_mut(usize::from(token)).unwrap() {
            Some(socket) => socket,
            None => {
                let message = "sockets get failed";
                Self::log_error(message);
                Self::print_warning(message);
                return Ok(RunLoopTokenResult::Return);
            }
        };
        let mut is_shutdown = false;
        match self.read_until_skk_server(socket, buffer, dictionary_file, &mut is_shutdown) {
            HandleClientResult::Continue => {}
            HandleClientResult::Exit => {
                poll.registry().deregister(socket.buffer_stream.get_mut())?;
                if is_shutdown {
                    if let Err(e) = &socket.buffer_stream.get_mut().shutdown(Shutdown::Both) {
                        Self::log_error(&format!("shutdown error={}", e));
                    }
                }
                sockets[usize::from(token)] = None;
                *sockets_some_count -= 1;
                *next_socket_index = usize::from(token);
                #[cfg(test)]
                {
                    if take_count_for_test > 0
                        && *sockets_some_count == 0
                        && *take_index_for_test >= take_count_for_test
                    {
                        return Ok(RunLoopTokenResult::Return);
                    }
                }
            }
        }
        #[cfg(test)]
        {
            if self.is_debug_force_exit_mode && std::env::var("YASKKSERV2_TEST_DIRECTORY").is_ok() {
                let debug_force_exit_directory_full_path =
                    std::path::Path::new(&std::env::var("YASKKSERV2_TEST_DIRECTORY").unwrap())
                        .join(DEBUG_FORCE_EXIT_DIRECTORY);
                if debug_force_exit_directory_full_path.exists() {
                    std::fs::remove_dir(&debug_force_exit_directory_full_path).unwrap();
                    return Ok(RunLoopTokenResult::Return);
                }
            }
        }
        buffer.clear();
        Ok(RunLoopTokenResult::Nop)
    }

    fn read_until_skk_server(
        &mut self,
        socket: &mut MioSocket,
        buffer: &mut Vec<u8>,
        dictionary_file: &mut DictionaryFile,
        is_shutdown: &mut bool,
    ) -> HandleClientResult {
        match socket.buffer_stream.read_until_skk_server(buffer) {
            Ok(0) => HandleClientResult::Exit,
            Ok(size) => {
                let skip = Self::get_buffer_skip_count(buffer, size);
                if size == skip {
                    HandleClientResult::Exit
                } else if size - skip > 0 {
                    self.server.handle_client(
                        &mut socket.buffer_stream,
                        dictionary_file,
                        &mut buffer[skip..],
                    )
                } else {
                    HandleClientResult::Continue
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    HandleClientResult::Continue
                } else {
                    match socket.buffer_stream.get_ref().peer_addr() {
                        Ok(peer_addr) => Self::log_error(&format!(
                            "read_line() error={}  port={}",
                            peer_addr, self.server.config.port
                        )),
                        Err(e) => Self::log_error(&format!(
                            "peer_address() get failed error={}  port={}",
                            e, self.server.config.port
                        )),
                    };
                    *is_shutdown = true;
                    HandleClientResult::Exit
                }
            }
        }
    }

    const fn get_buffer_skip_count(buffer: &[u8], size: usize) -> usize {
        if size >= 2
            && (buffer[1] == b'\n' || buffer[1] == b'\r')
            && (buffer[0] == b'\n' || buffer[0] == b'\r')
        {
            2
        } else if size >= 1 && (buffer[0] == b'\n' || buffer[0] == b'\r') {
            1
        } else {
            0
        }
    }

    fn print_warning(message: &str) {
        println!("Warning: {}", message);
    }

    #[cfg(all(not(test), unix))]
    fn get_log_formatter() -> Formatter3164 {
        #[allow(clippy::cast_possible_wrap)]
        Formatter3164 {
            facility: Facility::LOG_DAEMON,
            hostname: None,
            process: PKG_NAME.into(),
            pid: std::process::id() as i32,
        }
    }

    #[cfg(test)]
    fn log_error(message: &str) {
        println!("Error: {}", message);
    }

    #[cfg(all(not(test), unix))]
    fn log_error(message: &str) {
        match syslog::unix(Self::get_log_formatter()) {
            Err(e) => println!("impossible to connect to syslog: {:?}", e),
            Ok(mut writer) => {
                writer.err(message).expect("could not write error message");
            }
        }
    }

    #[cfg(all(not(test), not(unix)))]
    fn log_error(message: &str) {
        error!("{}", message);
    }

    #[cfg(test)]
    fn log_info(message: &str) {
        println!("Info: {}", message);
    }

    #[cfg(all(not(test), unix))]
    fn log_info(message: &str) {
        match syslog::unix(Self::get_log_formatter()) {
            Err(e) => println!("impossible to connect to syslog: {:?}", e),
            Ok(mut writer) => {
                writer.info(message).expect("could not send message");
            }
        }
    }

    #[cfg(all(not(test), not(unix)))]
    fn log_info(message: &str) {
        info!("{}", message);
    }
}

struct Server {
    config: Config,
    dictionary: DictionaryReader,
}

pub(in crate::skk) struct DictionaryReader {
    config: Config,
    google_japanese_input_protocol: String,
    google_suggest_protocol: String,
    on_memory: OnMemory,
}

type GoogleCacheBTreeMap = BTreeMap<Vec<u8>, Vec<Vec<u8>>>;

struct GoogleCacheObject {
    map: GoogleCacheBTreeMap,
}

impl GoogleCacheObject {
    fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }
}

pub(in crate::skk) struct GoogleCache;
struct Request;
