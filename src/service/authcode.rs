// Authcode validation

pub fn is_well_formed(authcode: &str) -> bool {
    !authcode.is_empty()
        && authcode.chars().all(|character| character.is_ascii_alphanumeric() || character == '-')
}