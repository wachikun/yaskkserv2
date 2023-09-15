use crate::skk::encoding_simple::{
    BufRead, BufReader, Decoder, Encoder, EncodingTable, File, Regex, SkkError,
    COMBINE_EUC_TO_UTF8_MAP, COMBINE_UTF8_4_TO_EUC_MAP, COMBINE_UTF8_6_TO_EUC_MAP,
    EUC_2_TO_UTF8_VEC, EUC_3_TO_UTF8_MAP, UTF8_2_4_TO_EUC_MAP, UTF8_3_TO_EUC_VEC,
};

use std::fmt::Write as _;

type SetupMapResult = (Vec<[u8; 4]>, Vec<[u8; 3]>);

impl EncodingTable {
    #[allow(dead_code)]
    pub(crate) fn create_src_table(table_full_path: &str) -> Result<String, SkkError> {
        let header_version: u32 = 1;
        let header_length: u32 = 32;
        let mut header_euc_utf8_combine_length: u32 = 0;
        let mut header_euc_utf8_length: u32 = 0;
        let header_reserved: [u8; 4 * 4] = [0; 4 * 4];
        let mut euc_utf8_combine_table =
            String::from("// euc_utf8_combine\n// euc 3 bytes, utf8 4 bytes, utf8 4 bytes\n");
        let mut euc_utf8_table = String::from("// euc_utf8\n// euc 3 bytes, utf8 4 bytes\n");
        let re_comment = Regex::new(r"^\s*#").unwrap();
        let re_line = Regex::new(r"^([^\s]+)\s+([^\s]+)\s+#").unwrap();
        let mut line = String::new();
        let mut reader = BufReader::new(File::open(table_full_path)?);
        while reader.read_line(&mut line)? > 0 {
            if re_comment.is_match(&line) {
                line.clear();
                continue;
            }
            if let Some(m) = re_line.captures(&line) {
                let base_euc = &m[1];
                let base_unicode = &m[2];
                let euc_3 = Self::convert_euc_code_to_euc_3_bytes(base_euc)?;
                let mut is_combine = false;
                let utf8_8 =
                    Self::convert_unicode_code_to_utf8_8_bytes(base_unicode, &mut is_combine)?;
                if is_combine {
                    header_euc_utf8_combine_length += 1;
                    _ = write!(
                        euc_utf8_combine_table,
                        "0x{:>02x},0x{:>02x},0x{:>02x},",
                        euc_3[0], euc_3[1], euc_3[2]
                    );
                    _ = writeln!(
                        euc_utf8_combine_table,
                        "0x{:>02x},0x{:>02x},0x{:>02x},0x{:>02x},0x{:>02x},0x{:>02x},0x{:>02x},0x{:>02x},",
                        utf8_8[0], utf8_8[1], utf8_8[2], utf8_8[3], utf8_8[4], utf8_8[5], utf8_8[6], utf8_8[7]);
                } else {
                    header_euc_utf8_length += 1;
                    _ = write!(
                        euc_utf8_table,
                        "0x{:>02x},0x{:>02x},0x{:>02x},",
                        euc_3[0], euc_3[1], euc_3[2]
                    );
                    Self::write_4_bytes(&mut euc_utf8_table, &utf8_8, "");
                }
            }
            line.clear();
        }
        let mut result = String::from("// header\n");
        Self::write_4_bytes(&mut result, &header_version.to_ne_bytes(), "    // version");
        Self::write_4_bytes(
            &mut result,
            &header_length.to_ne_bytes(),
            "    // header_length",
        );
        Self::write_4_bytes(
            &mut result,
            &header_euc_utf8_combine_length.to_ne_bytes(),
            "    // euc_utf8_combine_length",
        );
        Self::write_4_bytes(
            &mut result,
            &header_euc_utf8_length.to_ne_bytes(),
            "    // euc_utf8_length",
        );
        Self::write_4_bytes(&mut result, &header_reserved[0..4], "    // reserved");
        Self::write_4_bytes(&mut result, &header_reserved[4..8], "    // reserved");
        Self::write_4_bytes(&mut result, &header_reserved[8..12], "    // reserved");
        Self::write_4_bytes(&mut result, &header_reserved[12..16], "    // reserved");
        result.push_str(&euc_utf8_combine_table);
        result.push_str(&euc_utf8_table);
        Ok(result)
    }

