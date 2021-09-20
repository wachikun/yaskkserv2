mod candidates;
mod dictionary;
mod encoding_simple;
mod error;
mod yaskkserv2;
mod yaskkserv2_make_dictionary;

#[cfg(unix)]
#[cfg(test)]
mod test_unix;

#[cfg(unix)]
use daemonize::Daemonize;
use rustc_hash::FxHashMap;
use std::convert::TryInto;

use crate::skk::yaskkserv2::Yaskkserv2;
use crate::skk::yaskkserv2_make_dictionary::Yaskkserv2MakeDictionary;

pub(in crate::skk) use crate::skk::error::SkkError;

#[macro_export]
macro_rules! define_builder {
    ($name: ident, $type: ty) => {
        #[allow(dead_code, clippy::wrong_self_convention, clippy::missing_const_for_fn)]
        fn $name(mut self, $name: $type) -> Self {
            self.$name = $name;
            self
        }
    };
}

const DICTIONARY_FIXED_HEADER_AREA_LENGTH: u32 = 256;
#[cfg(not(test))]
const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const DICTIONARY_VERSION: u32 = 1;
const DEFAULT_PORT: i32 = 1178;
const DEFAULT_MAX_CONNECTIONS: i32 = 16;
const DEFAULT_LISTEN_ADDRESS: &str = "0.0.0.0";
const DEFAULT_CONFIG_FULL_PATH: &str = "/etc/yaskkserv2.conf";
const DEFAULT_HOSTNAME_AND_IP_ADDRESS_FOR_PROTOCOL_3: &str = "localhost:127.0.0.1";
const DEFAULT_GOOGLE_TIMEOUT_MILLISECONDS: u64 = 1000;
const DEFAULT_GOOGLE_CACHE_FULL_PATH: &str = "/tmp/yaskkserv2.google_cache";
const DEFAULT_GOOGLE_CACHE_ENTRIES: usize = 1024;
const DEFAULT_GOOGLE_CACHE_EXPIRE_SECONDS: u64 = 30 * 24 * 60 * 60;
const DEFAULT_GOOGLE_MAX_CANDIDATES_LENGTH: usize = 5 * 5;
const DEFAULT_MAX_SERVER_COMPLETIONS: u32 = 64;
const GOOGLE_JAPANESE_INPUT_URL: &str = "://www.google.com/transliterate?langpair=ja-Hira|ja&text=";
const GOOGLE_SUGGEST_URL: &str = "://www.google.com/complete/search?hl=ja&output=toolbar&q=";
const JISYO_MAXIMUM_LINE_LENGTH: usize = 128 * 1024;
const JISYO_MINIMUM_LINE_LENGTH: usize = 5; // b"M /C/".len()  (M = Midashi, C = Candidates)
const JISYO_MINIMUM_CANDIDATES_LENGTH: usize = 3; // b"/C/".len()
const PROTOCOL_RESULT_ERROR: &[u8; 2] = b"0\n";
const SHA1SUM_LENGTH: usize = 20;
const SHA1SUM_ZERO: [u8; SHA1SUM_LENGTH] = [0; SHA1SUM_LENGTH];
const INDEX_ASCII_HIRAGANA_VEC_LENGTH: usize = 256;

static ONCE_INIT: std::sync::Once = std::sync::Once::new();

fn once_init_encoding_table(encoding_table: &[u8]) {
    ONCE_INIT.call_once(|| {
        encoding_simple::EncodingTable::setup(encoding_table)
            .expect("encoding_simple setup failed");
    });
}

trait U8Slice {
    // buffer[offset].to_ne_u32() のように書くと遅いので引数に offset を取っている
    // buffer[offset..offset + 4].to_ne_u32() と書くと buffer.to_ne_u32(offset) と等速だが繁雑
    fn to_ne_u32(&self, offset: usize) -> u32;
    fn to_ne_u32_2(&self, offset: usize) -> (u32, u32);
    fn to_ne_u32_3(&self, offset: usize) -> (u32, u32, u32);
    fn to_array_4(&self, offset: usize) -> [u8; 4];
}

impl U8Slice for [u8] {
    fn to_ne_u32(&self, offset: usize) -> u32 {
        u32::from_ne_bytes(self[offset..offset + 4].try_into().unwrap())
    }

    fn to_ne_u32_2(&self, offset: usize) -> (u32, u32) {
        (self.to_ne_u32(offset), self.to_ne_u32(offset + 4))
    }

