use crate::skk::yaskkserv2::*;

impl Request {
    fn is_okuri_midashi_tail(midashi_tail: &[u8]) -> bool {
        if midashi_tail.len() != 1 {
            return false;
        }
        (midashi_tail[0] >= b'a') && (midashi_tail[0] <= b'z')
    }

    fn json_str_to_bytes(json: &json::JsonValue) -> Option<&[u8]> {
        if let Some(s) = json.as_str() {
            Some(s.as_bytes())
        } else {
            None
        }
    }

    fn get_result(json: json::JsonValue, max_candidates_length: usize) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        match json.len() {
            2 => {
                if let Some(midashi_tail) = Request::json_str_to_bytes(&json[1][0]) {
                    for u_0 in json[0][1].members() {
                        if let Some(b_0) = Request::json_str_to_bytes(u_0) {
                            for u_1 in json[1][1].members() {
                                if let Some(b_1) = Request::json_str_to_bytes(u_1) {
                                    let mut v = Vec::from(b_0);
                                    if !Request::is_okuri_midashi_tail(midashi_tail) {
                                        v.extend_from_slice(b_1);
                                    }
                                    result.push(v);
                                }
                            }
                        }
                    }
                }
            }
            3 => {
                if let Some(midashi_tail) = Request::json_str_to_bytes(&json[2][0]) {
                    for u_0 in json[0][1].members() {
                        if let Some(b_0) = Request::json_str_to_bytes(u_0) {
                            for u_1 in json[1][1].members() {
                                if let Some(b_1) = Request::json_str_to_bytes(u_1) {
                                    for u_2 in json[2][1].members() {
                                        if let Some(b_2) = Request::json_str_to_bytes(u_2) {
                                            let mut v = Vec::from(b_0);
                                            v.extend_from_slice(b_1);
                                            if !Request::is_okuri_midashi_tail(midashi_tail) {
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
            4 => {
                if let Some(midashi_tail) = Request::json_str_to_bytes(&json[3][0]) {
                    for u_0 in json[0][1].members() {
                        if let Some(b_0) = Request::json_str_to_bytes(u_0) {
                            for u_1 in json[1][1].members() {
                                if let Some(b_1) = Request::json_str_to_bytes(u_1) {
                                    for u_2 in json[2][1].members() {
                                        if let Some(b_2) = Request::json_str_to_bytes(u_2) {
                                            for u_3 in json[3][1].members() {
                                                if let Some(b_3) = Request::json_str_to_bytes(u_3) {
                                                    let mut v = Vec::from(b_0);
                                                    v.extend_from_slice(b_1);
                                                    v.extend_from_slice(b_2);
                                                    if !Request::is_okuri_midashi_tail(midashi_tail)
                                                    {
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
            _ => {
                for u in json[0][1].members() {
                    if let Some(bytes) = Request::json_str_to_bytes(u) {
                        result.push(Vec::from(bytes));
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
        let mut result = Vec::new();
        if json.is_array() && json[0].is_array() && (json[0].len() >= 2) {
            result = Request::get_result(json, max_candidates_length);
        } else {
            Yaskkserv2::log_error(&format!("json error? json={:?}", json));
        }
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
            .or_else(|e| {
                Yaskkserv2::log_error(&format!("reqwest::Client::builder()  error={:?}", e));
                Err(e)
            })?;
        let mut response = client.get(url).send().or_else(|e| {
            Yaskkserv2::log_error(&format!("get()  error={:?}", e));
            Err(e)
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
