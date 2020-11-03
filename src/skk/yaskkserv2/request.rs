use regex::Regex;
use std::convert::TryInto;

use crate::skk::yaskkserv2::{
    Request, SkkError, Yaskkserv2, GOOGLE_JAPANESE_INPUT_URL, GOOGLE_SUGGEST_URL,
};

impl Request {
    const fn is_utf8_hiragana(letter: [u8; 3]) -> bool {
        if letter[0] != 0xe3 {
            return false;
        }
        if letter[1] == 0x81 {
            return letter[2] >= 0x81 && letter[2] <= 0xbf;
        }
        if letter[1] == 0x82 {
            if letter[2] >= 0x9b && letter[2] <= 0x9e {
                return true;
            }
            if letter[2] < 0x80 || letter[2] > 0x93 {
                return false;
            }
            true
        } else {
            false
        }
    }

    const fn is_utf8_katakana(letter: [u8; 3]) -> bool {
        if letter[0] != 0xe3 {
            return false;
        }
        if letter[1] == 0x82 {
            return letter[2] >= 0xa1 && letter[2] <= 0xbf;
        }
        if letter[1] == 0x83 {
            if letter[2] >= 0xbb && letter[2] <= 0xbe {
                return true;
            }
            if letter[2] < 0x80 || letter[2] > 0xb6 {
                return false;
            }
            true
        } else {
            false
        }
    }

    const fn is_utf8_hankaku_katakana(letter: [u8; 3]) -> bool {
        if letter[0] != 0xef {
            return false;
        }
        if letter[1] == 0xbd {
            return letter[2] >= 0xa1 && letter[2] <= 0xbf;
        }
        if letter[1] == 0xbe {
            if letter[2] < 0x80 || letter[2] > 0x9f {
                return false;
            }
            true
        } else {
            false
        }
    }

    fn is_utf8_hiragana_only(candidate: &[u8]) -> bool {
        let length = candidate.len();
        if length % 3 != 0 || length < 3 {
            return false;
        }
        for i in (0..length).step_by(3) {
            if !Self::is_utf8_hiragana(candidate[i..i + 3].try_into().unwrap()) {
                return false;
            }
        }
        true
    }

    fn is_utf8_katakana_only(candidate: &[u8]) -> bool {
        let length = candidate.len();
        if length % 3 != 0 || length < 3 {
            return false;
        }
        for i in (0..length).step_by(3) {
            if !Self::is_utf8_katakana(candidate[i..i + 3].try_into().unwrap()) {
                return false;
            }
        }
        true
    }

    fn is_utf8_hankaku_katakana_only(candidate: &[u8]) -> bool {
        let length = candidate.len();
        if length % 3 != 0 || length < 3 {
            return false;
        }
        for i in (0..length).step_by(3) {
            if !Self::is_utf8_hankaku_katakana(candidate[i..i + 3].try_into().unwrap()) {
                return false;
            }
        }
        true
    }

    const fn should_add_tail_candidates(midashi_tail: &[u8]) -> bool {
        let length = midashi_tail.len();
        if length < 1 {
            return false;
        }
        let tail_ascii = midashi_tail[length - 1];
        tail_ascii < b'a' || tail_ascii > b'z'
    }

    fn should_add(
        candidates: &[&[u8]],
        is_insert_hiragana_only_candidate: bool,
        is_insert_katakana_only_candidate: bool,
        is_insert_hankaku_katakana_only_candidate: bool,
    ) -> bool {
        for candidate in candidates {
            if !is_insert_hiragana_only_candidate && Self::is_utf8_hiragana_only(candidate) {
                return false;
            }
            if !is_insert_katakana_only_candidate && Self::is_utf8_katakana_only(candidate) {
                return false;
            }
            if !is_insert_hankaku_katakana_only_candidate
                && Self::is_utf8_hankaku_katakana_only(candidate)
            {
                return false;
            }
        }
        true
    }

    fn convert_json_str_to_bytes(json: &json::JsonValue) -> Option<&[u8]> {
        let s = json.as_str()?;
        Some(s.as_bytes())
    }

