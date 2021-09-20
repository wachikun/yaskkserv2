use rand::Rng;
use sha1::Sha1;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};

use crate::skk::test_unix::{
    read_jisyo_entries_no_encoding_conversion, setup, BufRead, Path, Read,
};
use crate::skk::{
    encoding_simple, Config, Dictionary, DictionaryFixedHeader, Encoding, FxHashMap, ToFromNeBytes,
    Yaskkserv2MakeDictionary, DICTIONARY_FIXED_HEADER_AREA_LENGTH, JISYO_MINIMUM_LINE_LENGTH,
    SHA1SUM_LENGTH,
};

#[derive(Default)]
struct Jisyo {
    test_jisyo_full_path: String,
    compare_euc_jisyo_hash: Option<[u8; SHA1SUM_LENGTH]>,
    compare_utf8_jisyo_hash: Option<[u8; SHA1SUM_LENGTH]>,
}

impl Jisyo {
    fn new(name: &str) -> Self {
        Self {
            test_jisyo_full_path: Path::get_full_path(&format!("test_jisyo.{}.jisyo", name)),
            ..Self::default()
        }
    }

    fn get_writer_and_jisyo_entries_no_encoding_conversion(
        &mut self,
        jisyo_filename: Option<&str>,
    ) -> (BufWriter<File>, Vec<Vec<u8>>) {
        let jisyo_entries = jisyo_filename.map_or_else(Vec::new, |jisyo_filename| {
            read_jisyo_entries_no_encoding_conversion(&Path::get_full_path(jisyo_filename))
        });
        (
            BufWriter::new(
                OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&self.test_jisyo_full_path)
                    .unwrap(),
            ),
            jisyo_entries,
        )
    }

    fn compare_and_get_hash(
        jisyo_full_path: &str,
        jisyo_hash: &Option<[u8; SHA1SUM_LENGTH]>,
    ) -> [u8; SHA1SUM_LENGTH] {
        let mut hasher = Sha1::new();
        let mut buffer = Vec::new();
        let mut reader = File::open(&jisyo_full_path).unwrap();
        reader.read_to_end(&mut buffer).unwrap();
        hasher.update(&buffer);
        let hash = hasher.digest().bytes();
        if let Some(jisyo_hash) = *jisyo_hash {
            assert_eq!(hash, jisyo_hash);
        }
        hash
    }

    fn compare_and_create_dictionary(
        encoding_table: &[u8],
        dictionary_from_test_jisyo_full_path: &str,
        dictionary_from_compare_jisyo_full_path: &str,
        compare_jisyo_from_dictionary_full_path: &str,
        dictionary_from_compare_jisyo_encoding: Encoding,
    ) {
        let config = Config::new()
            .dictionary_full_path(String::from(dictionary_from_compare_jisyo_full_path))
            .encoding(dictionary_from_compare_jisyo_encoding);
        Yaskkserv2MakeDictionary::run_create_dictionary(
            &config,
            encoding_table,
            &[String::from(compare_jisyo_from_dictionary_full_path)],
        )
        .unwrap();
        {
            let mut buffer: Vec<u8> = vec![0; DICTIONARY_FIXED_HEADER_AREA_LENGTH as usize];
            {
                let mut reader = File::open(&dictionary_from_test_jisyo_full_path).unwrap();
                reader.read_exact(&mut buffer).unwrap();
            }
            let dictionary_fixed_header_a: DictionaryFixedHeader =
                DictionaryFixedHeader::from_ne_bytes(&buffer);
            {
                let mut reader = File::open(&dictionary_from_compare_jisyo_full_path).unwrap();
                reader.read_exact(&mut buffer).unwrap();
            }
            let dictionary_fixed_header_b: DictionaryFixedHeader =
                DictionaryFixedHeader::from_ne_bytes(&buffer);
            assert_eq!(
                dictionary_fixed_header_a.sha1sum,
                dictionary_fixed_header_b.sha1sum
            );
        }
    }

    /// 下記の順で jisyo/dictionary を verify する
    ///
    /// `test_jisyo`
    /// (`self.test_jisyo_full_path` で指定して作成しておく)
    ///     ↓
    /// `dictionary_from_test_jisyo_euc`
    /// `dictionary_from_test_jisyo_utf8`
    ///     ↓
    /// `compare_euc_jisyo_from_dictionary`
    /// `compare_utf8_jisyo_from_dictionary`
    /// (`convert_and_verify()` が複数回呼ばれた場合は hash を比較(冪等性の確認))
    ///     ↓
    /// `dictionary_euc_from_compare_euc_jisyo`
    /// `dictionary_euc_from_compare_utf8_jisyo`
    /// `dictionary_utf8_from_compare_euc_jisyo`
    /// `dictionary_utf8_from_compare_utf8_jisyo`
    /// (`dictionary_from_test_jisyo_(euc|utf8)` と header の sha1sum を比較)
    fn convert_and_verify(&mut self, dictionary_from_test_jisyo_encoding: Encoding) {
        let encoding_table = encoding_simple::EncodingTable::get();
        let dictionary_from_test_jisyo_full_path = format!(
            "{}.dictionary_{}",
            self.test_jisyo_full_path,
            &dictionary_from_test_jisyo_encoding
                .to_string()
                .to_lowercase()
        );
        let compare_euc_jisyo_full_path = format!(
            "{}.compare_euc_jisyo",
            &dictionary_from_test_jisyo_full_path
        );
        let compare_utf8_jisyo_full_path = format!(
            "{}.compare_utf8_jisyo",
            &dictionary_from_test_jisyo_full_path
        );
        let dictionary_euc_from_compare_euc_jisyo_full_path = format!(
            "{}.compare_euc_jisyo.euc_dictionary",
            &dictionary_from_test_jisyo_full_path
        );
        let dictionary_utf8_from_compare_euc_jisyo_full_path = format!(
            "{}.compare_euc_jisyo.utf8_dictionary",
            &dictionary_from_test_jisyo_full_path
        );
        let dictionary_euc_from_compare_utf8_jisyo_full_path = format!(
            "{}.compare_utf8_jisyo.euc_dictionary",
            &dictionary_from_test_jisyo_full_path
        );
        let dictionary_utf8_from_compare_utf8_jisyo_full_path = format!(
            "{}.compare_utf8_jisyo.utf8_dictionary",
            &dictionary_from_test_jisyo_full_path
        );
        let config = Config::new()
            .encoding(dictionary_from_test_jisyo_encoding)
            .dictionary_full_path(dictionary_from_test_jisyo_full_path.clone());
        Yaskkserv2MakeDictionary::run_create_dictionary(
            &config,
            encoding_table,
            &[self.test_jisyo_full_path.clone()],
        )
        .unwrap();
        {
            let mut config = config.clone();
            config.encoding = Encoding::Euc;
            Yaskkserv2MakeDictionary::run_create_jisyo(&config, &compare_euc_jisyo_full_path)
                .unwrap();
        }
        {
            let mut config = config;
            config.encoding = Encoding::Utf8;
            Yaskkserv2MakeDictionary::run_create_jisyo(&config, &compare_utf8_jisyo_full_path)
                .unwrap();
        }
        self.compare_euc_jisyo_hash = Some(Self::compare_and_get_hash(
            &compare_euc_jisyo_full_path,
            &self.compare_euc_jisyo_hash,
        ));
        self.compare_utf8_jisyo_hash = Some(Self::compare_and_get_hash(
            &compare_utf8_jisyo_full_path,
            &self.compare_utf8_jisyo_hash,
        ));
        match dictionary_from_test_jisyo_encoding {
            Encoding::Euc => {
                Self::compare_and_create_dictionary(
                    encoding_table,
                    &dictionary_from_test_jisyo_full_path,
                    &dictionary_euc_from_compare_euc_jisyo_full_path,
                    &compare_euc_jisyo_full_path,
                    Encoding::Euc,
                );
                Self::compare_and_create_dictionary(
                    encoding_table,
                    &dictionary_from_test_jisyo_full_path,
                    &dictionary_euc_from_compare_utf8_jisyo_full_path,
                    &compare_utf8_jisyo_full_path,
                    Encoding::Euc,
                );
            }
            Encoding::Utf8 => {
                Self::compare_and_create_dictionary(
                    encoding_table,
                    &dictionary_from_test_jisyo_full_path,
                    &dictionary_utf8_from_compare_euc_jisyo_full_path,
                    &compare_euc_jisyo_full_path,
                    Encoding::Utf8,
                );
                Self::compare_and_create_dictionary(
                    encoding_table,
                    &dictionary_from_test_jisyo_full_path,
                    &dictionary_utf8_from_compare_utf8_jisyo_full_path,
                    &compare_utf8_jisyo_full_path,
                    Encoding::Utf8,
                );
            }
        }
    }

    fn run_jisyo_lf_crlf_cr(&mut self) {
        let lf_crlf_cr_table = vec![("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")];
        for lf_crlf_cr in lf_crlf_cr_table {
            let (mut writer, jisyo_entries) =
                self.get_writer_and_jisyo_entries_no_encoding_conversion(Some("SKK-JISYO.L"));
            for entry in &jisyo_entries {
                let mut new_line_entry = Vec::new();
                new_line_entry.extend_from_slice(&entry[..entry.len()]);
                new_line_entry.extend_from_slice(lf_crlf_cr.1.as_bytes());
                writer.write_all(&new_line_entry).unwrap();
            }
            writer.flush().unwrap();
            self.convert_and_verify(Encoding::Euc);
        }
    }

    fn run_jisyo_random_lf_crlf_cr(&mut self) {
        let mut rng = rand::thread_rng();
        for _ in 0..20 {
            let (mut writer, jisyo_entries) =
                self.get_writer_and_jisyo_entries_no_encoding_conversion(Some("SKK-JISYO.L"));
            for entry in &jisyo_entries {
                let mut random_new_line_entry = Vec::new();
                random_new_line_entry.extend_from_slice(&entry[..entry.len()]);
                random_new_line_entry.extend_from_slice(match rng.gen_range(0, 3) {
                    0 => b"\n",
                    1 => b"\r\n",
                    _ => b"\r",
                });
                writer.write_all(&random_new_line_entry).unwrap();
            }
            writer.flush().unwrap();
            self.convert_and_verify(Encoding::Euc);
        }
    }

    fn run_jisyo_dynamic_random_lf_crlf_cr(&mut self) {
        let (mut writer, _jisyo_entries) =
            self.get_writer_and_jisyo_entries_no_encoding_conversion(None);
        let mut rng = rand::thread_rng();
        for _ in 0..20_000_000 {
            let mut random_new_line_entry = Vec::new();
            random_new_line_entry.extend_from_slice(b"a /a/");
            random_new_line_entry.extend_from_slice(match rng.gen_range(0, 3) {
                0 => b"\n",
                1 => b"\r\n",
                _ => b"\r",
            });
            writer.write_all(&random_new_line_entry).unwrap();
        }
        writer.flush().unwrap();
        self.convert_and_verify(Encoding::Euc);
    }

    fn run_jisyo_huge_dictionary(&mut self) {
        {
            let (mut writer, jisyo_entries) = self
                .get_writer_and_jisyo_entries_no_encoding_conversion(Some(
                    "yaskkserv2_test_unix.dictionary.jisyo.euc",
                ));
            for entry in &jisyo_entries {
                writer.write_all(entry).unwrap();
                writer.write_all(b"\n").unwrap();
            }
            writer.flush().unwrap();
            self.convert_and_verify(Encoding::Euc);
            self.convert_and_verify(Encoding::Utf8);
        }
        {
            let (mut writer, jisyo_entries) = self
                .get_writer_and_jisyo_entries_no_encoding_conversion(Some(
                    "yaskkserv2_test_unix.dictionary.jisyo.utf8",
                ));
            for entry in &jisyo_entries {
                writer.write_all(entry).unwrap();
                writer.write_all(b"\n").unwrap();
            }
            writer.flush().unwrap();
            self.convert_and_verify(Encoding::Euc);
            // huge dictionary は euc <-> utf8 変換できない文字を含み、 utf8 -> euc -> utf8 で
            // 失敗するため self.convert_and_verify(Encoding::Utf8) を呼んでいないことに注意。
        }
    }

    /// 辞書の euc/utf8 変換が正しく動作するか test する
    ///
    /// 1. `jisyo` -> `dictionary` (euc or utf8) -> jisyo.euc/jisyo.utf8 と変換
    /// 2. `jisyo.utf8` -> `jisyo.utf8_euc` のように utf8 を euc に変換
    /// 3. `jisyo.euc` と `jisyo.utf8_euc` を比較
    ///
    /// SKK-JISYO.L などの文字コード変換に対して無難な辞書を使用する必要があることに注意。
    fn run_euc_utf8_test(jisyo_base_filename: &str, dictionary_encoding: Encoding) {
        let skk_jisyo_full_path = Path::get_full_path(jisyo_base_filename);
        let encoding_str = &dictionary_encoding.to_string().to_lowercase();
        let output_dictionary_full_path = Path::get_full_path(&format!(
            "{}.jisyo_euc_utf8_dictionary_test.dictionary.{}",
            jisyo_base_filename, encoding_str
        ));
        let config = Config::new()
            .encoding(dictionary_encoding)
            .dictionary_full_path(output_dictionary_full_path);
        Yaskkserv2MakeDictionary::run_create_dictionary(
            &config,
            encoding_simple::EncodingTable::get(),
            &[skk_jisyo_full_path],
        )
        .unwrap();
        let euc_jisyo_full_path = Path::get_full_path(&format!(
            "{}.jisyo_euc_utf8_dictionary_test.dictionary.{}.jisyo.euc",
            jisyo_base_filename, encoding_str,
        ));
        {
            let mut config = config.clone();
            config.encoding = Encoding::Euc;
            Yaskkserv2MakeDictionary::run_create_jisyo(&config, &euc_jisyo_full_path).unwrap();
        }
        let utf8_jisyo_full_path = Path::get_full_path(&format!(
            "{}.jisyo_euc_utf8_dictionary_test.dictionary.{}.jisyo.utf8",
            jisyo_base_filename, encoding_str,
        ));
        {
            let mut config = config;
            config.encoding = Encoding::Utf8;
            Yaskkserv2MakeDictionary::run_create_jisyo(&config, &utf8_jisyo_full_path).unwrap();
        }
        let euc_map: FxHashMap<_, _> =
            read_jisyo_entries_no_encoding_conversion(&euc_jisyo_full_path)
                .iter()
                .map(|v| {
                    let (midashi, candidates) = Dictionary::get_midashi_candidates(v).unwrap();
                    (midashi.to_vec(), candidates.to_vec())
                })
                .collect();
        let utf8_to_euc_map: FxHashMap<_, _> =
            read_jisyo_entries_no_encoding_conversion(&utf8_jisyo_full_path)
                .iter()
                .map(|v| {
                    let (midashi, candidates) = Dictionary::get_midashi_candidates(v).unwrap();
                    (
                        encoding_simple::Euc::encode(midashi).unwrap(),
                        encoding_simple::Euc::encode(candidates).unwrap(),
                    )
                })
                .collect();
        // map が大きいため assert_eq! では失敗時の出力が長いので、あえて asset! を使用している
        // ことに注意
        assert!(euc_map == utf8_to_euc_map);
    }

    /// `JisyoReader` とは異なる方法で `jisyo_entries` を読み込み `JisyoReader` と比較 test する
    ///
    /// `JisyoReader` が想定外の動作をしていないかを調べる test 。
    fn run_read_test(encoding: Encoding) {
        const YASKKSERV2_JISYO_MINIMUM_ENTRIES: usize = 500_000;
        let jisyo_full_path = &Path::get_full_path_yaskkserv2_jisyo(encoding);
        let jisyo_entries = read_jisyo_entries_no_encoding_conversion(jisyo_full_path);
        let mut reader = File::open(jisyo_full_path).unwrap();
        let mut simple_jisyo_entries = Self::read_simple_jisyo_entries(&mut reader);
        simple_jisyo_entries.sort();
        assert!(jisyo_entries.len() > YASKKSERV2_JISYO_MINIMUM_ENTRIES);
        assert!(simple_jisyo_entries.len() > YASKKSERV2_JISYO_MINIMUM_ENTRIES);
        // 失敗時に表示が膨大になるため assert_eq!() を使わないことに注意
        assert!(jisyo_entries == simple_jisyo_entries);
    }

    /// `JisyoReader` とは異なる方法でシンプルに `jisyo_entries` を読み込む
    ///
    /// `JisyoReader` とは異なり、下記のように簡易的な実装であることに注意。
    ///
    /// - utf8/EUC 双方の SKK 辞書を生のまま扱うため binary を \n で分割しているだけ
    /// - つまり改行コードは LF のみ対応 (CRLF や CR が含まれる場合は `assert!()` で停止)
    /// - illegal line の判定も簡易的なもの
    fn read_simple_jisyo_entries(reader: &mut File) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        let mut buffer = Vec::new();
        let mut reader = std::io::BufReader::new(reader);
        while match reader.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => false,
            Ok(_size) => {
                assert!(
                    twoway::find_bytes(&buffer, b"\r").is_none(),
                    "UNSUPPORTED NEW LINE"
                );
                if buffer[0] == b';' {
                    // comment
                } else if buffer[0] == b' '
                    || buffer[0] == b'\t'
                    || buffer.len() < JISYO_MINIMUM_LINE_LENGTH
                    || buffer[buffer.len() - 1 - 1] != b'/'
                {
                    println!("LINE SKIPPED! {:?}", buffer);
                } else {
                    result.push(buffer[..buffer.len() - 1].to_vec());
                }
                buffer.clear();
                true
            }
        } {}
        result
    }
}

