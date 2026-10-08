pub struct Engine;
impl Engine {
    pub fn search(&self, needle: &str) -> bool { !needle.is_empty() }
}