    fn get_google_japanese_input_result_2(
        json: &json::JsonValue,
        is_insert_hiragana_only_candidate: bool,
        is_insert_katakana_only_candidate: bool,
        is_insert_hankaku_katakana_only_candidate: bool,
    ) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        if let Some(midashi_tail) = Self::convert_json_str_to_bytes(&json[1][0]) {
            for u_0 in json[0][1].members() {
                if let Some(b_0) = Self::convert_json_str_to_bytes(u_0) {
                    for u_1 in json[1][1].members() {
                        if let Some(b_1) = Self::convert_json_str_to_bytes(u_1) {
                            if Self::should_add(
                                &[b_0, b_1],
                                is_insert_hiragana_only_candidate,
                                is_insert_katakana_only_candidate,
                                is_insert_hankaku_katakana_only_candidate,
                            ) {
                                let mut v = Vec::from(b_0);
                                if Self::should_add_tail_candidates(midashi_tail) {
                                    v.extend_from_slice(b_1);
                                }
                                result.push(v);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    fn get_google_japanese_input_result_3(
        json: &json::JsonValue,
        is_insert_hiragana_only_candidate: bool,
        is_insert_katakana_only_candidate: bool,
        is_insert_hankaku_katakana_only_candidate: bool,
    ) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        if let Some(midashi_tail) = Self::convert_json_str_to_bytes(&json[2][0]) {
            for u_0 in json[0][1].members() {
                if let Some(b_0) = Self::convert_json_str_to_bytes(u_0) {
                    for u_1 in json[1][1].members() {
                        if let Some(b_1) = Self::convert_json_str_to_bytes(u_1) {
                            for u_2 in json[2][1].members() {
                                if let Some(b_2) = Self::convert_json_str_to_bytes(u_2) {
                                    if Self::should_add(
                                        &[b_0, b_1, b_2],
                                        is_insert_hiragana_only_candidate,
                                        is_insert_katakana_only_candidate,
                                        is_insert_hankaku_katakana_only_candidate,
                                    ) {
                                        let mut v = Vec::from(b_0);
                                        v.extend_from_slice(b_1);
                                        if Self::should_add_tail_candidates(midashi_tail) {
                                            v.extend_from_slice(b_2);
                                        }
                                        result.push(v);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }

    fn get_google_japanese_input_result_4(
        json: &json::JsonValue,
        is_insert_hiragana_only_candidate: bool,
        is_insert_katakana_only_candidate: bool,
        is_insert_hankaku_katakana_only_candidate: bool,
    ) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        if let Some(midashi_tail) = Self::convert_json_str_to_bytes(&json[3][0]) {
            for u_0 in json[0][1].members() {
                if let Some(b_0) = Self::convert_json_str_to_bytes(u_0) {
                    for u_1 in json[1][1].members() {
                        if let Some(b_1) = Self::convert_json_str_to_bytes(u_1) {
                            for u_2 in json[2][1].members() {
                                if let Some(b_2) = Self::convert_json_str_to_bytes(u_2) {
                                    for u_3 in json[3][1].members() {
                                        if let Some(b_3) = Self::convert_json_str_to_bytes(u_3) {
                                            if Self::should_add(
                                                &[b_0, b_1, b_2, b_3],
                                                is_insert_hiragana_only_candidate,
                                                is_insert_katakana_only_candidate,
                                                is_insert_hankaku_katakana_only_candidate,
                                            ) {
                                                let mut v = Vec::from(b_0);
                                                v.extend_from_slice(b_1);
                                                v.extend_from_slice(b_2);
                                                if Self::should_add_tail_candidates(midashi_tail) {
                                                    v.extend_from_slice(b_3);
                                                }
                                                result.push(v);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }

    fn get_google_japanese_input_result(
        json: &json::JsonValue,
        max_candidates_length: usize,
        is_insert_hiragana_only_candidate: bool,
        is_insert_katakana_only_candidate: bool,
        is_insert_hankaku_katakana_only_candidate: bool,
    ) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        match json.len() {
            2 => result.extend_from_slice(&Self::get_google_japanese_input_result_2(
                json,
                is_insert_hiragana_only_candidate,
                is_insert_katakana_only_candidate,
                is_insert_hankaku_katakana_only_candidate,
            )),
            3 => result.extend_from_slice(&Self::get_google_japanese_input_result_3(
                json,
                is_insert_hiragana_only_candidate,
                is_insert_katakana_only_candidate,
                is_insert_hankaku_katakana_only_candidate,
            )),
            4 => result.extend_from_slice(&Self::get_google_japanese_input_result_4(
                json,
                is_insert_hiragana_only_candidate,
                is_insert_katakana_only_candidate,
                is_insert_hankaku_katakana_only_candidate,
            )),
            _ => {
                for u in json[0][1].members() {
                    if let Some(bytes) = Self::convert_json_str_to_bytes(u) {
                        if Self::should_add(
                            &[bytes],
                            is_insert_hiragana_only_candidate,
                            is_insert_katakana_only_candidate,
                            is_insert_hankaku_katakana_only_candidate,
                        ) {
                            result.push(Vec::from(bytes));
                        }
                    }
                }
            }
        }
        if result.len() > max_candidates_length {
            result[..max_candidates_length].to_vec()
        } else {
            result
        }
    }

    pub(in crate::skk) fn request_google_japanese_input(
        protocol: &str,
        midashi: &[u8],
        timeout: u64,
        max_candidates_length: usize,
        is_insert_hiragana_only_candidate: bool,
        is_insert_katakana_only_candidate: bool,
        is_insert_hankaku_katakana_only_candidate: bool,
    ) -> Result<Vec<Vec<u8>>, SkkError> {
        let encoded_midashi: String = url::form_urlencoded::byte_serialize(midashi).collect();
        let content = Self::request(
            &format!(
                "{}{}{}",
                protocol, GOOGLE_JAPANESE_INPUT_URL, encoded_midashi
            ),
            timeout,
        )?;
        let json = json::parse(&content)?;
        let result = if json.is_array() && json[0].is_array() && (json[0].len() >= 2) {
            Self::get_google_japanese_input_result(
                &json,
                max_candidates_length,
                is_insert_hiragana_only_candidate,
                is_insert_katakana_only_candidate,
                is_insert_hankaku_katakana_only_candidate,
            )
        } else {
            Yaskkserv2::log_error(&format!("json error? json={:?}", json));
            Vec::new()
        };
        if result.is_empty() {
            Err(SkkError::Request)
        } else {
            Ok(result)
        }
    }

    pub(in crate::skk) fn request_google_suggest(
        protocol: &str,
        midashi: &[u8],
        timeout: u64,
    ) -> Result<Vec<Vec<u8>>, SkkError> {
        let encoded_midashi: String = url::form_urlencoded::byte_serialize(midashi).collect();
        let mut result = Vec::new();
        let content = Self::request(
            &format!("{}{}{}", protocol, GOOGLE_SUGGEST_URL, encoded_midashi),
            timeout,
        )?;
        // FIXME!
        // suggest は google japanese input とは異なり JSON ではなく XML で返ってくる。
        // ここでは正式な XML parser ではなく簡易的に取得していることに注意。
        let re_space_after_trim = Regex::new(r"^([^\s]+)\s.+$").unwrap();
        for splited in content.split('<') {
            if splited.starts_with(r#"suggestion data=""#) {
                let mut trimmed = String::from(
                    splited
                        .trim_start_matches(r#"suggestion data=""#)
                        .trim_end_matches(r#""/>"#),
                );
                if let Some(m) = re_space_after_trim.captures(&trimmed) {
                    trimmed = String::from(&m[1]);
                }
                result.push(trimmed.as_bytes().to_vec());
            }
        }
        if result.is_empty() {
            Err(SkkError::Request)
        } else {
            Ok(result)
        }
    }

    fn request(url: &str, timeout: u64) -> Result<String, SkkError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout))
            .build()
            .map_err(|e| {
                Yaskkserv2::log_error(&format!("reqwest::Client::builder()  error={:?}", e));
                e
            })?;
        let mut response = client.get(url).send().map_err(|e| {
            Yaskkserv2::log_error(&format!("get()  error={:?}", e));
            e
        })?;
        let status = response.status();
        if status == reqwest::StatusCode::OK {
            Ok(response.text()?)
        } else {
            Yaskkserv2::log_error(&format!("status()  error={:?}", status));
            Err(SkkError::Request)
        }
    }
}

#[cfg(test)]
pub(in crate::skk) mod test_unix {
    use crate::skk::yaskkserv2::Request;
    use std::convert::TryInto;

    #[test]
    fn is_utf8_hiragana_test() {
        assert!(Request::is_utf8_hiragana(
            "ぁ".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_hiragana(
            "ん".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_hiragana(
            "゛".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_hiragana(
            "ゞ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hiragana(
            "ァ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hiragana(
            "ヶ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hiragana(
            "・".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hiragana(
            "ヾ".as_bytes().try_into().unwrap()
        ));
    }

    #[test]
    fn is_utf8_katakana_test() {
        assert!(Request::is_utf8_katakana(
            "ァ".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_katakana(
            "ヶ".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_katakana(
            "・".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_katakana(
            "ヾ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_katakana(
            "ぁ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_katakana(
            "ん".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_katakana(
            "゛".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_katakana(
            "ゞ".as_bytes().try_into().unwrap()
        ));
    }

    #[test]
    fn is_utf8_hankaku_katakana_test() {
        assert!(Request::is_utf8_hankaku_katakana(
            "｡".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_hankaku_katakana(
            "､".as_bytes().try_into().unwrap()
        ));
        assert!(Request::is_utf8_hankaku_katakana(
            "ﾟ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hankaku_katakana(
            "ヾ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hankaku_katakana(
            "ぁ".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hankaku_katakana(
            "ん".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hankaku_katakana(
            "゛".as_bytes().try_into().unwrap()
        ));
        assert!(!Request::is_utf8_hankaku_katakana(
            "ゞ".as_bytes().try_into().unwrap()
        ));
    }

    #[test]
    fn is_utf8_hiragana_only_test() {
        assert!(Request::is_utf8_hiragana_only("あ".as_bytes()));
        assert!(Request::is_utf8_hiragana_only("あん".as_bytes()));
        assert!(!Request::is_utf8_hiragana_only("あア".as_bytes()));
        assert!(!Request::is_utf8_hiragana_only("あｱ".as_bytes()));
        assert!(!Request::is_utf8_hiragana_only("あa".as_bytes()));
        assert!(!Request::is_utf8_hiragana_only("ア".as_bytes()));
        assert!(!Request::is_utf8_hiragana_only("ｱ".as_bytes()));
        assert!(!Request::is_utf8_hiragana_only("a".as_bytes()));
    }

    #[test]
    fn is_utf8_katakana_only_test() {
        assert!(Request::is_utf8_katakana_only("ア".as_bytes()));
        assert!(Request::is_utf8_katakana_only("アン".as_bytes()));
        assert!(!Request::is_utf8_katakana_only("あア".as_bytes()));
        assert!(!Request::is_utf8_katakana_only("あｱ".as_bytes()));
        assert!(!Request::is_utf8_katakana_only("あa".as_bytes()));
        assert!(!Request::is_utf8_katakana_only("あ".as_bytes()));
        assert!(!Request::is_utf8_katakana_only("ｱ".as_bytes()));
        assert!(!Request::is_utf8_katakana_only("a".as_bytes()));
    }

    #[test]
    fn is_utf8_hankaku_katakana_only_test() {
        assert!(Request::is_utf8_hankaku_katakana_only("｡｡｡".as_bytes()));
        assert!(Request::is_utf8_hankaku_katakana_only("ﾝﾝﾝ".as_bytes()));
        assert!(Request::is_utf8_hankaku_katakana_only("ｱ".as_bytes()));
        assert!(!Request::is_utf8_hankaku_katakana_only("ア".as_bytes()));
        assert!(!Request::is_utf8_hankaku_katakana_only("アン".as_bytes()));
        assert!(!Request::is_utf8_hankaku_katakana_only("あア".as_bytes()));
        assert!(!Request::is_utf8_hankaku_katakana_only("あｱ".as_bytes()));
        assert!(!Request::is_utf8_hankaku_katakana_only("あa".as_bytes()));
        assert!(!Request::is_utf8_hankaku_katakana_only("あ".as_bytes()));
        assert!(!Request::is_utf8_hankaku_katakana_only("a".as_bytes()));
    }
}