    fn to_ne_u32_3(&self, offset: usize) -> (u32, u32, u32) {
        (
            self.to_ne_u32(offset),
            self.to_ne_u32(offset + 4),
            self.to_ne_u32(offset + 2 * 4),
        )
    }

    fn to_array_4(&self, offset: usize) -> [u8; 4] {
        self[offset..offset + 4].try_into().unwrap()
    }
}

struct Dictionary;
struct Candidates;

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Default)]
pub(in crate::skk) struct Config {
    port: String,
    max_connections: i32,
    listen_address: String,
    hostname_and_ip_address_for_protocol_3: String,
    dictionary_full_path: String,
    config_full_path: String,
    google_timeout_milliseconds: u64,
    google_timing: GoogleTiming,
    google_cache_full_path: String,
    google_cache_entries: usize,
    google_cache_expire_seconds: u64,
    google_max_candidates_length: usize,
    max_server_completions: u32,
    google_insert_hiragana_only_candidate: bool,
    google_insert_katakana_only_candidate: bool,
    google_insert_hankaku_katakana_only_candidate: bool,
    is_http_enabled: bool,
    is_google_cache_enabled: bool,
    is_google_suggest_enabled: bool,
    encoding: Encoding,
    is_no_daemonize: bool,
    is_verbose: bool,
    #[cfg(test)]
    is_debug_send: bool,
}

impl Config {
    fn new() -> Self {
        Self {
            port: DEFAULT_PORT.to_string(),
            max_connections: DEFAULT_MAX_CONNECTIONS,
            listen_address: String::from(DEFAULT_LISTEN_ADDRESS),
            hostname_and_ip_address_for_protocol_3: String::from(
                DEFAULT_HOSTNAME_AND_IP_ADDRESS_FOR_PROTOCOL_3,
            ),
            dictionary_full_path: String::new(),
            config_full_path: String::from(DEFAULT_CONFIG_FULL_PATH),
            google_timeout_milliseconds: DEFAULT_GOOGLE_TIMEOUT_MILLISECONDS,
            google_timing: GoogleTiming::NotFound,
            google_cache_full_path: String::from(DEFAULT_GOOGLE_CACHE_FULL_PATH),
            google_cache_entries: DEFAULT_GOOGLE_CACHE_ENTRIES,
            google_cache_expire_seconds: DEFAULT_GOOGLE_CACHE_EXPIRE_SECONDS,
            google_max_candidates_length: DEFAULT_GOOGLE_MAX_CANDIDATES_LENGTH,
            max_server_completions: DEFAULT_MAX_SERVER_COMPLETIONS,
            ..Self::default()
        }
    }

    define_builder!(port, String);
    define_builder!(max_connections, i32);
    define_builder!(listen_address, String);
    define_builder!(hostname_and_ip_address_for_protocol_3, String);
    define_builder!(dictionary_full_path, String);
    define_builder!(google_timeout_milliseconds, u64);
    define_builder!(google_timing, GoogleTiming);
    define_builder!(google_cache_full_path, String);
    define_builder!(google_cache_entries, usize);
    define_builder!(google_cache_expire_seconds, u64);
    define_builder!(google_max_candidates_length, usize);
    define_builder!(max_server_completions, u32);
    define_builder!(is_http_enabled, bool);
    define_builder!(is_google_cache_enabled, bool);
    define_builder!(is_google_suggest_enabled, bool);
    define_builder!(encoding, Encoding);
    define_builder!(is_no_daemonize, bool);
    define_builder!(is_verbose, bool);

    #[cfg(test)]
    define_builder!(is_debug_send, bool);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Encoding {
    Euc,
    Utf8,
}

impl Default for Encoding {
    fn default() -> Self {
        Self::Euc
    }
}

impl std::fmt::Display for Encoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EncodingOptions {
    None,
    Bom,
}

impl Default for EncodingOptions {
    fn default() -> Self {
        Self::None
    }
}

impl Encoding {
    fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::Euc,
            1 => Self::Utf8,
            _ => panic!("unknown value={}", value),
        }
    }
}

#[derive(Clone, PartialEq)]
enum GoogleTiming {
    NotFound,
    Disable,
    Last,
    First,
}

impl Default for GoogleTiming {
    fn default() -> Self {
        Self::NotFound
    }
}

type DictionaryMidashiKey = [u8; 4];
type IndexMap = FxHashMap<DictionaryMidashiKey, Vec<DictionaryBlockInformation>>;
type IndexAsciiHiraganaVec = Vec<Vec<DictionaryBlockInformation>>;

