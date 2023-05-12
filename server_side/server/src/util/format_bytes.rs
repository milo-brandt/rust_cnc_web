use std::ascii;

pub fn format_byte_string<Bytes: IntoIterator<Item=u8>>(bytes: Bytes) -> String {
    unsafe {
        String::from_utf8_unchecked(bytes.into_iter().flat_map(ascii::escape_default).collect())
    }
}