struct EucUtf8OkuriAriNashi;

impl EucUtf8OkuriAriNashi {
    fn create_source_jisyo(name: &str, encoding: Encoding, bytes: &[u8]) -> String {
        let encoding_string = encoding.to_string();
        let source_jisyo_full_path =
            Path::get_full_path(&format!("test_jisyo.{}.{}.jisyo", name, &encoding_string));
        let mut writer = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&source_jisyo_full_path)
                .unwrap(),
        );
        writer.write_all(bytes).unwrap();
        writer.flush().unwrap();
        source_jisyo_full_path
    }

    fn compare(encoding_table: &[u8], dictionary_encoding: Encoding, source_jisyo_full_path: &str) {
        let dictionary_full_path = format!(
            "{}.{}.dictionary",
            source_jisyo_full_path,
            dictionary_encoding.to_string()
        );
        let euc_jisyo_full_path = format!("{}.euc.jisyo", &dictionary_full_path);
        let utf8_jisyo_full_path = format!("{}.utf8.jisyo", &dictionary_full_path);
        let config = Config::new()
            .encoding(dictionary_encoding)
            .dictionary_full_path(dictionary_full_path);
        Yaskkserv2MakeDictionary::run_create_dictionary(
            &config,
            encoding_table,
            &[String::from(source_jisyo_full_path)],
        )
        .unwrap();
        Yaskkserv2MakeDictionary::run_create_jisyo(
            &config.clone().encoding(Encoding::Euc),
            &euc_jisyo_full_path,
        )
        .unwrap();
        let mut euc_jisyo_buffer = Vec::new();
        {
            let mut reader = File::open(&euc_jisyo_full_path).unwrap();
            reader.read_to_end(&mut euc_jisyo_buffer).unwrap();
        }
        Yaskkserv2MakeDictionary::run_create_jisyo(
            &config.encoding(Encoding::Utf8),
            &utf8_jisyo_full_path,
        )
        .unwrap();
        let mut utf8_jisyo_buffer = Vec::new();
        {
            let mut reader = File::open(&utf8_jisyo_full_path).unwrap();
            reader.read_to_end(&mut utf8_jisyo_buffer).unwrap();
        }
        // 先頭行が異なるので skip(1) していることに注意
        let euc_jisyo_bytes = euc_jisyo_buffer
            .split(|v| *v == b'\n')
            .into_iter()
            .skip(1)
            .map(|v| v.to_vec())
            .collect::<Vec<Vec<u8>>>();
        let utf8_jisyo_bytes = utf8_jisyo_buffer
            .split(|v| *v == b'\n')
            .into_iter()
            .skip(1)
            .map(|v| encoding_simple::Euc::encode(v).unwrap())
            .collect::<Vec<Vec<u8>>>();
        assert_eq!(euc_jisyo_bytes, utf8_jisyo_bytes,);
    }

    fn run(name: &str, source_jisyo_encoding: Encoding, source_jisyo_bytes: &[u8]) {
        let source_jisyo_full_path =
            Self::create_source_jisyo(name, source_jisyo_encoding, source_jisyo_bytes);
        let encoding_table = encoding_simple::EncodingTable::get();
        Self::compare(encoding_table, Encoding::Euc, &source_jisyo_full_path);
        Self::compare(encoding_table, Encoding::Utf8, &source_jisyo_full_path);
    }
}

