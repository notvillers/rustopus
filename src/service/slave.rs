use uuid::Uuid;

/// This functions gets an uuid
pub fn get_uuid() -> String {
    Uuid::new_v4().to_string()
}
