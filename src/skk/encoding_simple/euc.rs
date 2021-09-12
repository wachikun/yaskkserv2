use rustc_hash::FxHashMap;
use std::sync::RwLockReadGuard;

use crate::skk::encoding_simple::{
    Decoder, Encoder, Euc, SkkError, Utility, COMBINE_EUC_TO_UTF8_MAP, COMBINE_UTF8_4_TO_EUC_MAP,
    COMBINE_UTF8_6_TO_EUC_MAP, EUC_2_TO_UTF8_VEC, EUC_3_TO_UTF8_MAP, UTF8_2_4_TO_EUC_MAP,
    UTF8_3_TO_EUC_VEC,
};

enum EucResult {
    Nop,
    Break,
    ErrorExit,
}

impl Euc {
    pub(crate) fn decode(euc_buffer: &[u8]) -> Result<Vec<u8>, SkkError> {
        if euc_buffer.is_empty() {
            return Ok(Vec::new());
        }
        let is_error_exit = false;
        let mut euc_i = 0;
        let mut result_utf8 = Vec::new();
        let euc_buffer_length = euc_buffer.len();
        let euc_2_to_utf8_vec = EUC_2_TO_UTF8_VEC.get();
        let mut euc_3_to_utf8_map = None;
        let mut combine_euc_to_utf8_map = None;
        // HashMap へのアクセスが遅いので注意。
        // できるだけ HashMap に触れる前に弾くこと。
        loop {
            match euc_buffer[euc_i] {
                0xa1..=0xfe | 0x8e => {
                    match Self::decode_a1_fe_8e(
                        is_error_exit,
                        euc_buffer,
                        euc_buffer_length,
                        euc_2_to_utf8_vec,
                        &mut combine_euc_to_utf8_map,
                        &mut result_utf8,
                        &mut euc_i,
                    ) {
                        EucResult::Break => break,
                        EucResult::ErrorExit => return Err(SkkError::Encoding),
                        EucResult::Nop => {}
                    }
                }
                0x00..=0x7f => {
                    result_utf8.push(euc_buffer[euc_i]);
                    euc_i += 1;
                }
                0x8f => {
                    match Self::decode_8f(
                        is_error_exit,
                        euc_buffer,
                        euc_buffer_length,
                        &mut euc_3_to_utf8_map,
                        &mut result_utf8,
                        &mut euc_i,
                    ) {
                        EucResult::Break => break,
                        EucResult::ErrorExit => return Err(SkkError::Encoding),
                        EucResult::Nop => {}
                    }
                }
                _ => {
                    result_utf8
                        .extend_from_slice(format!("&#x{:>02x}", euc_buffer[euc_i]).as_bytes());
                    euc_i += 1;
                }
            }
            #[cfg(feature = "assert_paranoia")]
            {
                assert!(euc_i <= euc_buffer_length);
            }
            if euc_i >= euc_buffer_length {
                break;
            }
        }
        Ok(result_utf8)
    }

