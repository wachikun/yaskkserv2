//! dictionary benchmark
//!
//! feature benchmark を必要とするので下記コマンドで実行する
//!
//! ```sh
//! $ cargo +nightly bench --features="benchmark" dictionary --
//! ```
//!
//! test benchmark との違いは下記の通り。
//!
//! - network を介さないので特性を見たり性能の評価に向く
//! - 特徴が出やすい位置にある midashi だけを検索
//!
//! 各 test の len() next() last() は下記のように `--features="print_debug_for_benchmark"` を
//! 追加して `yaskkserv2_make_dictionary` を実行すると表示されるので、意味のありそうな単語を
//! そこから選択する。
//!
//! ```sh
//! $ cargo run --release --bin=yaskkserv2_make_dictionary --features="print_debug_for_benchmark" -- --dictionary-filename=/tmp/tmp.yaskkserv2 SKK-JISYO
//! ```
//!
//! その他、上では表示されないが経験的に candidates が多いことが知られているものなど、特徴的な
//! test 候補もいくつか存在することに注意。(`systemadministrator`, `utf8_kou`,
//! `utf8_nishirozannjityou`, `utf8_nishi` や `utf8_nishirozannjityou` など)

#[cfg(all(feature = "benchmark", test))]
mod benchmark {
    use std::fs::File;
    use std::io::Read;

    use crate::skk::test_unix::*;
    use crate::skk::yaskkserv2::*;
    use crate::skk::yaskkserv2_make_dictionary::*;
    use crate::skk::*;

    use test::Bencher;

    const LOOP: usize = 1000;

    fn setup() -> (DictionaryReader, Config, DictionaryFile) {
        let dictionary_full_path = Path::get_full_path(&format!("benchmark.dictionary",));
        let dictionary_created_directory = format!("{}.created", dictionary_full_path);
        let config = Config::new()
            .google_timing(GoogleTiming::Disable)
            .dictionary_full_path(dictionary_full_path);
        if !std::path::Path::new(&dictionary_created_directory).exists() {
            let mut reader = File::open(BINARY_ENCODING_TABLE_FILENAME).unwrap();
            let mut encoding_table = Vec::new();
            reader.read_to_end(&mut encoding_table).unwrap();
            let encoding_table = encoding_table;
            DictionaryCreator::create(
                config.clone(),
                &encoding_table,
                &[Path::get_full_path(
                    "yaskkserv2_test_unix.dictionary.jisyo.euc",
                )],
            )
            .unwrap();
            std::fs::create_dir_all(&dictionary_created_directory).unwrap();
        }
        const INITIAL_READ_BUFFER_LENGTH: usize = 8 * 1024;
        let dictionary_file = DictionaryFile::new(
            File::open(&config.dictionary_full_path).unwrap(),
            INITIAL_READ_BUFFER_LENGTH,
        );
        let mut dictionary_reader = DictionaryReader::new();
        const SHA1_READ_BUFFER_LENGTH: usize = 256 * 1024;
        dictionary_reader.setup(
            config.clone(),
            Dictionary::setup(SHA1_READ_BUFFER_LENGTH, &config.dictionary_full_path).unwrap(),
        );
        (dictionary_reader, config, dictionary_file)
    }

    // len() : 322
    // next() : Ok("0996415") length=579 [30, 39, 39, 36, 34, 31, 35]
    // last() : Ok("0") length=2014 [30]
    #[bench]
    fn bench_dictionary_0(b: &mut Bencher) {
        let name = "bench_dictionary_0";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, b"10 ")
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    // len() : 72
    // next() : Ok("cyclosporine") length=1558 [63, 79, 63, 6c, 6f, 73, 70, 6f, 72, 69, 6e, 65]
    // last() : Ok("c") length=1992 [63]
    // len() : 77
    // next() : Ok("systemdown") length=622 [73, 79, 73, 74, 65, 6d, 64, 6f, 77, 6e]
    // last() : Ok("s") length=2039 [73]
    #[bench]
    fn bench_dictionary_cyclosporine(b: &mut Bencher) {
        let name = "bench_dictionary_cyclosporine";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, b"1cyclosporine ")
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_c(b: &mut Bencher) {
        let name = "bench_dictionary_c";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, b"1c ")
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_systemdown(b: &mut Bencher) {
        let name = "bench_dictionary_systemdown";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, b"1systemdown ")
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_s(b: &mut Bencher) {
        let name = "bench_dictionary_s";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, b"1s ")
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    // len() : 359
    // next() : Ok("かんろじ") length=537 [a4, ab, a4, f3, a4, ed, a4, b8]
    // last() : Ok("か") length=2047 [a4, ab]
    #[bench]
    fn bench_dictionary_utf8_kanroji(b: &mut Bencher) {
        let name = "bench_dictionary_utf8_kanroji";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(
                        &mut dictionary_file,
                        &[0x31, 0xa4, 0xab, 0xa4, 0xf3, 0xa4, 0xed, 0xa4, 0xb8, 0x20],
                    )
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_utf8_ka(b: &mut Bencher) {
        let name = "bench_dictionary_utf8_ka";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, &[0x31, 0xa4, 0xab, 0x20])
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    // len() : 201
    // next() : Ok("こんらい") length=681 [a4, b3, a4, f3, a4, e9, a4, a4]
    // last() : Ok("こ") length=2003 [a4, b3]
    #[bench]
    fn bench_dictionary_utf8_konrai(b: &mut Bencher) {
        let name = "bench_dictionary_utf8_konrai";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(
                        &mut dictionary_file,
                        &[0x31, 0xa4, 0xb3, 0xa4, 0xf3, 0xa4, 0xe9, 0xa4, 0xa4, 0x20],
                    )
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_utf8_ko(b: &mut Bencher) {
        let name = "bench_dictionary_utf8_ko";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, &[0x31, 0xa4, 0xb3, 0x20])
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_systemadministrator(b: &mut Bencher) {
        let name = "bench_dictionary_systemadministrator";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, b"1systemadministrator ")
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_utf8_kou(b: &mut Bencher) {
        let name = "bench_dictionary_utf8_kou";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, &[0x31, 0xa4, 0xb3, 0xa4, 0xa6, 0x20])
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_utf8_nishirozannjityou(b: &mut Bencher) {
        let name = "bench_dictionary_utf8_nishirozannjityou";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(
                        &mut dictionary_file,
                        &[
                            0x31, 0xa4, 0xcb, 0xa4, 0xb7, 0xa4, 0xed, 0xa4, 0xb6, 0xa4, 0xf3, 0xa4,
                            0xb8, 0xa4, 0xc1, 0xa4, 0xe7, 0xa4, 0xa6, 0x20,
                        ],
                    )
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }

    #[bench]
    fn bench_dictionary_utf8_nishi(b: &mut Bencher) {
        let name = "bench_dictionary_utf8_nishi";
        setup::setup_and_wait(name);
        let (dictionary_reader, _config, mut dictionary_file) = setup();
        b.iter(|| {
            for _ in 0..LOOP {
                let result = dictionary_reader
                    .read_candidates(&mut dictionary_file, &[0x31, 0xa4, 0xcb, 0xa4, 0xb7, 0x20])
                    .unwrap();
                assert!(result.len() > 1);
            }
        });
        setup::exit();
    }
}
