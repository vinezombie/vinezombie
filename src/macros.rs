macro_rules! collection_methods {
    ($name:tt) => {
        #[doc = "Returns the number of elements in `self`."]
        pub fn len(&self) -> usize {
            self.$name.len()
        }
        #[doc = "Returns true if `self` contains no elements."]
        pub fn is_empty(&self) -> bool {
            self.$name.is_empty()
        }
    };
}
