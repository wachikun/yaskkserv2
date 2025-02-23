use crate::skk::encoding_simple::Decoder;

impl Decoder {
    /// 2 bytes euc から `euc_2_to_utf8_vec` の index を返す
    ///
    /// 0x0001-0x0018 までは空になるので、引数が範囲外の場合は 1 を返す。
    /// 返す値は 0 から `Self::EUC_2_TO_UTF8_VEC_INDEX_MAXIMUM` の間に収まる。
    ///
    /// 2 bytes euc の範囲は、
    ///     high: 0x8e-0xfe -> 0x00-0x70
    ///      low: 0xa1-0xfe -> 0x00-0x5d
    /// なので、 high low 入れ替えることで 0x0000-0x5d70(23920) に収まる。
    pub(crate) const EUC_2_TO_UTF8_VEC_INDEX_MAXIMUM: usize = 0x5d70;
    const EUC_2_TO_UTF8_VEC_INDEX_EMPTY: usize = 1;
    pub(crate) const fn get_euc_2_to_utf8_vec_index(high: u8, low: u8) -> usize {
        if low < 0xa1 || high < 0x8e {
            return Self::EUC_2_TO_UTF8_VEC_INDEX_EMPTY;
        }
        let high = high - 0x8e;
        let low = low - 0xa1;
        let result = ((low as usize) << 8) | high as usize;
        if result > Self::EUC_2_TO_UTF8_VEC_INDEX_MAXIMUM {
            return Self::EUC_2_TO_UTF8_VEC_INDEX_EMPTY;
        }
        result
    }

    pub(crate) fn push_to_buffer_utf8(utf8: &[u8], write_buffer: &mut Vec<u8>) {
        if utf8[0] >= 0xf0 {
            write_buffer.extend_from_slice(&utf8[..4]);
        } else if utf8[0] >= 0xe0 {
            write_buffer.extend_from_slice(&utf8[..3]);
        } else if utf8[0] >= 0xc2 {
            write_buffer.extend_from_slice(&utf8[..2]);
        } else {
            write_buffer.push(utf8[0]);
        }
    }
}
