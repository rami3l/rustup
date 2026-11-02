pub struct HashEncoder;

impl HashEncoder {
    pub const ALPHABET: [u8; 32] = *b"0123456789abcdefhjkmnqprstuvwxyz";

    pub fn encode(hash: u64) -> String {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss
        )]
        let width = ((size_of::<u64>() as f32) * 8. / 32_f32.log2()).ceil() as usize;
        let raw = base_x::encode(Self::ALPHABET.as_ref(), &hash.to_be_bytes());
        format!("{raw:0>width$}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_encode() {
        let original = [1, 112_358_777, 1_618_033_988, 2_718_281_828, u64::MAX - 1];
        let encoded = original.map(HashEncoder::encode);
        assert_eq!(
            encoded,
            [
                "0000000000001",
                "00000003b4xbt",
                "0000001h72fa4",
                "0000002j0bc34",
                "fzzzzzzzzzzzy"
            ]
        );
        assert_eq!(
            encoded.iter().map(String::len).collect::<Vec<_>>(),
            [13, 13, 13, 13, 13]
        );
    }
}
