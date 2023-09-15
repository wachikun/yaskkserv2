use rustc_hash::FxHashSet;

use crate::skk::Candidates;

impl Candidates {
    #[allow(dead_code)]
    pub(in crate::skk) fn need_quote(source: &[u8]) -> bool {
        for u in source {
            match u {
                b'\r' | b'\n' | b'\\' | b'\"' | b';' | b'/' => return true,
                _ => {}
            }
        }
        false
    }

    pub(in crate::skk) fn quote_and_add_prefix(source: &[u8], prefix: Option<u8>) -> Vec<u8> {
        let mut result = Vec::new();
        if let Some(prefix) = prefix {
            result.push(prefix);
        }
        for u in source {
            match u {
                b'\r' | b'\n' => {}
                b'\\' => result.extend_from_slice(br"\\"),
                b'\"' => result.extend_from_slice(br#"\""#),
                b';' => result.extend_from_slice(br#"(concat "\073")"#),
                b'/' => result.extend_from_slice(br#"(concat "\057")"#),
                _ => result.push(*u),
            }
        }
        result
    }

    /// 先端と終端の `b'/'` を 1 つだけ trim する
    ///
    /// `trim_matches(b'/')` とは異なり、連続した `b'/'` が存在しても刈り取られるのは先端と終端の
    /// 1 つだけであることに注意。
    pub(in crate::skk) fn trim_one_slash(source: &[u8]) -> &[u8] {
        const SKIP_ZERO_INDEX: usize = 0;
        const SKIP_SLASH_INDEX: usize = 1;
        let mut end = source.len();
        if end > 1 && source[end - 1] == b'/' {
            end -= 1;
        } else if end == 0 {
            return source;
        }
        let start = if source[0] == b'/' {
            SKIP_SLASH_INDEX
        } else {
            SKIP_ZERO_INDEX
        };
        &source[start..end]
    }

    /// `trim_matches(b'/')` された candidates をマージする
    ///
    /// `b"aa/bbb/cc"` のように、引数の先端と終端に `b'/'` が含まれてはならないことに注意。
    ///
    /// かなり重い処理なのでタイトな部分では下記のように処理を分けて考える必要がある。
    ///
    /// 1. merge する必要が無い場合は呼ばない
    /// 2. annotate が無い場合は単純な merge
    /// 3. annotate がある場合は面倒な merge
    ///    (重いが annotate が含まれる candidates は多くない)
    ///
    /// merge する必要が無い場合、すなわち `base_trimmed_slash_candidates` が空ならば本メソッドを呼ばずに
    /// `new_trimmed_slash_candidates` で上書きしてやればよい。
    pub(in crate::skk) fn merge_trimmed_slash_candidates(
        base_trimmed_slash_candidates: &[u8],
        new_trimmed_slash_candidates: &[u8],
    ) -> Vec<u8> {
        #[cfg(feature = "assert_paranoia")]
        {
            if base_trimmed_slash_candidates.len() >= 3 {
                assert_ne!(*base_trimmed_slash_candidates.iter().next().unwrap(), b'/');
                assert_ne!(*base_trimmed_slash_candidates.last().unwrap(), b'/');
            }
            if new_trimmed_slash_candidates.len() >= 3 {
                assert_ne!(*new_trimmed_slash_candidates.iter().next().unwrap(), b'/');
                assert_ne!(*new_trimmed_slash_candidates.last().unwrap(), b'/');
            }
        }
        if base_trimmed_slash_candidates.iter().any(|&v| v == b';')
            || new_trimmed_slash_candidates.iter().any(|&v| v == b';')
        {
            // annotate が絡むので遅い
            Self::merge_annotated_trimmed_slash_candidates(
                base_trimmed_slash_candidates,
                new_trimmed_slash_candidates,
            )
        } else {
            // annotate が絡まないのでそこまで遅くはない
            let mut new_raw_and_add_flag: Vec<(&[u8], bool)> = new_trimmed_slash_candidates
                .split(|v| *v == b'/')
                .map(|v| (v, true))
                .collect();
            let mut result_vec = vec![b'/'];
            if !base_trimmed_slash_candidates.is_empty() {
                for base_unit in base_trimmed_slash_candidates.split(|v| *v == b'/') {
                    result_vec.extend_from_slice(base_unit);
                    result_vec.push(b'/');
                    if let Some(new_unit) =
                        new_raw_and_add_flag.iter_mut().find(|v| v.0 == base_unit)
                    {
                        new_unit.1 = false;
                    }
                }
            }
            for new_unit in new_raw_and_add_flag.iter().filter(|v| v.1) {
                result_vec.extend_from_slice(new_unit.0);
                result_vec.push(b'/');
            }
            result_vec
        }
    }

    pub(in crate::skk) fn remove_duplicates<
        T: std::hash::Hash + std::cmp::Eq + std::clone::Clone,
    >(
        candidates: &[T],
    ) -> Vec<T> {
        let mut duplicates_hash = FxHashSet::default();
        candidates
            .iter()
            .filter(|&v| duplicates_hash.insert(v))
            .cloned()
            .collect::<Vec<T>>()
    }

    /// `b'/'` で区切られた `candidates_bytes` から重複した candidate を取り除いた bytes を返す
    ///
    /// `b"/aa/bbb/cc/"` のように、引数の先端と終端に `b'/'` が含まれている必要があることに注意。
    pub(in crate::skk) fn remove_duplicates_bytes(candidates_bytes: &[u8]) -> Vec<u8> {
        #[cfg(feature = "assert_paranoia")]
        {
            assert!(candidates_bytes.len() >= 3);
            assert_eq!(*candidates_bytes.iter().next().unwrap(), b'/');
            assert_eq!(*candidates_bytes.last().unwrap(), b'/');
        }
        // 下記のように tricky な動作をするので注意
        //
        // b"/abc/def/"           : candidates_bytes
        //                          入力データの先頭と終端には / が必要
        // [[], [abc], [def], []] : splited_candidates
        //                          先頭と終端の / で [] が発生する
        // [[], [abc], [def]]     : Self::remove_duplicates(splited_candidates)
        //                        : [] が重複しているので終端の [] が削除されている
        // b"/abc/def"            : join("/")
        //                        : 先頭の [] が先頭の / になる
        // b"/abc/def/"           : result.push(b'/')
        let splited_candidates = candidates_bytes
            .split(|v| *v == b'/')
            .collect::<Vec<&[u8]>>();
        let mut result = Self::remove_duplicates(&splited_candidates).join(&b'/');
        result.push(b'/');
        result
    }

    #[allow(dead_code)]
    pub(in crate::skk) fn remove_duplicates_str(candidates_str: &str) -> String {
        #[cfg(feature = "assert_paranoia")]
        {
            assert!(candidates_str.chars().count() >= 3);
            assert_eq!(candidates_str.chars().next(), Some('/'));
            assert_eq!(candidates_str.chars().last(), Some('/'));
        }
        // remove_duplicates_bytes() と同様 tricky な動作をするので注意
        let splited_candidates = candidates_str.split('/').collect::<Vec<&str>>();
        let mut result = Self::remove_duplicates(&splited_candidates).join("/");
        result.push('/');
        result
    }

    /// annotate 付きの `trim_matches(b'/')` された candidates をマージする
    ///
    /// `b"aa/bbb/cc"` のように、引数の先端と終端に `b'/'` が含まれてはならないことに注意。
    ///
    /// 遅いので可能な限り呼び出しを避けること。
    fn merge_annotated_trimmed_slash_candidates(
        base_trimmed_slash_candidates: &[u8],
        new_trimmed_slash_candidates: &[u8],
    ) -> Vec<u8> {
        #[cfg(feature = "assert_paranoia")]
        {
            if base_trimmed_slash_candidates.len() >= 3 {
                assert_ne!(*base_trimmed_slash_candidates.iter().next().unwrap(), b'/');
                assert_ne!(*base_trimmed_slash_candidates.last().unwrap(), b'/');
            }
            if new_trimmed_slash_candidates.len() >= 3 {
                assert_ne!(*new_trimmed_slash_candidates.iter().next().unwrap(), b'/');
                assert_ne!(*new_trimmed_slash_candidates.last().unwrap(), b'/');
            }
        }
        // (raw, annotate_removed, add_flag)
        let mut new_raw_and_annotate_removed_and_add_flag = new_trimmed_slash_candidates
            .split(|v| *v == b'/')
            .map(|v| {
                v.iter()
                    .position(|t| *t == b';')
                    .map_or((v, v, true), |find| (v, &v[..find], true))
            })
            .collect::<Vec<(&[u8], &[u8], bool)>>();
        let mut result_vec: Vec<u8> = vec![b'/'];
        // (raw, annotate_removed)
        if !base_trimmed_slash_candidates.is_empty() {
            for base_unit in base_trimmed_slash_candidates
                .split(|v| *v == b'/')
                .map(|v| {
                    v.iter()
                        .position(|t| *t == b';')
                        .map_or((v, v), |find| (v, &v[..find]))
                })
            {
                if let Some(new_unit) = new_raw_and_annotate_removed_and_add_flag
                    .iter_mut()
                    .find(|v| v.1 == base_unit.1)
                {
                    new_unit.2 = false;
                    // annotate は base の物を優先
                    // annotate が base に無く new に annotate が存在する場合のみ置き換える
                    if !base_unit.0.iter().any(|&v| v == b';')
                        && new_unit.0.iter().any(|&v| v == b';')
                    {
                        result_vec.extend_from_slice(new_unit.0);
                    } else {
                        result_vec.extend_from_slice(base_unit.0);
                    }
                } else {
                    result_vec.extend_from_slice(base_unit.0);
                }
                result_vec.push(b'/');
            }
        }
        for new_unit in new_raw_and_annotate_removed_and_add_flag
            .iter()
            .filter(|v| v.2)
        {
            result_vec.extend_from_slice(new_unit.0);
            result_vec.push(b'/');
        }
        result_vec
    }
}
