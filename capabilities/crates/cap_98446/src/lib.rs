/// Return a friendly greeting message.
pub fn greet() -> String {
    "Hello! How can I help you today?".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet(), "Hello! How can I help you today?");
    }
}
