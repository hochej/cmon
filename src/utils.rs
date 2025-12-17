//! Utility functions shared across modules.

/// Find a key in a collection that matches the target case-insensitively.
///
/// This is useful for matching user-configured partition names against
/// actual Slurm partition names, where case may differ.
///
/// # Example
/// ```
/// let keys = vec!["CPU".to_string(), "GPU".to_string()];
/// let found = find_partition_key(keys.iter(), "cpu");
/// assert_eq!(found, Some(&"CPU".to_string()));
/// ```
pub fn find_partition_key<'a>(
    keys: impl Iterator<Item = &'a String>,
    config_name: &str,
) -> Option<&'a String> {
    keys.into_iter()
        .find(|k| k.eq_ignore_ascii_case(config_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_partition_key_exact_match() {
        let keys = vec!["cpu".to_string(), "gpu".to_string()];
        assert_eq!(find_partition_key(keys.iter(), "cpu"), Some(&keys[0]));
    }

    #[test]
    fn test_find_partition_key_case_insensitive() {
        let keys = vec!["CPU".to_string(), "GPU".to_string()];
        assert_eq!(find_partition_key(keys.iter(), "cpu"), Some(&keys[0]));
        assert_eq!(find_partition_key(keys.iter(), "Cpu"), Some(&keys[0]));
        assert_eq!(find_partition_key(keys.iter(), "GPU"), Some(&keys[1]));
    }

    #[test]
    fn test_find_partition_key_not_found() {
        let keys = vec!["cpu".to_string(), "gpu".to_string()];
        assert_eq!(find_partition_key(keys.iter(), "fat"), None);
    }

    #[test]
    fn test_find_partition_key_empty_keys() {
        let keys: Vec<String> = vec![];
        assert_eq!(find_partition_key(keys.iter(), "cpu"), None);
    }
}
