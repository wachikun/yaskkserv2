//! `yaskkserv2_make_dictionary`
//!
//! # はじめに
//!
//! SKK-JISYO から yaskkserv2 専用の dictionary を作成する。
//!
//! 文字コード変換、複数 SKK 辞書のマージやその正当性確認などの面倒な処理は可能な限り
//! `yaskkserv2_make_dictionary` 側で事前に済ませ、 daemon として実行される `yaskkserv2` 本体側
//! での処理を簡略化している。
//!
//! `yaskkserv2` 本体に比べると、辞書全体を map で保持するなどリソースを大量に使用する。とはいえ
//! SKK-JISYO のサイズは巨大なものでも 30MB に満たないため、現代的な Rust が動作するような
//! 環境で問題になることはない。
//!
//!
//! # SKK-JISYO について
//!
//! - utf8 か euc の SKK 辞書に対応
//! - 改行コードは LF 、 CRLF または CR (混在可能)
//! - SKK 辞書は内部的に JISYO と表記される
//! - JISYO は `yaskkserv2_make_dictionary` コマンドで dictionary と呼ばれる形式に変換される
//! - 複数与えられた JISYO は 1 つの dictionary に merge される
//!
//!
//! # candidate の重複ルール
//!
//! - 元 candidate に annotate があれば元 candidate を使用
//! - 新 candidate に annotate があって元 candidate に annotate が無ければ新 candidate を使用
//!
//! - `base:"/aaa;annotate/"  new:"/aaa;ANNOTATE/"  ->  "/aaa;annotate/"`
//! - `base:"/aaa/"           new:"/aaa;ANNOTATE/"  ->  "/aaa;ANNOTATE/"`
//! - `base:"/aaa;annotate/"  new:"/aaa/"           ->  "/aaa;annotate/"`
//!
//! これは dictionary 作成時にも、 google API などで動的に candidate が重複するような場合にも
//! 同じルールが適用される (もっとも google API に annotate が付加されることはないが)。
//!
//!
//! # dictionary format
//!
//! +------------------------------------------------------------------------------------------------
//! | `struct DictionaryFixedHeader`
//! |
//! |     `DICTIONARY_FIXED_HEADER_AREA_LENGTH` bytes 以下であることが保証された raw data 。
//! |
//! |     無条件に `DICTIONARY_FIXED_HEADER_AREA_LENGTH` bytes read 可能。
//! +------------------------------------------------------------------------------------------------
//! | encoding table
//! |
//! |     `encoding_simple` 用の encoding table 。
//! |
//! |     `DictionaryFixedHeader` の offset/length で read する。
//! +------------------------------------------------------------------------------------------------
//! | index data header
//! |
//! |     `IndexAsciiHiraganaVec/IndexMap` 生成用 data header。
//! |
//! |     `DictionaryFixedHeader` の offset/length で read する。
//! |
//! |     `index_data_header: IndexDataHeader`
//! |         `block_buffer_length` と `block_header_length`
//! |     `block_header: [IndexDataHeaderBlockHeader; index_data_header.block_header_length]`
//! |         block の `offset, length` と `unit_length`
//! +------------------------------------------------------------------------------------------------
//! | index data
//! |
//! |     `IndexAsciiHiraganaVec/IndexMap` 生成用 data 。
//! |
//! |     index data header の offset/length で read する。
//! |
//! |     読み込み時 index data を一気に読まずに済ませるため (とはいえ、現状は大きくても 512k
//! |     に満たないが) index data header の `block_buffer_length (INDEX_DATA_BLOCK_LENGTH)` 以下
//! |     になる block と呼ばれる単位で分割されている。
//! |
//! |     下記 block が `block_length` 個続く
//! |     {
//! |         下記 unit が `unit_length` 個続く
//! |         {
//! |             `dictionary_block_heads:`
//! |                 `DictionaryBlockHeads`
//! |             `dictionary_block_informations_offset_length:`
//! |                 `[BlockInformationOffsetLength; dictionary_block_informations_length]`
//! |             `dictionary_block_informations_midashi:`
//! |                 `[u8; dictionary_block_informations_midashi_length]`
//! |                 `b' '` 区切りの `DictionaryBlockInformation` の midashi
//! |         }
//! |     }
//! +------------------------------------------------------------------------------------------------
//! | string blocks
//! |
//! |     生の文字列群。 midashi は必ず euc だが、 candidates は euc だけではなく utf8 の場合も
//! |     あるので文字コードが混在する場合があることに注意。
//! |
//! |     `IndexMap` の value である `DictionaryBlockInformation` の midashi/offset/length で
//! |     read する。
//! |
//! |     `"\nmidashiA candidatesA\nmidashiB candidatesB\n"` のように entry の前後に `b'\n'` が
//! |     必ず含まれる。 (block の先頭にも末尾にも必ず \n が存在する)
//! +------------------------------------------------------------------------------------------------
//!
//!
//! # `IndexAsciiHiraganaVec/IndexMap`
//!
//! `IndexAsciiHiraganaVec/IndexMap` は midashi に対応する candidates の情報を保持する `Vec/Nap` 。
//! `Vec` と `Map` に分かれているのは高速化のため。ほとんどのケースで高速な `IndexAsciiHiraganaVec` を
//! アクセスするだけで済む (`Map` 信頼できないデータを扱うわけではないので `FxHashMap` を使用し
//! 高速化しているが、それでも `Vec` に比べると圧倒的に遅い) 。
//!
//! `IndexAsciiHiraganaVec/IndexMap` の value は `DictionaryBlockInformation` の `Vec` だが、 value
//! へ直接 key に対応するデータを持たず `Vec` に分割しているのは、探索時に扱うデータサイズを
//! 小さくするため。このサイズは I/O と文字列探索にかかる時間にほぼ比例するので、適切な値を
//! 指定する必要がある。 `Vec` 分割しない場合これは 1MB 以上となるが、これを
//! `DICTIONARY_BLOCK_UNIT_LENGTH` 以下になるよう `Vec` に分割している。