#[test]
fn jisyo_lf_crlf_cr_test() {
    let name = "jisyo_lf_crlf_cr";
    setup::setup_and_wait(name);
    let mut jisyo = Jisyo::new(name);
    jisyo.run_jisyo_lf_crlf_cr();
    setup::exit();
}

#[test]
fn jisyo_random_lf_crlf_cr_test() {
    let name = "jisyo_random_lf_crlf_cr";
    setup::setup_and_wait(name);
    let mut jisyo = Jisyo::new(name);
    jisyo.run_jisyo_random_lf_crlf_cr();
    setup::exit();
}

#[test]
fn jisyo_dynamic_random_lf_crlf_cr_test() {
    let name = "jisyo_dynamic_random_lf_crlf_cr";
    setup::setup_and_wait(name);
    let mut jisyo = Jisyo::new(name);
    jisyo.run_jisyo_dynamic_random_lf_crlf_cr();
    setup::exit();
}

#[test]
fn jisyo_huge_dictionary_test() {
    let name = "jisyo_huge_dictionary";
    setup::setup_and_wait(name);
    let mut jisyo = Jisyo::new(name);
    jisyo.run_jisyo_huge_dictionary();
    setup::exit();
}

#[test]
fn jisyo_euc_utf8_test() {
    const SKK_JISYO_BASE_NAME: &str = "SKK-JISYO.L";
    let name = "jisyo_euc_utf8";
    setup::setup_and_wait(<&str>::clone(&name));
    let _jisyo = Jisyo::new(name);
    Jisyo::run_euc_utf8_test(SKK_JISYO_BASE_NAME, Encoding::Euc);
    Jisyo::run_euc_utf8_test(SKK_JISYO_BASE_NAME, Encoding::Utf8);
    setup::exit();
}