    pub(crate) fn encode(utf8_buffer: &[u8]) -> Result<Vec<u8>, SkkError> {
        if utf8_buffer.is_empty() {
            return Ok(Vec::new());
        }
        let is_error_exit = false;
        let mut utf8_i = 0;
        let mut result_euc = Vec::new();
        let utf8_buffer_length = utf8_buffer.len();
        let utf8_3_to_euc_vec = UTF8_3_TO_EUC_VEC.get();
        let mut utf8_2_4_to_euc_map = None;
        let mut combine_utf8_4_to_euc_map = None;
        let mut combine_utf8_6_to_euc_map = None;
        // HashMap へのアクセスが遅いので注意。
        // できるだけ Vec や is_candidate_combine_*() などで HashMap に触れる前に弾くこと。
        loop {
            match utf8_buffer[utf8_i] {
                0xe0..=0xef
                    if Encoder::is_utf8_3_bytes(utf8_buffer, utf8_buffer_length, utf8_i) =>
                {
                    if let EucResult::ErrorExit = Self::encode_e0_ef(
                        is_error_exit,
                        utf8_buffer,
                        utf8_buffer_length,
                        utf8_3_to_euc_vec,
                        &mut combine_utf8_6_to_euc_map,
                        &mut result_euc,
                        &mut utf8_i,
                    ) {
                        return Err(SkkError::Encoding);
                    }
                }
                0..=0x7f => {
                    result_euc.push(utf8_buffer[utf8_i]);
                    utf8_i += 1;
                }
                0xc2..=0xdf
                    if Encoder::is_utf8_2_bytes(utf8_buffer, utf8_buffer_length, utf8_i) =>
                {
                    if let EucResult::ErrorExit = Self::encode_c2_df(
                        is_error_exit,
                        utf8_buffer,
                        utf8_buffer_length,
                        &mut utf8_2_4_to_euc_map,
                        &mut combine_utf8_4_to_euc_map,
                        &mut result_euc,
                        &mut utf8_i,
                    ) {
                        return Err(SkkError::Encoding);
                    }
                }
                0xf0..=0xf7
                    if Encoder::is_utf8_4_bytes(utf8_buffer, utf8_buffer_length, utf8_i) =>
                {
                    if let EucResult::ErrorExit = Self::encode_f0_f7(
                        is_error_exit,
                        utf8_buffer,
                        &mut utf8_2_4_to_euc_map,
                        &mut result_euc,
                        &mut utf8_i,
                    ) {
                        return Err(SkkError::Encoding);
                    }
                }
                _ => {
                    result_euc
                        .extend_from_slice(format!("&#x{:>02x}", utf8_buffer[utf8_i]).as_bytes());
                    if is_error_exit {
                        return Err(SkkError::Encoding);
                    }
                    utf8_i += 1;
                }
            }
            assert!(utf8_i <= utf8_buffer_length);
            if utf8_i >= utf8_buffer_length {
                break;
            }
        }
        Ok(result_euc)
    }

    fn decode_a1_fe_8e(
        is_error_exit: bool,
        euc_buffer: &[u8],
        euc_buffer_length: usize,
        euc_2_to_utf8_vec: &[[u8; 4]],
        combine_euc_to_utf8_map: &mut Option<RwLockReadGuard<'_, FxHashMap<[u8; 3], [u8; 8]>>>,
        result_utf8: &mut Vec<u8>,
        euc_i: &mut usize,
    ) -> EucResult {
        if !Utility::is_enough_2_bytes(euc_buffer_length, *euc_i) {
            result_utf8.extend_from_slice(format!("&#x{:>02x}", euc_buffer[*euc_i]).as_bytes());
            if is_error_exit {
                return EucResult::ErrorExit;
            }
            return EucResult::Break;
        }
        let euc_2_index =
            Decoder::get_euc_2_to_utf8_vec_index(euc_buffer[*euc_i], euc_buffer[*euc_i + 1]);
        if Utility::contains_euc_2(euc_2_to_utf8_vec, euc_2_index) {
            Decoder::push_to_buffer_utf8(&euc_2_to_utf8_vec[euc_2_index], result_utf8);
        } else {
            let array_key_3_in_2 = [euc_buffer[*euc_i], euc_buffer[*euc_i + 1], 0];
            if combine_euc_to_utf8_map.is_none() {
                *combine_euc_to_utf8_map = Some(COMBINE_EUC_TO_UTF8_MAP.read().unwrap());
            }
            let combine_euc_to_utf8_map = combine_euc_to_utf8_map.as_ref().unwrap();
            if combine_euc_to_utf8_map.contains_key(&array_key_3_in_2) {
                let v = combine_euc_to_utf8_map[&array_key_3_in_2];
                Decoder::push_to_buffer_utf8(&v[..4], result_utf8);
                Decoder::push_to_buffer_utf8(&v[4..8], result_utf8);
            } else {
                result_utf8.extend_from_slice(
                    format!(
                        "&#x{:>02x}{:>02x}",
                        array_key_3_in_2[0], array_key_3_in_2[1]
                    )
                    .as_bytes(),
                );
                if is_error_exit {
                    return EucResult::ErrorExit;
                }
            }
        }
        *euc_i += 2;
        EucResult::Nop
    }