mod dictionary_creator;
mod jisyo_creator;
mod jisyo_reader;

pub(in crate::skk) mod command_line;

use sha1::Sha1;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Seek, Write};

use crate::skk::{
    encoding_simple, BlockInformationOffsetLength, Candidates, Config, Dictionary,
    DictionaryBlockHeads, DictionaryBlockInformation, DictionaryFixedHeader, DictionaryMidashiKey,
    Encoding, EncodingOptions, IndexDataHeader, IndexDataHeaderBlockHeader, OnMemory, SkkError,
    ToFromNeBytes, DICTIONARY_FIXED_HEADER_AREA_LENGTH, DICTIONARY_VERSION,
    JISYO_MAXIMUM_LINE_LENGTH, JISYO_MINIMUM_CANDIDATES_LENGTH, JISYO_MINIMUM_LINE_LENGTH,
    SHA1SUM_ZERO,
};

const JISYO_ENCODING_DETECT_BUFFER_LENGTH: usize = 16 * 1024;
const SHA1_READ_BUFFER_LENGTH: usize = 256 * 1024;

const HEADER_ALIGNMENT_LENGTH: u32 = 4 * 1024;
const BLOCK_ALIGNMENT_LENGTH: u32 = 16;

const INDEX_DATA_BLOCK_LENGTH: usize = 64 * 1024;

// 基本的にこの値を基準として I/O read した後に文字列探索するので、パフォーマンスに影響する
// ことに注意。
const DICTIONARY_BLOCK_UNIT_LENGTH: usize = 2 * 1024;

type JisyoEntriesMap = BTreeMap<Vec<u8>, Vec<u8>>;
type TemporaryBlockMap = BTreeMap<DictionaryMidashiKey, Vec<u8>>;

pub(in crate::skk) struct Yaskkserv2MakeDictionary {}

impl Yaskkserv2MakeDictionary {
    #[allow(dead_code)]
    pub(in crate::skk) fn run_create_dictionary(
        config: &Config,
        encoding_table: &[u8],
        jisyo_full_paths: &[String],
    ) -> Result<(), SkkError> {
        DictionaryCreator::create(config, encoding_table, jisyo_full_paths)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(in crate::skk) fn run_create_jisyo(
        config: &Config,
        output_jisyo_full_path: &str,
    ) -> Result<(), SkkError> {
        JisyoCreator::create(config, output_jisyo_full_path)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(in crate::skk) fn run_create_jisyo_from_cache(
        input_cache_full_path: &str,
        output_jisyo_full_path: &str,
        output_jisyo_encoding: Encoding,
    ) -> Result<(), SkkError> {
        JisyoCreator::create_from_cache(
            input_cache_full_path,
            output_jisyo_full_path,
            output_jisyo_encoding,
        )?;
        Ok(())
    }

    fn print_warning(message: &str) {
        println!("Warning: {}", message);
    }

    fn print_error(message: &str) {
        println!("Error: {}", message);
    }
}

pub(in crate::skk) struct JisyoReader {}

struct JisyoCreator {}

pub(in crate::skk) struct DictionaryCreator {}
