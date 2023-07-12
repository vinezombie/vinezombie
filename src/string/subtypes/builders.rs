impl super::NoNulBuilder {
    /// Appends a space.
    pub fn push_space(&mut self) {
        self.bytes.push(b' ');
    }
}
impl super::LineBuilder {
    /// Appends a space.
    pub fn push_space(&mut self) {
        self.bytes.push(b' ');
    }
}