    fn decode_8f(
        is_error_exit: bool,
        euc_buffer: &[u8],
        euc_buffer_length: usize,
        euc_3_to_utf8_map: &mut Option<RwLockReadGuard<'_, FxHashMap<[u8; 3], [u8; 4]>>>,
        result_utf8: &mut Vec<u8>,
        euc_i: &mut usize,
    ) -> EucResult {
        if !Utility::is_enough_3_bytes(euc_buffer_length, *euc_i) {
            if *euc_i + 1 >= euc_buffer_length {
                result_utf8.extend_from_slice(format!("&#x{:>02x}", euc_buffer[*euc_i]).as_bytes());
            } else {
                result_utf8.extend_from_slice(
                    format!(
                        "&#x{:>02x}{:>02x}",
                        euc_buffer[*euc_i],
                        euc_buffer[*euc_i + 1]
                    )
                    .as_bytes(),
                );
            }
            if is_error_exit {
                return EucResult::ErrorExit;
            }
            return EucResult::Break;
        }
        let key_3 = &euc_buffer[*euc_i..*euc_i + 3];
        if euc_3_to_utf8_map.is_none() {
            *euc_3_to_utf8_map = Some(EUC_3_TO_UTF8_MAP.read().unwrap());
        }
        let euc_3_to_utf8_map = euc_3_to_utf8_map.as_ref().unwrap();
        if euc_3_to_utf8_map.contains_key(key_3) {
            Decoder::push_to_buffer_utf8(&euc_3_to_utf8_map[key_3], result_utf8);
        } else {
            result_utf8.extend_from_slice(
                format!("&#x{:>02x}{:>02x}{:>02x}", key_3[0], key_3[1], key_3[2]).as_bytes(),
            );
            if is_error_exit {
                return EucResult::ErrorExit;
            }
        }
        *euc_i += 3;
        EucResult::Nop
    }

    fn encode_e0_ef(
        is_error_exit: bool,
        utf8_buffer: &[u8],
        utf8_buffer_length: usize,
        utf8_3_to_euc_vec: &[[u8; 3]],
        combine_utf8_6_to_euc_map: &mut Option<RwLockReadGuard<'_, FxHashMap<[u8; 6], [u8; 3]>>>,
        result_euc: &mut Vec<u8>,
        utf8_i: &mut usize,
    ) -> EucResult {
        if Utility::is_enough_6_bytes(utf8_buffer_length, *utf8_i)
            && Encoder::is_candidate_combine_utf8_6(utf8_buffer, *utf8_i)
        {
            if combine_utf8_6_to_euc_map.is_none() {
                *combine_utf8_6_to_euc_map = Some(COMBINE_UTF8_6_TO_EUC_MAP.read().unwrap());
            }
            let tmp_combine_utf8_6_to_euc_map = combine_utf8_6_to_euc_map.as_ref().unwrap();
            if tmp_combine_utf8_6_to_euc_map.contains_key(&utf8_buffer[*utf8_i..*utf8_i + 6]) {
                Encoder::push_to_buffer_euc(
                    &tmp_combine_utf8_6_to_euc_map[&utf8_buffer[*utf8_i..*utf8_i + 6]],
                    result_euc,
                );
                *utf8_i += 3 + 3;
                return EucResult::Nop;
            }
        }
        Self::encode_e0_ef_encode_3(
            is_error_exit,
            utf8_buffer,
            utf8_3_to_euc_vec,
            result_euc,
            utf8_i,
        )
    }

    fn encode_e0_ef_encode_3(
        is_error_exit: bool,
        utf8_buffer: &[u8],
        utf8_3_to_euc_vec: &[[u8; 3]],
        result_euc: &mut Vec<u8>,
        utf8_i: &mut usize,
    ) -> EucResult {
        let utf8_3_index = Encoder::get_utf8_3_to_euc_vec_index(&utf8_buffer[*utf8_i..*utf8_i + 3]);
        if Utility::contains_utf8_3(utf8_3_to_euc_vec, utf8_3_index) {
            Encoder::push_to_buffer_euc(&utf8_3_to_euc_vec[utf8_3_index], result_euc);
        } else {
            result_euc.extend_from_slice(
                format!(
                    "&#x{:>02x}{:>02x}{:>02x}",
                    utf8_buffer[*utf8_i],
                    utf8_buffer[*utf8_i + 1],
                    utf8_buffer[*utf8_i + 2]
                )
                .as_bytes(),
            );
            if is_error_exit {
                return EucResult::ErrorExit;
            }
        }
        *utf8_i += 3;
        EucResult::Nop
    }