    pub(crate) fn setup(encoding_table: &[u8]) -> Result<(), SkkError> {
        const EUC_UTF8_COMBINE_UNIT_LENGTH: usize = 3 + 8;
        const EUC_UTF8_UNIT_LENGTH: usize = 3 + 4;
        let mut array_u32 = [0; 4];
        array_u32.copy_from_slice(&encoding_table[4..4 + 4]);
        let header_length = u32::from_ne_bytes(array_u32) as usize;
        array_u32.copy_from_slice(&encoding_table[8..8 + 4]);
        let header_euc_utf8_combine_length = u32::from_ne_bytes(array_u32) as usize;
        array_u32.copy_from_slice(&encoding_table[12..12 + 4]);
        let header_euc_utf8_length = u32::from_ne_bytes(array_u32) as usize;
        Self::setup_combine_map(
            encoding_table,
            EUC_UTF8_COMBINE_UNIT_LENGTH,
            header_length,
            header_euc_utf8_combine_length,
        )?;
        let (euc_2_to_utf8_vec, utf8_3_to_euc_vec) = Self::setup_map(
            encoding_table,
            EUC_UTF8_UNIT_LENGTH,
            EUC_UTF8_COMBINE_UNIT_LENGTH,
            header_length,
            header_euc_utf8_length,
            header_euc_utf8_combine_length,
        );
        *EUC_2_TO_UTF8_VEC.get_mut_for_setup() = euc_2_to_utf8_vec;
        *UTF8_3_TO_EUC_VEC.get_mut_for_setup() = utf8_3_to_euc_vec;
        Ok(())
    }

    /// `"U+????"` 形式の unicode code を utf8 binary に変換し 8 bytes の Vec に返す
    ///
    /// 返される Vec は通常前半の 4 bytes が使われるが、結合文字の場合は後半の 4 bytes
    /// も使われる。Vec へは下記のように格納される。
    ///
    ///    - `[A0, A1,  0,  0,  0,  0,  0,  0]  is_combine = false`
    ///    - `[A0, A1, A2,  0,  0,  0,  0,  0]  is_combine = false`
    ///
    ///    - `[A0, A1,  0,  0, B1, B1,  0,  0]  is_combine = true`
    ///    - `[A0, A1, A2,  0, B0, B1, B2,  0]  is_combine = true`
    ///
    /// 対応する `unicode_str` は下記 format のみ。
    ///
    /// A. `U+????       is_combine = false`
    /// B. `U+?????      is_combine = false`
    /// C. `U+????+????  is_combine = true`
    pub(crate) fn convert_unicode_code_to_utf8_8_bytes(
        unicode_code: &str,
        is_combine: &mut bool,
    ) -> Result<Vec<u8>, SkkError> {
        const A_LENGTH: usize = 2 + 4;
        const B_LENGTH: usize = 2 + 5;
        const C_LENGTH: usize = 2 + 4 + 1 + 4;
        #[cfg(feature = "assert_paranoia")]
        {
            assert!(unicode_code.starts_with("U+"));
            if unicode_code.len() == C_LENGTH {
                assert_eq!(unicode_code.chars().nth(6), Some('+'));
            }
        }
        let mut result_utf8_8 = vec![0; 8];
        match unicode_code.len() {
            A_LENGTH => {
                if let Ok(ok) = u32::from_str_radix(&unicode_code[2..6], 16) {
                    Self::convert_unicode_to_utf8(ok, &mut result_utf8_8[..4]);
                }
            }
            B_LENGTH => {
                if let Ok(ok) = u32::from_str_radix(&unicode_code[2..7], 16) {
                    Self::convert_unicode_to_utf8(ok, &mut result_utf8_8[..4]);
                }
            }
            C_LENGTH => {
                if let Ok(ok) = u32::from_str_radix(&unicode_code[2..6], 16) {
                    Self::convert_unicode_to_utf8(ok, &mut result_utf8_8[..4]);
                    if let Ok(ok) = u32::from_str_radix(&unicode_code[7..11], 16) {
                        Self::convert_unicode_to_utf8(ok, &mut result_utf8_8[4..4 + 4]);
                        *is_combine = true;
                    }
                }
            }
            _ => {
                return Err(SkkError::Encoding);
            }
        }
        Ok(result_utf8_8)
    }

