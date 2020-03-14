use crate::skk::yaskkserv2::*;

const BINARY_SEARCH_THRESHOLD: usize = 30;

impl DictionaryReader {
    pub(in crate::skk) fn new() -> DictionaryReader {
        DictionaryReader {
            config: Config::new(),
            on_memory: OnMemory::new(),
            google_japanese_input_protocol: String::from("https"),
            google_suggest_protocol: String::from("https"),
        }
    }

    pub(in crate::skk) fn setup(&mut self, config: Config, on_memory: OnMemory) {
        self.config = config;
        self.on_memory = on_memory;
        if self.config.is_use_http {
            self.google_japanese_input_protocol = String::from("http");
            self.google_suggest_protocol = String::from("http");
        }
    }

    /// midashi_buffer にある midashi から対応する candidates を返す
    ///
    /// midashi_buffer は server に送られてくる b"1midashi ' のような形式。
    ///
    /// 戻り値は常に先頭に b'1' が付加されるため candidates が見付からなかった場合でも
    /// result.len() == 0 とはならないことに注意。見付からなかった場合の判定には
    /// Yaskkserv2::is_empty_candidates(&result) を使うこと。
    pub(in crate::skk) fn read_candidates(
        &self,
        dictionary_file: &mut DictionaryFile,
        midashi_buffer: &[u8],
    ) -> Result<Vec<u8>, SkkError> {
        let mut result = Vec::with_capacity(RESULT_VEC_CAPACITY);
        result.push(b'1');
        let midashi = Self::get_midashi(midashi_buffer);
        let dictionary_midashi_key = Dictionary::get_dictionary_midashi_key(&midashi_buffer[1..])?;
        if self.config.google_timing == GoogleTiming::First {
            // Google API など、外部要因エラーは無視して継続させることに注意
            let _ignore_error_and_continue = self.read_google_candidates(midashi, &mut result);
        }
        if let Some(block_information_vectors) =
            self.get_block_information_vectors(dictionary_midashi_key)
        {
            Self::read_dictionary_for_read_candidates(
                midashi,
                block_information_vectors,
                self.on_memory.dictionary_fixed_header.blocks_offset,
                dictionary_file,
                &mut result,
            )?;
        }
        if self.config.google_timing == GoogleTiming::Last
            || (self.config.google_timing == GoogleTiming::NotFound
                && Yaskkserv2::is_empty_candidates(&result))
        {
            let _ignore_error_and_continue = self.read_google_candidates(midashi, &mut result);
        }
        Ok(result)
    }

    /// midashi_buffer にある midashi から対応する midashi 群を返す
    ///
    /// midashi_buffer は server に送られてくる b"4midashi ' のような形式。
    ///
    /// read_candidates() と同様に戻り値の長さは 0 にならないため、見付からなかった場合の
    /// 判定には Yaskkserv2::is_empty_candidates(&result) を使うこと。
    pub(in crate::skk) fn read_abbrev(
        &self,
        dictionary_file: &mut DictionaryFile,
        midashi_buffer: &[u8],
    ) -> Result<Vec<u8>, SkkError> {
        let mut result = Vec::with_capacity(RESULT_VEC_CAPACITY);
        result.push(b'1');
        let midashi = Self::get_midashi(midashi_buffer);
        let dictionary_midashi_key = Dictionary::get_dictionary_midashi_key(&midashi_buffer[1..])?;
        if let Some(block_information_vectors) =
            self.get_block_information_vectors(dictionary_midashi_key)
        {
            Self::read_dictionary_for_read_abbrev(
                midashi,
                block_information_vectors,
                self.on_memory.dictionary_fixed_header.blocks_offset,
                dictionary_file,
                &mut result,
                self.config.max_server_completions as usize,
                Encoding::from_u32(self.on_memory.dictionary_fixed_header.encoding),
            )?;
        }
        if result.len() > 1 {
            result.push(b'/');
        }
        Ok(result)
    }

