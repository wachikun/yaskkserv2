use rand::Rng;
use rustc_hash::FxHashSet;
use std::fs::File;

use crate::skk::test_unix::setup;
use crate::skk::yaskkserv2_make_dictionary::JisyoReader;
use crate::skk::{Candidates, Encoding};

#[test]
fn quote_skk_jisyo_static_test() {
    let static_not_quote_table = [(r"abc", r"/abc"), (r"ABC", r"/ABC")];
    let static_quote_table = [
        ("a\rb\ncA\n\rB\r\nC\r\rX\n\nYZ", r"/abcABCXYZ"),
        (r"abc\ABC", r"/abc\\ABC"),
        (r#"abc"ABC"#, r#"/abc\"ABC"#),
        (r"abc;XYZ", r#"/abc(concat "\073")XYZ"#),
        (r"abc/XYZ", r#"/abc(concat "\057")XYZ"#),
    ];
    for unit in &static_not_quote_table {
        assert!(!Candidates::need_quote(unit.0.as_bytes()));
        assert_eq!(
            Candidates::quote_and_add_prefix(unit.0.as_bytes(), Some(b'/')),
            unit.1.as_bytes()
        );
    }
    for unit in &static_quote_table {
        assert!(Candidates::need_quote(unit.0.as_bytes()));
        assert_eq!(
            Candidates::quote_and_add_prefix(unit.0.as_bytes(), Some(b'/')),
            unit.1.as_bytes()
        );
    }
}

#[test]
fn quote_skk_jisyo_random_test() {
    let quote_chars = b"\r\n\\\";/";
    let quote_set: FxHashSet<u8> = quote_chars.iter().copied().collect();
    let mut rng = rand::thread_rng();
    let loop_limit = 10000;
    let buffer_length = 10000;
    for _ in 0..loop_limit {
        let mut buffer = Vec::new();
        let mut add_count = 0;
        loop {
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let rand_u8 = rng.gen_range(1..256) as u8;
            if !quote_set.contains(&rand_u8) {
                buffer.push(rand_u8);
                add_count += 1;
                if add_count > buffer_length {
                    break;
                }
            }
        }
        assert!(!Candidates::need_quote(&buffer));
        let mut cmp_data = vec![b'/'];
        cmp_data.extend_from_slice(&buffer);
        assert_eq!(
            &Candidates::quote_and_add_prefix(&buffer, Some(b'/')),
            &cmp_data
        );
    }
    for _ in 0..loop_limit {
        let mut buffer = Vec::new();
        let mut add_count = 0;
        loop {
            let index = rng.gen_range(0..quote_chars.len());
            buffer.push(quote_chars[index]);
            add_count += 1;
            if add_count > buffer_length {
                break;
            }
        }
        assert!(Candidates::need_quote(&buffer));
    }
}

#[test]
fn detect_encoding_test() {
    let name = "detect_encoding";
    setup::setup_and_wait(name);
    for full_path in &setup::JisyoDownloader::get_jisyo_full_paths(Encoding::Euc) {
        let mut reader = File::open(full_path).unwrap();
        let (encoding, _encoding_options) =
            JisyoReader::detect_jisyo_encoding(&mut reader).unwrap();
        println!("euc {full_path} {encoding:?}");
        assert_eq!(encoding, Encoding::Euc);
    }
    for full_path in &setup::JisyoDownloader::get_jisyo_full_paths(Encoding::Utf8) {
        let mut reader = File::open(full_path).unwrap();
        let (encoding, _encoding_options) =
            JisyoReader::detect_jisyo_encoding(&mut reader).unwrap();
        println!("utf8 {full_path} {encoding:?}");
        assert!(encoding == Encoding::Utf8);
    }
    setup::exit();
}

// compare_vec_* は remove_duplicates() とは異なるロジックで remove_duplicates() 相当の
// 処理をするために使用する。
#[test]
fn remove_duplicates_test() {
    const OUTER_LOOP_COUNT: usize = 50000;
    const BUFFER_LOOP: usize = 1000;
    let mut rng = rand::thread_rng();
    for _ in 0..OUTER_LOOP_COUNT {
        let mut compare_vec_buffer = Vec::new();
        let mut compare_vec_tmp = Vec::new();
        let mut buffer = vec![b'/'];
        let mut previous_u8 = b'/';
        for _ in 0..BUFFER_LOOP {
            let next_u8 = if previous_u8 != b'/' && rng.gen_range(0..10) < 3 {
                b'/'
            } else {
                const PRINTABLE_MIN_CODE: u8 = b' ';
                const PRINTABLE_MAX_CODE: u8 = b'~';
                rng.gen_range(PRINTABLE_MIN_CODE..=PRINTABLE_MAX_CODE)
            };
            if previous_u8 == b'/' && next_u8 == b'/' {
                const SAME_REPLACE_CODE: u8 = b'S';
                buffer.push(SAME_REPLACE_CODE);
                previous_u8 = SAME_REPLACE_CODE;
                compare_vec_tmp.push(SAME_REPLACE_CODE);
            } else {
                buffer.push(next_u8);
                previous_u8 = next_u8;
                compare_vec_tmp.push(next_u8);
                if next_u8 == b'/' {
                    compare_vec_buffer.push(Candidates::trim_one_slash(&compare_vec_tmp).to_vec());
                    compare_vec_tmp.clear();
                }
            }
        }
        if !compare_vec_tmp.is_empty() {
            compare_vec_buffer.push(Candidates::trim_one_slash(&compare_vec_tmp).to_vec());
        }
        if previous_u8 != b'/' {
            buffer.push(b'/');
        }
        let mut compare_vec_buffer_joined = vec![b'/'];
        compare_vec_buffer_joined
            .extend_from_slice(&Candidates::remove_duplicates(&compare_vec_buffer).join(&b'/'));
        compare_vec_buffer_joined.resize(compare_vec_buffer_joined.len() + 1, b'/');
        assert!(Candidates::remove_duplicates_bytes(&buffer) == compare_vec_buffer_joined);
        assert!(
            Candidates::remove_duplicates_bytes(&buffer)
                == Candidates::remove_duplicates_str(&String::from_utf8(buffer).unwrap())
                    .as_bytes()
        );
    }
}