    /// 0xNNNN 形式の euc code を euc binary に変換し 3 bytes の Vec に返す
    pub(crate) fn convert_euc_code_to_euc_3_bytes(euc_code: &str) -> Result<Vec<u8>, SkkError> {
        #[cfg(feature = "assert_paranoia")]
        {
            assert!(euc_code.starts_with("0x"));
        }
        const PREFIX_LENGTH: usize = 2;
        const DIGIT_2_LENGTH: usize = PREFIX_LENGTH + 2;
        const DIGIT_4_LENGTH: usize = PREFIX_LENGTH + 4;
        const DIGIT_6_LENGTH: usize = PREFIX_LENGTH + 6;
        let mut result_euc_3 = vec![0; 3];
        match euc_code.len() {
            DIGIT_2_LENGTH => {
                result_euc_3[0] = u8::from_str_radix(&euc_code[2..4], 16)?;
            }
            DIGIT_4_LENGTH => {
                result_euc_3[0] = u8::from_str_radix(&euc_code[2..4], 16)?;
                result_euc_3[1] = u8::from_str_radix(&euc_code[4..6], 16)?;
            }
            DIGIT_6_LENGTH => {
                result_euc_3[0] = u8::from_str_radix(&euc_code[2..4], 16)?;
                result_euc_3[1] = u8::from_str_radix(&euc_code[4..6], 16)?;
                result_euc_3[2] = u8::from_str_radix(&euc_code[6..8], 16)?;
            }
            _ => {
                return Err(SkkError::Encoding);
            }
        }
        Ok(result_euc_3)
    }

    fn setup_combine_map(
        encoding_table: &[u8],
        euc_utf8_combine_unit_length: usize,
        header_length: usize,
        header_euc_utf8_combine_length: usize,
    ) -> Result<(), SkkError> {
        let mut offset = header_length;
        for _ in 0..header_euc_utf8_combine_length {
            let mut euc_3 = [0; 3];
            let mut utf8_value = [0; 8];
            euc_3.copy_from_slice(&encoding_table[offset..offset + 3]);
            utf8_value.copy_from_slice(&encoding_table[offset + 3..offset + 3 + 8]);
            COMBINE_EUC_TO_UTF8_MAP
                .write()
                .unwrap()
                .insert(euc_3, utf8_value);
            match encoding_table[offset + 3] {
                0xc2..=0xdf => {
                    let utf8_4_key = [
                        encoding_table[offset + 3],
                        encoding_table[offset + 4],
                        encoding_table[offset + 3 + 4],
                        encoding_table[offset + 4 + 4],
                    ];
                    COMBINE_UTF8_4_TO_EUC_MAP
                        .write()
                        .unwrap()
                        .insert(utf8_4_key, euc_3);
                }
                0xe0..=0xef => {
                    let utf8_6_key = [
                        encoding_table[offset + 3],
                        encoding_table[offset + 4],
                        encoding_table[offset + 5],
                        encoding_table[offset + 3 + 4],
                        encoding_table[offset + 4 + 4],
                        encoding_table[offset + 5 + 4],
                    ];
                    COMBINE_UTF8_6_TO_EUC_MAP
                        .write()
                        .unwrap()
                        .insert(utf8_6_key, euc_3);
                }
                _ => {
                    println!(
                        "combine encoding_table error {:?}",
                        encoding_table[offset + 3]
                    );
                    return Err(SkkError::Encoding);
                }
            }
            offset += euc_utf8_combine_unit_length;
        }
        Ok(())
    }

    fn setup_map(
        encoding_table: &[u8],
        euc_utf8_unit_length: usize,
        euc_utf8_combine_unit_length: usize,
        header_length: usize,
        header_euc_utf8_length: usize,
        header_euc_utf8_combine_length: usize,
    ) -> SetupMapResult {
        let mut offset =
            header_length + header_euc_utf8_combine_length * euc_utf8_combine_unit_length;
        let mut euc_2_to_utf8_vec = vec![[0; 4]; Decoder::EUC_2_TO_UTF8_VEC_INDEX_MAXIMUM + 1];
        let mut utf8_3_to_euc_vec = vec![[0; 3]; Encoder::UTF8_3_TO_EUC_VEC_INDEX_MAXIMUM + 1];
        for _ in 0..header_euc_utf8_length {
            let euc_3 = [
                encoding_table[offset],
                encoding_table[offset + 1],
                encoding_table[offset + 2],
            ];
            let utf8 = [
                encoding_table[offset + 3],
                encoding_table[offset + 4],
                encoding_table[offset + 5],
                encoding_table[offset + 6],
            ];
            match encoding_table[offset] {
                0x8f => {
                    EUC_3_TO_UTF8_MAP.write().unwrap().insert(euc_3, utf8);
                }
                0x8e | 0xa0..=0xff => {
                    euc_2_to_utf8_vec[Decoder::get_euc_2_to_utf8_vec_index(
                        encoding_table[offset],
                        encoding_table[offset + 1],
                    )]
                    .copy_from_slice(&encoding_table[offset + 3..offset + 3 + 4]);
                }
                _ => {}
            }
            match utf8[0] {
                0x00..=0x7f => {}
                0xe0..=0xef => {
                    utf8_3_to_euc_vec[Encoder::get_utf8_3_to_euc_vec_index(&utf8)]
                        .copy_from_slice(&encoding_table[offset..offset + 3]);
                }
                _ => {
                    UTF8_2_4_TO_EUC_MAP.write().unwrap().insert(utf8, euc_3);
                }
            }
            offset += euc_utf8_unit_length;
        }
        (euc_2_to_utf8_vec, utf8_3_to_euc_vec)
    }