trait ToFromNeBytes {
    fn to_ne_bytes_unsafe(&self) -> Vec<u8>
    where
        Self: std::marker::Sized,
    {
        let mut result = Vec::new();
        unsafe {
            result.extend_from_slice(std::slice::from_raw_parts(
                (self as *const Self).cast::<u8>(),
                std::mem::size_of::<Self>(),
            ));
        }
        result
    }

    fn to_ne_bytes(&self) -> Vec<u8>
    where
        Self: std::marker::Sized,
    {
        self.to_ne_bytes_unsafe()
    }

    fn from_ne_bytes_unsafe(bytes: &[u8]) -> Self
    where
        Self: std::marker::Sized,
    {
        #[deny(clippy::cast_ptr_alignment)]
        unsafe {
            std::ptr::read_unaligned((bytes.as_ptr().cast::<u8>()).cast::<Self>())
        }
    }

    fn from_ne_bytes(bytes: &[u8]) -> Self
    where
        Self: std::marker::Sized,
    {
        Self::from_ne_bytes_unsafe(bytes)
    }
}

#[repr(C)]
#[derive(Debug, PartialEq)]
struct DictionaryBlockHeads {
    information_length: u32,
    information_midashi_length: u32,
    dictionary_midashi_key: DictionaryMidashiKey,
}

impl ToFromNeBytes for DictionaryBlockHeads {
    #[cfg(feature = "assert_paranoia")]
    fn to_ne_bytes(&self) -> Vec<u8> {
        let result = self.to_ne_bytes_unsafe();
        let mut safe_result = Vec::new();
        safe_result.extend_from_slice(&self.information_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.information_midashi_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.dictionary_midashi_key);
        assert_eq!(&safe_result, &result);
        result
    }

    #[cfg(feature = "assert_paranoia")]
    fn from_ne_bytes(bytes: &[u8]) -> Self {
        let result = Self::from_ne_bytes_unsafe(bytes);
        let safe_result = Self {
            information_length: bytes.to_ne_u32(0),
            information_midashi_length: bytes.to_ne_u32(std::mem::size_of::<u32>()),
            dictionary_midashi_key: bytes.to_array_4(2 * std::mem::size_of::<u32>()),
        };
        assert_eq!(&safe_result, &result);
        result
    }
}

#[repr(C)]
#[derive(Debug, PartialEq)]
struct BlockInformationOffsetLength {
    offset: u32,
    length: u32,
}

impl ToFromNeBytes for BlockInformationOffsetLength {
    #[cfg(feature = "assert_paranoia")]
    fn to_ne_bytes(&self) -> Vec<u8> {
        let result = self.to_ne_bytes_unsafe();
        let mut safe_result = Vec::new();
        safe_result.extend_from_slice(&self.offset.to_ne_bytes());
        safe_result.extend_from_slice(&self.length.to_ne_bytes());
        assert_eq!(&safe_result, &result);
        result
    }

    #[cfg(feature = "assert_paranoia")]
    fn from_ne_bytes(bytes: &[u8]) -> Self {
        let result = Self::from_ne_bytes_unsafe(bytes);
        let safe_result = Self {
            offset: bytes.to_ne_u32(0),
            length: bytes.to_ne_u32(std::mem::size_of::<u32>()),
        };
        assert_eq!(&safe_result, &result);
        result
    }
}

#[repr(C)]
#[derive(Default, Debug, PartialEq)]
struct IndexDataHeader {
    block_buffer_length: u32,
    block_header_length: u32,
}

impl ToFromNeBytes for IndexDataHeader {
    #[cfg(feature = "assert_paranoia")]
    fn to_ne_bytes(&self) -> Vec<u8> {
        let result = self.to_ne_bytes_unsafe();
        let mut safe_result = Vec::new();
        safe_result.extend_from_slice(&self.block_buffer_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.block_header_length.to_ne_bytes());
        assert_eq!(&safe_result, &result);
        result
    }

    #[cfg(feature = "assert_paranoia")]
    fn from_ne_bytes(bytes: &[u8]) -> Self {
        let result = Self::from_ne_bytes_unsafe(bytes);
        let safe_result = Self {
            block_buffer_length: bytes.to_ne_u32(0),
            block_header_length: bytes.to_ne_u32(std::mem::size_of::<u32>()),
        };
        assert_eq!(&safe_result, &result);
        result
    }
}

#[repr(C)]
#[derive(Debug, PartialEq)]
struct IndexDataHeaderBlockHeader {
    offset: u32,
    length: u32,
    unit_length: u32,
}

