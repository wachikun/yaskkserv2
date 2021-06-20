use rand::Rng;

use crate::skk::encoding_simple;
use crate::skk::test_unix::{setup, BufRead, BufReader, File, Path};
use regex::Regex;

fn pack_euc(bytes: &[u8]) -> Vec<u8> {
    let mut packed = Vec::new();
    match bytes[0] {
        0xa1..=0xfe | 0x8e => packed.extend_from_slice(&bytes[..2]),
        0x00..=0x7f => packed.push(bytes[0]),
        0x8f => packed.extend_from_slice(&bytes[..3]),
        _ => panic!("unknown euc 0x{:02x}", bytes[0]),
    }
    packed
}

fn pack_utf8(bytes: &[u8]) -> Vec<u8> {
    let mut packed = Vec::new();
    match bytes[0] {
        0xe0..=0xef => packed.extend_from_slice(&bytes[..3]),
        0x00..=0x7f => packed.push(bytes[0]),
        0xc2..=0xdf => packed.extend_from_slice(&bytes[..2]),
        0xf0..=0xf7 => packed.extend_from_slice(&bytes[..4]),
        _ => panic!("unknown utf8 0x{:02x}", bytes[0]),
    }
    packed
}

#[test]
fn encoding_simple_test() {
    let name = "encoding_simple";
    setup::setup_and_wait(name);
    let empty_u8_vec: Vec<u8> = vec![];
    assert_eq!(encoding_simple::Euc::encode(b"").unwrap(), empty_u8_vec);
    assert_eq!(encoding_simple::Euc::decode(b"").unwrap(), empty_u8_vec);
    assert_eq!(encoding_simple::Euc::encode(b"a").unwrap(), b"a");
    assert_eq!(encoding_simple::Euc::decode(b"a").unwrap(), b"a");
    assert_eq!(encoding_simple::Euc::encode(b"ab").unwrap(), b"ab");
    assert_eq!(encoding_simple::Euc::decode(b"ab").unwrap(), b"ab");
    assert_eq!(
        encoding_simple::Euc::decode(&[0x8f, 0xfe]).unwrap(),
        b"&#x8ffe"
    );
    let mut random_bytes = Vec::new();
    for _ in 0..20000 {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        random_bytes.push(rand::thread_rng().gen_range(0, 0x100) as u8);
        let _droppable = encoding_simple::Euc::encode(&random_bytes).unwrap();
        let _droppable = encoding_simple::Euc::decode(&random_bytes).unwrap();
    }
    setup::exit();
}

// euc-jis-2004-std.txt に含まれる全ての文字が変換できるかどうか test する
//
// http://x0213.org/codetable/euc-jis-2004-std.txt を YASKKSERV2_TEST_DIRECTORY に置くことで
// テスト可能になる。このファイルは自動で download されないことに注意。
#[test]
fn encoding_all_test() {
    let name = "encoding_all";
    setup::setup_and_wait(name);
    let list_full_path = Path::get_full_path("euc-jis-2004-std.txt");
    if !std::path::Path::new(&list_full_path).exists() {
        println!("{} not found", list_full_path);
        setup::exit();
        return;
    }
    let re_line = Regex::new(r#"^(0x[^\s]+)\s+([^\s]+)\s+#"#).unwrap();
    let re_euc_ignore = Regex::new(r#"^0x[89][0-9A-F]"#).unwrap();
    for line in BufReader::new(File::open(&list_full_path).unwrap()).lines() {
        let line = line.unwrap();
        if let Some(m) = re_line.captures(&line) {
            let base_euc = &m[1];
            if re_euc_ignore.is_match(base_euc) {
                continue;
            }
            let base_unicode = &m[2];
            let euc_3 =
                encoding_simple::EncodingTable::convert_euc_code_to_euc_3_bytes(base_euc).unwrap();
            let mut is_combine = false;
            let utf8_8 = encoding_simple::EncodingTable::convert_unicode_code_to_utf8_8_bytes(
                base_unicode,
                &mut is_combine,
            )
            .unwrap();
            let mut packed_utf8 = Vec::new();
            packed_utf8.extend_from_slice(&pack_utf8(&utf8_8));
            if is_combine {
                packed_utf8.extend_from_slice(&pack_utf8(&utf8_8[4..]));
            }
            let mut packed_euc = Vec::new();
            packed_euc.extend_from_slice(&pack_euc(&euc_3));
            let decoded = encoding_simple::Euc::decode(&packed_euc).unwrap();
            let encoded = encoding_simple::Euc::encode(&packed_utf8).unwrap();
            assert_eq!(decoded, packed_utf8);
            assert_eq!(encoded, packed_euc);
        }
    }
    setup::exit();
}
