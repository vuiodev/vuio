/// Generates a natural sort key for a string.
/// Numeric substrings are padded with leading zeros to a fixed width of 20 characters
/// so that lexicographical sorting sorts them numerically.
pub fn natural_sort_key(input: &str) -> String {
    let mut key = String::with_capacity(input.len() + 10);
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            let mut num_str = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_ascii_digit() {
                    num_str.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            let trimmed = num_str.trim_start_matches('0');
            let val = if trimmed.is_empty() { "0" } else { trimmed };
            if val.len() < 20 {
                // Pad with leading zeros to 20 digits
                for _ in 0..(20 - val.len()) {
                    key.push('0');
                }
                key.push_str(val);
            } else {
                key.push_str(val);
            }
        } else {
            key.push(chars.next().unwrap());
        }
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_sort_key() {
        assert_eq!(natural_sort_key("a1b"), "a00000000000000000001b");
        assert_eq!(natural_sort_key("a01b"), "a00000000000000000001b");
        assert_eq!(natural_sort_key("a10b"), "a00000000000000000010b");

        // Verify comparison
        assert!(natural_sort_key("s01e02") < natural_sort_key("s01e08"));
        assert!(natural_sort_key("s01e08") < natural_sort_key("s01e10"));
        assert!(natural_sort_key("Season 2") < natural_sort_key("Season 10"));
        assert!(natural_sort_key("Episode 1.mkv") < natural_sort_key("Episode 2.mkv"));
        assert!(natural_sort_key("Episode 2.mkv") < natural_sort_key("Episode 10.mkv"));

        // Mixed cases and empty string
        assert_eq!(natural_sort_key(""), "");
        assert_eq!(natural_sort_key("abc"), "abc");
        assert_eq!(natural_sort_key("123"), "00000000000000000123");
    }
}
