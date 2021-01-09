//! `encoding_simple`
//!
//! # Usage
//!
//! 1. `EncodingTable::create_table()` などで euc-jis-2004-std.txt から convert table を作成
//! 2. `EncodingTable::setup()` で `encoding table` を setup (table から Hash や Vec を用意する)
//! 3. `Euc::encode/Euc::decode` で変換
//!
//!
//! # convert table format
//!
//! header
//!     - `version: u32 (1)`
//!     - `header length: u32 (32)`
//!     - `euc_utf8_combine_length: u32 (25)`
//!     - `euc_utf8_length: u32 (11431)`
//!     - `reserved: u32 (0)`
//!     - `reserved: u32 (0)`
//!     - `reserved: u32 (0)`
//!     - `reserved: u32 (0)`
//! `euc_utf8_combining: [u8; euc_utf8_combine_length * (3 + 4 + 4)]`
//! `euc_utf8:           [u8; euc_utf8_length * (3 + 4)]`
//!
//! 3 + 4 + 4 は euc(3bytes) と utf8(4bytes) 結合文字 2 文字分。
//! 3 + 4 は euc(3bytes) と utf8(4bytes) 1 文字分。
//!
//!
//! # memo
//!
//! `Vec` と `FxHashMap` が混在しているのは高速化のため。
//! 頻繁にアクセスする table を Vec で保持している。
//!
//! - `EUC_2_TO_UTF8_VEC: 23920 * 4 = 95680 bytes`
//! - `UTF8_3_TO_EUC_VEC: 34792 * 3 = 104376 bytes`
//!
//! `Vec` の添字は容量削減のため簡単な計算を必要とするので、下記関数で取得する。
//!
//! - `Decoder::get_euc_2_to_utf8_vec_index()`
//! - `Encoder::get_utf8_3_to_euc_vec_index()`
//!
//! prefix `EUC_2_` は euc の 2 bytes を key とすることを意味する。
//! `UTF8_3_` は utf8 の 3 bytes 、 `UTF8_2_4_` は utf8 の 2 bytes or 4 bytes が key となる。

mod decoder;
mod encoder;
mod encoding_table;
mod encoding_table_get;
mod euc;

use regex::Regex;
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::RwLock;

use crate::skk::{Encoding, EncodingOptions, SkkError};

static EUC_2_TO_UTF8_VEC: once_cell::sync::Lazy<RwLock<Vec<[u8; 4]>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));
static UTF8_3_TO_EUC_VEC: once_cell::sync::Lazy<RwLock<Vec<[u8; 3]>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));
static EUC_3_TO_UTF8_MAP: once_cell::sync::Lazy<RwLock<FxHashMap<[u8; 3], [u8; 4]>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(FxHashMap::default()));
static UTF8_2_4_TO_EUC_MAP: once_cell::sync::Lazy<RwLock<FxHashMap<[u8; 4], [u8; 3]>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(FxHashMap::default()));
static COMBINE_EUC_TO_UTF8_MAP: once_cell::sync::Lazy<RwLock<FxHashMap<[u8; 3], [u8; 8]>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(FxHashMap::default()));
static COMBINE_UTF8_4_TO_EUC_MAP: once_cell::sync::Lazy<RwLock<FxHashMap<[u8; 4], [u8; 3]>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(FxHashMap::default()));
static COMBINE_UTF8_6_TO_EUC_MAP: once_cell::sync::Lazy<RwLock<FxHashMap<[u8; 6], [u8; 3]>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(FxHashMap::default()));

pub struct Euc {}

pub struct Utility {}

impl Utility {
    pub(crate) fn contains_euc_2(euc_2_to_utf8_vec: &[[u8; 4]], index: usize) -> bool {
        euc_2_to_utf8_vec[index] != [0, 0, 0, 0]
    }

    pub(crate) fn contains_utf8_3(utf8_3_to_euc_vec: &[[u8; 3]], index: usize) -> bool {
        utf8_3_to_euc_vec[index] != [0, 0, 0]
    }

    pub(crate) const fn is_enough_2_bytes(buffer_length: usize, index: usize) -> bool {
        index + 2 <= buffer_length
    }

    pub(crate) const fn is_enough_3_bytes(buffer_length: usize, index: usize) -> bool {
        index + 3 <= buffer_length
    }

    pub(crate) const fn is_enough_4_bytes(buffer_length: usize, index: usize) -> bool {
        index + 4 <= buffer_length
    }

    pub(crate) const fn is_enough_6_bytes(buffer_length: usize, index: usize) -> bool {
        index + 6 <= buffer_length
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub(crate) fn detect_encoding(buffer: &[u8]) -> Result<(Encoding, EncodingOptions), SkkError> {
        if (buffer.len() > 3) && (buffer[0] == 0xef) && (buffer[1] == 0xbb) && (buffer[2] == 0xbf) {
            return Ok((Encoding::Utf8, EncodingOptions::Bom));
        }
        let mut i = 0;
        let mut utf8_valid_count: i32 = 0;
        let mut utf8_invalid_count: i32 = 0;
        while i < buffer.len() - 3 {
            let buffer_0 = buffer[i];
            let buffer_1 = buffer[i + 1];
            let buffer_2 = buffer[i + 2];
            let buffer_3 = buffer[i + 3];
            if (0xc2..=0xdf).contains(&buffer_0) && (0x80..=0xbf).contains(&buffer_1) {
                i += 2;
                utf8_valid_count += 1;
            } else if (0xe0..=0xef).contains(&buffer_0)
                && (0x80..=0xbf).contains(&buffer_1)
                && (0x80..=0xbf).contains(&buffer_2)
            {
                i += 3;
                utf8_valid_count += 1;
            } else if (0xf0..=0xf7).contains(&buffer_0)
                && (0x80..=0xbf).contains(&buffer_1)
                && (0x80..=0xbf).contains(&buffer_2)
                && (0x80..=0xbf).contains(&buffer_3)
            {
                i += 4;
                utf8_valid_count += 1;
            } else if (0x01..=0x7f).contains(&buffer_0) {
                i += 1;
            } else {
                i += 1;
                utf8_invalid_count += 1;
            }
        }
        let ambiguous_threshold = buffer.len() as i32 / 100;
        if (utf8_valid_count - utf8_invalid_count).abs() < ambiguous_threshold {
            if twoway::find_bytes(buffer, b"coding: euc-").is_some() {
                return Ok((Encoding::Euc, EncodingOptions::None));
            }
            if twoway::find_bytes(buffer, b"coding: utf-8").is_some() {
                return Ok((Encoding::Utf8, EncodingOptions::None));
            }
            // ASCII が極端に多ければ EUC にしておく
            let near_zero_threshold = std::cmp::min(1000, buffer.len() as i32 / 1000);
            if utf8_valid_count <= near_zero_threshold && utf8_invalid_count <= near_zero_threshold
            {
                return Ok((Encoding::Euc, EncodingOptions::None));
            }
        } else if utf8_valid_count > utf8_invalid_count {
            return Ok((Encoding::Utf8, EncodingOptions::None));
        } else {
            return Ok((Encoding::Euc, EncodingOptions::None));
        }
        Err(SkkError::Encoding)
    }
}

struct Decoder {}

struct Encoder {}

pub struct EncodingTable {}
