/// Estimativa: ~4 caracteres por token, arredondando para cima.
/// O tokenizer do Claude não é público; para comparação relativa basta.
pub fn estimate_tokens(byte_len: usize) -> u64 {
    byte_len.div_ceil(4) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_four_chars_per_token_rounding_up() {
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(1), 1);
        assert_eq!(estimate_tokens(4), 1);
        assert_eq!(estimate_tokens(5), 2);
        assert_eq!(estimate_tokens(400), 100);
    }
}