    /// unicode から utf8 に変換し `result_utf8` へ書き込む。書き込んだサイズを返す。
    ///
    /// これ Rust 標準でできないっけ?
    #[allow(clippy::cast_possible_truncation)]
    fn convert_unicode_to_utf8(unicode: u32, result_utf8: &mut [u8]) -> usize {
        match unicode {
            0x00..=0x7f => {
                result_utf8[0] = unicode as u8;
                1
            }
            0x80..=0x7ff => {
                result_utf8[0] = (((unicode >> 6) & 0b1_1111) + 0xc0) as u8;
                result_utf8[1] = ((unicode & 0b11_1111) + 0x80) as u8;
                2
            }
            0x800..=0xffff => {
                result_utf8[0] = (((unicode >> 12) & 0b1111) + 0xe0) as u8;
                result_utf8[1] = (((unicode >> 6) & 0b11_1111) + 0x80) as u8;
                result_utf8[2] = ((unicode & 0b11_1111) + 0x80) as u8;
                3
            }
            0x1_0000..=0x1f_ffff => {
                result_utf8[0] = (((unicode >> 18) & 0b111) + 0xf0) as u8;
                result_utf8[1] = (((unicode >> 12) & 0b11_1111) + 0x80) as u8;
                result_utf8[2] = (((unicode >> 6) & 0b11_1111) + 0x80) as u8;
                result_utf8[3] = ((unicode & 0b11_1111) + 0x80) as u8;
                4
            }
            _ => 0,
        }
    }

    #[allow(dead_code)]
    fn create_table(table_full_path: &str) -> Result<Vec<u8>, SkkError> {
        let header_version: u32 = 1;
        let header_length: u32 = 32;
        let mut header_euc_utf8_combine_length: u32 = 0;
        let mut header_euc_utf8_length: u32 = 0;
        let header_reserved: [u8; 4 * 4] = [0; 4 * 4];
        let mut euc_utf8_combine_table = Vec::new();
        let mut euc_utf8_table = Vec::new();
        let re_comment = Regex::new(r"^\s*#").unwrap();
        let re_line = Regex::new(r"^([^\s]+)\s+([^\s]+)\s+#").unwrap();
        let mut line = String::new();
        let mut reader = BufReader::new(File::open(table_full_path)?);
        while reader.read_line(&mut line)? > 0 {
            if re_comment.is_match(&line) {
                line.clear();
                continue;
            }
            if let Some(m) = re_line.captures(&line) {
                let base_euc = &m[1];
                let base_unicode = &m[2];
                let euc_3 = Self::convert_euc_code_to_euc_3_bytes(base_euc)?;
                let mut is_combine = false;
                let utf8_8 =
                    Self::convert_unicode_code_to_utf8_8_bytes(base_unicode, &mut is_combine)?;
                if is_combine {
                    header_euc_utf8_combine_length += 1;
                    euc_utf8_combine_table.extend_from_slice(&euc_3);
                    euc_utf8_combine_table.extend_from_slice(&utf8_8);
                } else {
                    header_euc_utf8_length += 1;
                    euc_utf8_table.extend_from_slice(&euc_3);
                    euc_utf8_table.extend_from_slice(&utf8_8[..4]);
                }
            }
            line.clear();
        }
        let mut result = Vec::new();
        result.extend_from_slice(&header_version.to_ne_bytes());
        result.extend_from_slice(&header_length.to_ne_bytes());
        result.extend_from_slice(&header_euc_utf8_combine_length.to_ne_bytes());
        result.extend_from_slice(&header_euc_utf8_length.to_ne_bytes());
        result.extend_from_slice(&header_reserved);
        result.extend_from_slice(&euc_utf8_combine_table);
        result.extend_from_slice(&euc_utf8_table);
        Ok(result)
    }

    fn write_4_bytes(string: &mut String, buffer: &[u8], suffix: &str) {
        _ = writeln!(
            string,
            "0x{:>02x},0x{:>02x},0x{:>02x},0x{:>02x},{suffix}",
            buffer[0], buffer[1], buffer[2], buffer[3]
        );
    }
}