    fn encode_c2_df(
        is_error_exit: bool,
        utf8_buffer: &[u8],
        utf8_buffer_length: usize,
        utf8_2_4_to_euc_map: &mut Option<RwLockReadGuard<'_, FxHashMap<[u8; 4], [u8; 3]>>>,
        combine_utf8_4_to_euc_map: &mut Option<RwLockReadGuard<'_, FxHashMap<[u8; 4], [u8; 3]>>>,
        result_euc: &mut Vec<u8>,
        utf8_i: &mut usize,
    ) -> EucResult {
        if combine_utf8_4_to_euc_map.is_none() {
            *combine_utf8_4_to_euc_map = Some(COMBINE_UTF8_4_TO_EUC_MAP.read().unwrap());
        }
        if Utility::is_enough_4_bytes(utf8_buffer_length, *utf8_i)
            && Encoder::is_candidate_combine_utf8_4(utf8_buffer, *utf8_i)
            && combine_utf8_4_to_euc_map
                .as_ref()
                .unwrap()
                .contains_key(&utf8_buffer[*utf8_i..*utf8_i + 4])
        {
            Encoder::push_to_buffer_euc(
                &combine_utf8_4_to_euc_map.as_ref().unwrap()[&utf8_buffer[*utf8_i..*utf8_i + 4]],
                result_euc,
            );
            *utf8_i += 2 + 2;
        } else {
            let key = [utf8_buffer[*utf8_i], utf8_buffer[*utf8_i + 1], 0, 0];
            if utf8_2_4_to_euc_map.is_none() {
                *utf8_2_4_to_euc_map = Some(UTF8_2_4_TO_EUC_MAP.read().unwrap());
            }
            let utf8_2_4_to_euc_map = utf8_2_4_to_euc_map.as_ref().unwrap();
            if utf8_2_4_to_euc_map.contains_key(&key) {
                Encoder::push_to_buffer_euc(&utf8_2_4_to_euc_map[&key], result_euc);
            } else {
                result_euc
                    .extend_from_slice(format!("&#x{:>02x}{:>02x}", key[0], key[1]).as_bytes());
                if is_error_exit {
                    return EucResult::ErrorExit;
                }
            }
            *utf8_i += 2;
        }
        EucResult::Nop
    }

    fn encode_f0_f7(
        is_error_exit: bool,
        utf8_buffer: &[u8],
        utf8_2_4_to_euc_map: &mut Option<RwLockReadGuard<'_, FxHashMap<[u8; 4], [u8; 3]>>>,
        result_euc: &mut Vec<u8>,
        utf8_i: &mut usize,
    ) -> EucResult {
        let key = &utf8_buffer[*utf8_i..*utf8_i + 4];
        if utf8_2_4_to_euc_map.is_none() {
            *utf8_2_4_to_euc_map = Some(UTF8_2_4_TO_EUC_MAP.read().unwrap());
        }
        let tmp_utf8_2_4_to_euc_map = utf8_2_4_to_euc_map.as_ref().unwrap();
        if tmp_utf8_2_4_to_euc_map.contains_key(key) {
            Encoder::push_to_buffer_euc(&tmp_utf8_2_4_to_euc_map[key], result_euc);
        } else {
            result_euc.extend_from_slice(
                format!(
                    "&#x{:>02x}{:>02x}{:>02x}{:>02x}",
                    key[0], key[1], key[2], key[3]
                )
                .as_bytes(),
            );
            if is_error_exit {
                return EucResult::ErrorExit;
            }
        }
        *utf8_i += 4;
        EucResult::Nop
    }
}
