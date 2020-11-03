use crate::skk::encoding_simple::{Encoder, Utility};

impl Encoder {
    /// 3 bytes utf8 から `utf8_3_to_euc_vec` の index を返す
    ///
    /// 先頭の 0x0002- は空になるので、引数が範囲外の場合は 2 を返す。
    /// 返す値は 0 から `Self::UTF8_3_TO_EUC_VEC_INDEX_MAXIMUM` の間に収まる。
    ///
    /// 3 bytes utf8 の範囲は、
    ///     0xe0-0xef, 0x80-0xbf, 0x80-0xbf
    ///          4bit,      6bit,      6bit
    ///          0x0f,      0x3f,      0x3f
    /// と 16bit だが、実データでは最小値が 0x1e3e になるのでこれを引いてやることで
    /// 0x0000-0xe161(57697) に収まる。さらに 0x8165-0xdade に大きな隙間ができるので、
    /// これらを考慮すると 0x0000-0x87e8(34792) に収まる。
    pub(crate) const UTF8_3_TO_EUC_VEC_INDEX_RAW_MINIMUM: usize = 0x1e3e;
    pub(crate) const UTF8_3_TO_EUC_VEC_INDEX_MAXIMUM: usize =
        0xff9f - Self::UTF8_3_TO_EUC_VEC_INDEX_RAW_MINIMUM - (0xdade - 0x8165);
    const UTF8_3_TO_EUC_VEC_INDEX_EMPTY: usize = 2;
    pub(crate) const fn get_utf8_3_to_euc_vec_index(utf8: &[u8]) -> usize {
        if utf8[0] < 0xe0 || utf8[1] < 0x80 || utf8[2] < 0x80 {
            return Self::UTF8_3_TO_EUC_VEC_INDEX_EMPTY;
        }
        let utf8_a = (utf8[0] - 0xe0) as usize;
        let utf8_b = (utf8[1] - 0x80) as usize;
        let utf8_c = (utf8[2] - 0x80) as usize;
        let mut raw_index = (utf8_a << 12) | (utf8_b << 6) | utf8_c;
        if raw_index > 0xdade {
            raw_index -= 0xdade - 0x8165;
        }
        if raw_index < Self::UTF8_3_TO_EUC_VEC_INDEX_RAW_MINIMUM {
            return Self::UTF8_3_TO_EUC_VEC_INDEX_EMPTY;
        }
        let result = raw_index - Self::UTF8_3_TO_EUC_VEC_INDEX_RAW_MINIMUM;
        if result > Self::UTF8_3_TO_EUC_VEC_INDEX_MAXIMUM {
            return Self::UTF8_3_TO_EUC_VEC_INDEX_EMPTY;
        }
        result
    }

    pub(crate) fn push_to_buffer_euc(euc: &[u8], write_buffer: &mut Vec<u8>) {
        if euc[0] == 0x8f {
            write_buffer.extend_from_slice(&euc[..3]);
        } else if euc[0] >= 0x80 {
            write_buffer.extend_from_slice(&euc[..2]);
        } else {
            write_buffer.push(euc[0]);
        }
    }

    pub(crate) const fn is_utf8_2_bytes(
        utf8_buffer: &[u8],
        utf8_buffer_length: usize,
        utf8_i: usize,
    ) -> bool {
        Utility::is_enough_2_bytes(utf8_buffer_length, utf8_i)
            && utf8_buffer[utf8_i + 1] >= 0x80
            && utf8_buffer[utf8_i + 1] <= 0xbf
    }

    pub(crate) const fn is_utf8_3_bytes(
        utf8_buffer: &[u8],
        utf8_buffer_length: usize,
        utf8_i: usize,
    ) -> bool {
        Utility::is_enough_3_bytes(utf8_buffer_length, utf8_i)
            && utf8_buffer[utf8_i + 1] >= 0x80
            && utf8_buffer[utf8_i + 1] <= 0xbf
            && utf8_buffer[utf8_i + 2] >= 0x80
            && utf8_buffer[utf8_i + 2] <= 0xbf
    }

    pub(crate) const fn is_utf8_4_bytes(
        utf8_buffer: &[u8],
        utf8_buffer_length: usize,
        utf8_i: usize,
    ) -> bool {
        Utility::is_enough_4_bytes(utf8_buffer_length, utf8_i)
            && utf8_buffer[utf8_i + 1] >= 0x80
            && utf8_buffer[utf8_i + 1] <= 0xbf
            && utf8_buffer[utf8_i + 2] >= 0x80
            && utf8_buffer[utf8_i + 2] <= 0xbf
            && utf8_buffer[utf8_i + 3] >= 0x80
            && utf8_buffer[utf8_i + 3] <= 0xbf
    }

    pub(crate) const fn is_candidate_combine_utf8_4(utf8_buffer: &[u8], utf8_i: usize) -> bool {
        utf8_buffer[utf8_i + 2] == 0xcc || utf8_buffer[utf8_i + 2] == 0xcb
    }

    pub(crate) const fn is_candidate_combine_utf8_6(utf8_buffer: &[u8], utf8_i: usize) -> bool {
        utf8_buffer[utf8_i + 4] == 0x82 && utf8_buffer[utf8_i + 5] == 0x9a
    }
}