#[test]
fn jisyo_read_test() {
    let name = "jisyo_read";
    setup::setup_and_wait(<&str>::clone(&name));
    Jisyo::run_read_test(Encoding::Euc);
    Jisyo::run_read_test(Encoding::Utf8);
    setup::exit();
}

/// 辞書の euc/utf8 出力で okuri-ari/okuri-nashi 判定が正しく動作しているか test する
///
/// 「ー」など euc/utf8 で sort 結果が異なるような文字を考慮する必要があるため、埋め込みの
/// 簡単な辞書を使用していることに注意。
#[test]
fn jisyo_euc_utf8_okuri_ari_nashi_test() {
    let name = "jisyo_euc_utf8_okuri_ari_nashi";
    setup::setup_and_wait(<&str>::clone(&name));
    EucUtf8OkuriAriNashi::run(
        name,
        Encoding::Euc,
        b"\xa4\xa2 /OkuriNashiA/\n\xa4\xa2s /OkuriAri\xa4\xa2s/\n",
    );
    EucUtf8OkuriAriNashi::run(
        name,
        Encoding::Utf8,
        b"\xe3\x81\x82 /OkuriNashiA/\n\xe3\x81\x82s /OkuriAri\xe3\x81\x82s/\n",
    );
    setup::exit();
}