    pub(in crate::skk) fn is_okuri_ari(midashi: &[u8]) -> bool {
        const OKURI_ARI_MIDASHI_MINIMUM_LENGTH: usize = 2 + 1; // "あs"
        let length = midashi.len();
        if length < OKURI_ARI_MIDASHI_MINIMUM_LENGTH {
            return false;
        }
        let last_character_index = length - 1;
        #[cfg(feature = "assert_paranoia")]
        {
            const UTF8_OKURI_ARI_MIDASHI_MINIMUM_LENGTH: usize = 3 + 1; // "あs"
            if length >= UTF8_OKURI_ARI_MIDASHI_MINIMUM_LENGTH {
                match midashi[last_character_index] {
                    b'a'..=b'z' => match midashi[last_character_index - 2] {
                        0x81..=0x82 if midashi[last_character_index - 3] == 0xe3 => {
                            panic!("utf8 hiragana found");
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        match midashi[last_character_index] {
            b'a'..=b'z' => {}
            _ => return false,
        }
        match midashi[last_character_index - 1] {
            0xa1..=0xf3 => {}
            _ => return false,
        }
        midashi[last_character_index - 2] == 0xa4
    }

    fn get_block_information_vectors(
        &self,
        dictionary_midashi_key: DictionaryMidashiKey,
    ) -> Option<&[DictionaryBlockInformation]> {
        if let Some(index) = OnMemory::get_ascii_hiragana_vec_index(dictionary_midashi_key) {
            if self.on_memory.index_ascii_hiragana_vec[index].is_empty() {
                None
            } else {
                Some(&self.on_memory.index_ascii_hiragana_vec[index])
            }
        } else if self
            .on_memory
            .index_map
            .contains_key(&dictionary_midashi_key)
        {
            Some(&self.on_memory.index_map[&dictionary_midashi_key])
        } else {
            None
        }
    }

    fn read_google_candidates(&self, midashi: &[u8], result: &mut Vec<u8>) -> Result<(), SkkError> {
        let utf8_midashi = encoding_simple::Euc::decode(midashi).or_else(|e| {
            Yaskkserv2::log_error(&format!("{}", e));
            Err(e)
        })?;
        let cached_google_utf8_candidates = if self.config.is_use_google_cache {
            GoogleCache::get_candidates(&utf8_midashi)
        } else {
            Vec::new()
        };
        let google_utf8_candidates = if !cached_google_utf8_candidates.is_empty() {
            cached_google_utf8_candidates
        } else if self.config.is_enable_google_suggest {
            let mut tmp_candidates: Vec<Vec<u8>> = Request::request_google_japanese_input(
                &self.google_japanese_input_protocol,
                &utf8_midashi,
                self.config.google_timeout_milliseconds,
                self.config.google_max_candidates_length,
                self.config.google_insert_hiragana_only_candidate,
                self.config.google_insert_katakana_only_candidate,
                self.config.google_insert_hankaku_katakana_only_candidate,
            )
            .unwrap_or_default();
            tmp_candidates.extend(Request::request_google_suggest(
                &self.google_suggest_protocol,
                &utf8_midashi,
                self.config.google_timeout_milliseconds,
            ).unwrap_or_default());
            Candidates::remove_duplicates(&tmp_candidates)
        } else {
            let tmp_candidates = Request::request_google_japanese_input(
                &self.google_japanese_input_protocol,
                &utf8_midashi,
                self.config.google_timeout_milliseconds,
                self.config.google_max_candidates_length,
                self.config.google_insert_hiragana_only_candidate,
                self.config.google_insert_katakana_only_candidate,
                self.config.google_insert_hankaku_katakana_only_candidate,
            )?;
            Candidates::remove_duplicates(&tmp_candidates)
        };
        if google_utf8_candidates.is_empty() {
            return Err(SkkError::Request);
        }
        let mut new_result = Vec::with_capacity(RESULT_VEC_CAPACITY);
        {
            let base_candidates_bytes = Candidates::trim_one_slash(&result[1..]);
            let new_candidates_bytes_tmp = google_utf8_candidates
                .iter()
                .flat_map(|v| Candidates::quote_and_add_prefix(&v, Some(b'/')))
                .collect::<Vec<u8>>();
            new_result.push(b'1');
            let new_candidates_bytes = Candidates::trim_one_slash(&new_candidates_bytes_tmp);
            if Encoding::from_u32(self.on_memory.dictionary_fixed_header.encoding) == Encoding::Euc
            {
                match encoding_simple::Euc::encode(&new_candidates_bytes) {
                    Ok(encoded) => {
                        new_result.extend_from_slice(&Candidates::merge_trimmed_slash_candidates(
                            &base_candidates_bytes,
                            &encoded,
                        ));
                    }
                    Err(e) => {
                        Yaskkserv2::log_error(&format!("{}", e));
                        return Err(SkkError::Request);
                    }
                }
            } else {
                new_result.extend_from_slice(&Candidates::merge_trimmed_slash_candidates(
                    &base_candidates_bytes,
                    &new_candidates_bytes,
                ));
            }
        }
        *result = new_result;
        if self.config.is_use_google_cache {
            GoogleCache::write_candidates(
                &utf8_midashi,
                &google_utf8_candidates,
                &self.config.google_cache_full_path,
                self.config.google_cache_entries,
                self.config.google_cache_expire_seconds,
            )?;
        }
        Ok(())
    }

    /// dictionary_block_informations の探索 loop を開始するのに適した index を返す
    ///
    /// あくまでも loop 開始に適した index で、 index に目的の midashi が含まれるわけではない
    /// ことに注意。
    ///
    /// dictionary_block_informations は巨大な辞書で数百程度まで増えるため、
    /// BINARY_SEARCH_THRESHOLD を越える場合は binary search する。
    /// BINARY_SEARCH_THRESHOLD は network を介さず直接辞書探索する benchmark で時間を計測した
    /// ものを plot してそこそこ効果がありそうだった値。
    ///
    /// binary search した場合、返す index は最大で目的の 2 個手前を指すことがある。
    /// dictionary_block_informations が小さく binary search をしなかった場合は、最大で
    /// BINARY_SEARCH_THRESHOLD / 2 個手前を指す可能性がある (binary search しない方が離れる
    /// が、 binary search のコストがかからない)。
    fn get_block_informations_loop_start_index(
        midashi: &[u8],
        dictionary_block_informations: &[DictionaryBlockInformation],
    ) -> usize {
        const RETURN_ZERO_THRESHOLD: usize = 10;
        let dictionary_block_informations_length = dictionary_block_informations.len();
        if dictionary_block_informations_length < RETURN_ZERO_THRESHOLD {
            0
        } else if dictionary_block_informations_length < BINARY_SEARCH_THRESHOLD {
            let half_index = dictionary_block_informations_length / 2;
            if *midashi > dictionary_block_informations[half_index].midashi[..] {
                0
            } else {
                half_index
            }
        } else {
            // 通常の binary search ではなく、範囲に含まれる index のヒントを返すことに注意。
            let mut index = (dictionary_block_informations_length / 2) as i32;
            let mut diff = index / 2;
            let mut previous_direction = 0;
            let mut diff_zero_count = 0;
            loop {
                let mut direction = 0;
                if *midashi > dictionary_block_informations[index as usize].midashi[..] {
                    index -= diff;
                    if index < 0 {
                        index = 0;
                        break;
                    }
                } else {
                    direction = 1;
                    index += diff;
                    if index >= dictionary_block_informations_length as i32 {
                        index = (dictionary_block_informations_length - 1) as i32;
                        break;
                    }
                }
                diff /= 2;
                if diff == 0 {
                    diff = 1;
                    if diff_zero_count >= 1 && direction != previous_direction {
                        index = std::cmp::max(0, index - 1);
                        break;
                    }
                    diff_zero_count += 1;
                }
                previous_direction = direction;
            }
            let mut dictionary_block_informations_index = index as usize;
            while dictionary_block_informations_index < dictionary_block_informations_length {
                if dictionary_block_informations[dictionary_block_informations_index].midashi[..]
                    <= *midashi
                {
                    break;
                }
                dictionary_block_informations_index += 1;
            }
            #[cfg(feature = "assert_paranoia")]
            {
                let mut debug_counter = 0;
                for dictionary_block_information in &dictionary_block_informations[index as usize..]
                {
                    if dictionary_block_information.midashi[..] <= *midashi {
                        break;
                    }
                    debug_counter += 1;
                }
                assert!(debug_counter <= 2);
            }
            dictionary_block_informations_index
        }
    }

    fn get_block_informations_index(
        midashi: &[u8],
        dictionary_block_informations: &[DictionaryBlockInformation],
    ) -> usize {
        let loop_start_index =
            Self::get_block_informations_loop_start_index(midashi, dictionary_block_informations);
        let mut dictionary_block_informations_index = loop_start_index;
        for dictionary_block_information in &dictionary_block_informations[loop_start_index..] {
            if dictionary_block_information.midashi[..] <= *midashi {
                break;
            }
            dictionary_block_informations_index += 1;
        }
        dictionary_block_informations_index
    }

    fn get_midashi(buffer: &[u8]) -> &[u8] {
        #[cfg(feature = "assert_paranoia")]
        {
            assert!(buffer.len() >= PROTOCOL_MINIMUM_LENGTH);
            assert!(buffer[0] == b'1' || buffer[0] == b'4');
            assert_eq!(buffer[buffer.len() - 1], b' ');
        }
        &buffer[1..buffer.len() - 1]
    }

    fn read_dictionary_for_read_candidates(
        midashi: &[u8],
        dictionary_block_informations: &[DictionaryBlockInformation],
        blocks_offset: u32,
        dictionary_file: &mut DictionaryFile,
        result: &mut Vec<u8>,
    ) -> Result<(), SkkError> {
        let dictionary_block_informations_index =
            Self::get_block_informations_index(midashi, dictionary_block_informations);
        if dictionary_block_informations_index >= dictionary_block_informations.len() {
            return Ok(());
        }
        let unit = &dictionary_block_informations[dictionary_block_informations_index];
        let mut midashi_search_bytes = Vec::with_capacity(MIDASHI_VEC_CAPACITY);
        midashi_search_bytes.extend_from_slice(b"\n");
        midashi_search_bytes.extend_from_slice(midashi);
        midashi_search_bytes.extend_from_slice(b" /");
        let midashi_search_bytes = midashi_search_bytes;
        let buffer = dictionary_file.read(
            u64::from(blocks_offset) + u64::from(unit.offset),
            unit.length as usize,
        )?;
        if let Some(midashi_find) = twoway::find_bytes(&buffer, &midashi_search_bytes) {
            let candidates_start = midashi_find + midashi_search_bytes.len();
            if let Some(lf_find) = twoway::find_bytes(&buffer[candidates_start..], b"\n") {
                if Yaskkserv2::is_empty_candidates(&result) {
                    result.push(b'/');
                    result
                        .extend_from_slice(&buffer[candidates_start..(candidates_start + lf_find)]);
                } else {
                    let mut new_result = Vec::with_capacity(RESULT_VEC_CAPACITY);
                    {
                        let base_candidates_bytes = Candidates::trim_one_slash(&result[1..]);
                        let new_candidates_bytes = Candidates::trim_one_slash(
                            &buffer[candidates_start..(candidates_start + lf_find)],
                        );
                        new_result.push(b'1');
                        new_result = Candidates::merge_trimmed_slash_candidates(
                            base_candidates_bytes,
                            new_candidates_bytes,
                        );
                    }
                    *result = new_result;
                }
                return Ok(());
            } else {
                return Err(SkkError::BrokenDictionary);
            }
        }
        Ok(())
    }

    fn read_dictionary_for_read_abbrev(
        midashi: &[u8],
        dictionary_block_informations: &[DictionaryBlockInformation],
        blocks_offset: u32,
        dictionary_file: &mut DictionaryFile,
        result: &mut Vec<u8>,
        max_server_completions: usize,
        encoding: Encoding,
    ) -> Result<(), SkkError> {
        let mut dictionary_block_informations_index =
            Self::get_block_informations_index(midashi, dictionary_block_informations);
        // abbrev では midashi が entry に存在しないことがあるので、 vec を越えてしまった場合は
        // 最後の index (dictionary_block_informations は逆順に並んでいるので意味的には先頭)
        // に補正していることに注意。
        if dictionary_block_informations_index >= dictionary_block_informations.len() {
            dictionary_block_informations_index -= 1;
        }
        let mut midashi_search_bytes = Vec::with_capacity(MIDASHI_VEC_CAPACITY);
        midashi_search_bytes.extend_from_slice(b"\n");
        midashi_search_bytes.extend_from_slice(midashi);
        let midashi_search_bytes = midashi_search_bytes;
        let mut found_count = 0;
        'outer: loop {
            let unit = &dictionary_block_informations[dictionary_block_informations_index];
            if found_count > 0 && !unit.midashi.starts_with(midashi) {
                break 'outer;
            }
            let buffer = dictionary_file.read(
                u64::from(blocks_offset) + u64::from(unit.offset),
                unit.length as usize,
            )?;
            let mut offset = 0;
            'inner: loop {
                if let Some(midashi_find) =
                    twoway::find_bytes(&buffer[offset..], &midashi_search_bytes)
                {
                    const MIDASHI_SEARCH_TOP_LF_LENGTH: usize = 1;
                    let midashi_start = offset + midashi_find + MIDASHI_SEARCH_TOP_LF_LENGTH;
                    let space_start = offset + midashi_find + midashi_search_bytes.len();
                    if let Some(space_find) = twoway::find_bytes(&buffer[space_start..], b" ") {
                        offset = space_start + space_find;
                        if !Self::is_okuri_ari(&buffer[midashi_start..space_start + space_find]) {
                            result.extend_from_slice(&Candidates::quote_and_add_prefix(
                                &buffer[midashi_start..(space_start + space_find)],
                                Some(b'/'),
                            ));
                            found_count += 1;
                            if found_count >= max_server_completions {
                                break 'outer;
                            }
                        }
                    } else {
                        return Err(SkkError::BrokenDictionary);
                    }
                } else if found_count == 0 {
                    break 'outer;
                } else {
                    break 'inner;
                }
            }
            if dictionary_block_informations_index == 0 {
                break 'outer;
            }
            dictionary_block_informations_index -= 1;
        }
        if encoding == Encoding::Utf8 {
            let tmp_utf8 = encoding_simple::Euc::decode(result).or_else(|e| {
                Yaskkserv2::log_error(&format!("{}", e));
                Err(e)
            })?;
            result.clear();
            result.extend_from_slice(&tmp_utf8);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(in crate::skk) mod test_unix {
    use rand::Rng;

    use crate::skk::yaskkserv2::dictionary_reader::*;

    fn get_random_ascii_vec(length: usize) -> Vec<u8> {
        let mut ascii_vec = Vec::new();
        let rand_length = rand::thread_rng().gen_range(1, length + 1);
        for _ in 0..rand_length {
            ascii_vec.push(rand::thread_rng().gen_range(b'0', b'9' + 1));
        }
        ascii_vec
    }

    /// get_block_informations_loop_start_index() の test
    ///
    /// get_block_informations_loop_start_index() は少し複雑なので、乱数とループ数であらゆる
    /// ケースの test をしている。
    #[test]
    fn yaskkserv2_dictionary_reader_get_block_informations_loop_start_index_test() {
        const TEST_LOOP: usize = 100000;
        for _ in 0..TEST_LOOP {
            let search_midashis = {
                let mut search_midashis = Vec::new();
                let search_rand_length = rand::thread_rng().gen_range(1, 100 + 1);
                for _ in 0..search_rand_length {
                    search_midashis.push(get_random_ascii_vec(10));
                }
                search_midashis
            };
            let dictionary_block_informations = {
                let mut dictionary_block_informations_midashis = Vec::new();
                {
                    let dictionary_block_informations_rand_length =
                        rand::thread_rng().gen_range(1, 100 + 1);
                    for _ in 0..dictionary_block_informations_rand_length {
                        dictionary_block_informations_midashis.push(get_random_ascii_vec(10));
                    }
                }
                dictionary_block_informations_midashis.sort();
                dictionary_block_informations_midashis.reverse();
                dictionary_block_informations_midashis
            }
            .iter()
            .map(|v| DictionaryBlockInformation {
                midashi: v.to_vec(),
                offset: 0,
                length: 0,
            })
            .collect::<Vec<DictionaryBlockInformation>>();
            // for u in &dictionary_block_informations {
            //     println!(
            //         "dictionary_block_informations  midashi={:?}",
            //         String::from_utf8(u.midashi.to_vec())
            //     );
            // }
            for midashi in &search_midashis {
                let loop_start_index = DictionaryReader::get_block_informations_loop_start_index(
                    &midashi[..],
                    &dictionary_block_informations,
                );
                let mut debug_counter = 0;
                for dictionary_block_information in
                    &dictionary_block_informations[loop_start_index..]
                {
                    if dictionary_block_information.midashi[..] <= midashi[..] {
                        break;
                    }
                    debug_counter += 1;
                }
                // println!(
                //     "loop_start_index={}  midashi={:?}  debug_counter={}  dictionary_block_informations.len()={}",
                //     loop_start_index,
                //     String::from_utf8(midashi.to_vec()),
                //     debug_counter,
                //     dictionary_block_informations.len()
                // );
                if dictionary_block_informations.len() < BINARY_SEARCH_THRESHOLD {
                    assert!(debug_counter <= BINARY_SEARCH_THRESHOLD / 2);
                } else {
                    assert!(debug_counter <= 2);
                }
                // println!(
                //     "midashi={:?}  loop_start_index={}",
                //     String::from_utf8(midashi.to_vec()),
                //     loop_start_index
                // );
            }
        }
    }
}
