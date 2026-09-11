pub fn encode_length(length: usize, output: &mut Vec<u8>) {
    if length <= 127 {
        output.push(length as u8);
    } else {
        // FastCGI uses the high bit to mark this four-byte big-endian form.
        let encoded = (length as u32 | 0x8000_0000).to_be_bytes();
        output.extend_from_slice(&encoded);
    }
}

pub fn encode_param(name: &str, value: &str, output: &mut Vec<u8>) {
    encode_length(name.len(), output);
    encode_length(value.len(), output);
    output.extend_from_slice(name.as_bytes());
    output.extend_from_slice(value.as_bytes());
}