impl ToFromNeBytes for IndexDataHeaderBlockHeader {
    #[cfg(feature = "assert_paranoia")]
    fn to_ne_bytes(&self) -> Vec<u8> {
        let result = self.to_ne_bytes_unsafe();
        let mut safe_result = Vec::new();
        safe_result.extend_from_slice(&self.offset.to_ne_bytes());
        safe_result.extend_from_slice(&self.length.to_ne_bytes());
        safe_result.extend_from_slice(&self.unit_length.to_ne_bytes());
        assert_eq!(&safe_result, &result);
        result
    }

    #[cfg(feature = "assert_paranoia")]
    fn from_ne_bytes(bytes: &[u8]) -> Self {
        let result = Self::from_ne_bytes_unsafe(bytes);
        let safe_result = Self {
            offset: bytes.to_ne_u32(0),
            length: bytes.to_ne_u32(std::mem::size_of::<u32>()),
            unit_length: bytes.to_ne_u32(2 * std::mem::size_of::<u32>()),
        };
        assert_eq!(&safe_result, &result);
        result
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
struct DictionaryFixedHeader {
    dictionary_version: u32,
    encoding_table_offset: u32,
    encoding_table_length: u32,
    index_data_header_offset: u32,
    index_data_header_length: u32,
    index_data_offset: u32,
    index_data_length: u32,
    blocks_offset: u32,
    blocks_length: u32,
    dictionary_length: u32,
    encoding: u32,
    // std::mem::size_of::<Self>() - SHA1SUM_LENGTH で offset を求めているため、 sha1sum は
    // 末尾に置く必要があることに注意
    sha1sum: [u8; SHA1SUM_LENGTH],
}

impl DictionaryFixedHeader {
    fn new() -> Self {
        Self {
            dictionary_version: DICTIONARY_VERSION,
            sha1sum: SHA1SUM_ZERO,
            ..Self::default()
        }
    }
}

impl ToFromNeBytes for DictionaryFixedHeader {
    #[cfg(feature = "assert_paranoia")]
    fn to_ne_bytes(&self) -> Vec<u8> {
        let result = self.to_ne_bytes_unsafe();
        let mut safe_result = Vec::new();
        safe_result.extend_from_slice(&self.dictionary_version.to_ne_bytes());
        safe_result.extend_from_slice(&self.encoding_table_offset.to_ne_bytes());
        safe_result.extend_from_slice(&self.encoding_table_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.index_data_header_offset.to_ne_bytes());
        safe_result.extend_from_slice(&self.index_data_header_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.index_data_offset.to_ne_bytes());
        safe_result.extend_from_slice(&self.index_data_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.blocks_offset.to_ne_bytes());
        safe_result.extend_from_slice(&self.blocks_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.dictionary_length.to_ne_bytes());
        safe_result.extend_from_slice(&self.encoding.to_ne_bytes());
        safe_result.extend_from_slice(&self.sha1sum);
        assert_eq!(&safe_result, &result);
        result
    }

    #[cfg(feature = "assert_paranoia")]
    fn from_ne_bytes(bytes: &[u8]) -> Self {
        let result = Self::from_ne_bytes_unsafe(bytes);
        let mut sha1sum = SHA1SUM_ZERO;
        sha1sum.copy_from_slice(
            &bytes[std::mem::size_of::<Self>() - SHA1SUM_LENGTH..std::mem::size_of::<Self>()],
        );
        let safe_result = Self {
            dictionary_version: bytes.to_ne_u32(0),
            encoding_table_offset: bytes.to_ne_u32(std::mem::size_of::<u32>()),
            encoding_table_length: bytes.to_ne_u32(2 * std::mem::size_of::<u32>()),
            index_data_header_offset: bytes.to_ne_u32(3 * std::mem::size_of::<u32>()),
            index_data_header_length: bytes.to_ne_u32(4 * std::mem::size_of::<u32>()),
            index_data_offset: bytes.to_ne_u32(5 * std::mem::size_of::<u32>()),
            index_data_length: bytes.to_ne_u32(6 * std::mem::size_of::<u32>()),
            blocks_offset: bytes.to_ne_u32(7 * std::mem::size_of::<u32>()),
            blocks_length: bytes.to_ne_u32(8 * std::mem::size_of::<u32>()),
            dictionary_length: bytes.to_ne_u32(9 * std::mem::size_of::<u32>()),
            encoding: bytes.to_ne_u32(10 * std::mem::size_of::<u32>()),
            sha1sum,
        };
        assert_eq!(&safe_result, &result);
        result
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DictionaryBlockInformation {
    midashi: Vec<u8>,
    offset: u32,
    length: u32,
}

/// `index_map` と `index_ascii_hiragana_vec` が分かれているのは高速化のため。
/// 頻繁にアクセスするひらがなを `Vec` で保持している。 ASCII は benchmark 対策のおまけ。
/// `index_ascii_hiragana_vec` の index は `OnMemory::get_ascii_hiragana_vec_index()` で取得する。
/// `index_ascii_hiragana_vec` は `INDEX_ASCII_HIRAGANA_VEC_LENGTH(256)` 要素で 0x00-0x7f が ASCII
/// 用、 0x80-0xff がひらがなの 2 bytes 目用の領域となっている。(便宜上ひらがなとしているが、
/// 1 byte 目が 0xa4 のものをすべて対象としている。 2 bytes 目も実装を単純にするため、詰める
/// ようなことをせず 0xa1-0xf3 をそのままマップしている。)
#[derive(Clone)]
pub(in crate::skk) struct OnMemory {
    dictionary_fixed_header: DictionaryFixedHeader,
    index_map: IndexMap,
    index_ascii_hiragana_vec: IndexAsciiHiraganaVec,
}

impl OnMemory {
    fn new() -> Self {
        Self {
            dictionary_fixed_header: DictionaryFixedHeader::new(),
            index_map: IndexMap::default(),
            index_ascii_hiragana_vec: IndexAsciiHiraganaVec::new(),
        }
    }

    const fn get_ascii_hiragana_vec_index(
        dictionary_midashi_key: DictionaryMidashiKey,
    ) -> Option<usize> {
        match dictionary_midashi_key[0] {
            0x00..=0x7f => Some(dictionary_midashi_key[0] as usize),
            0xa4 => Some(dictionary_midashi_key[1] as usize),
            _ => None,
        }
    }
}

/// # Errors
///
/// 起動時、すなわち config file や dictionary の読み込みに失敗した場合は `Err` を返す。
/// 起動後に `Err` を返すことはない。
#[allow(dead_code)]
pub fn run_yaskkserv2() -> Result<(), SkkError> {
    let mut command_line = yaskkserv2::command_line::Yaskkserv2CommandLine::new();
    let is_help_exit = command_line.start()?;
    if is_help_exit {
        return Ok(());
    }
    let mut core = Yaskkserv2::new();
    let command_line_config = command_line.get_config();
    let mut config_file = yaskkserv2::config_file::Yaskkserv2ConfigFile::new(&command_line_config);
    config_file.read()?;
    let config = config_file.get_config();
    core.setup(&config)?;
    run_yaskkserv2_impl(&mut core, config.is_no_daemonize);
    Ok(())
}

#[cfg(unix)]
fn run_yaskkserv2_impl(core: &mut Yaskkserv2, is_no_daemonize: bool) {
    if is_no_daemonize {
        core.run();
    } else {
        let daemonize = Daemonize::new();
        match daemonize.start() {
            Ok(_) => core.run(),
            Err(e) => println!("Error: {}", e),
        }
    }
}

#[cfg(not(unix))]
fn run_yaskkserv2_impl(core: &mut Yaskkserv2, _is_no_daemonize: bool) {
    core.run();
}

/// # Errors
///
/// 辞書変換失敗や I/O error が発生した場合に `Err` を返す。
#[allow(dead_code)]
pub fn run_yaskkserv2_make_dictionary() -> Result<(), SkkError> {
    let mut command_line =
        yaskkserv2_make_dictionary::command_line::Yaskkserv2MakeDictionaryCommandLine::new();
    let is_help_exit = command_line.start()?;
    if is_help_exit {
        return Ok(());
    }
    let encoding_table = encoding_simple::EncodingTable::get();
    once_init_encoding_table(encoding_table);
    if !command_line.get_input_cache_full_path().is_empty() {
        let config = command_line.get_config();
        Yaskkserv2MakeDictionary::run_create_jisyo_from_cache(
            command_line.get_input_cache_full_path(),
            command_line.get_output_jisyo_full_path(),
            config.encoding,
        )?;
    } else if command_line.get_output_jisyo_full_path().is_empty() {
        Yaskkserv2MakeDictionary::run_create_dictionary(
            &command_line.get_config(),
            encoding_table,
            &command_line.get_jisyo_full_paths(),
        )?;
    } else {
        Yaskkserv2MakeDictionary::run_create_jisyo(
            &command_line.get_config(),
            command_line.get_output_jisyo_full_path(),
        )?;
    }
    Ok(())
}
