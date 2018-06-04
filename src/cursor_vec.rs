pub struct CursorVec<T> {
    index: usize,
    vec: Vec<T>,
}

impl<T> CursorVec<T> {
    // The whole point of this structure is to always be able to get from it
    pub fn new(base: Vec<T>) -> Self<T> {
        assert!(base.len() > 0);
        Self {
            index: 0,
            vec: base,
        }
    }
    pub fn get(&self) -> &T {
        debug_assert!(self.index < self.vec.len());
        unsafe {self.vec.get_unchecked(self.index)}
    }
    pub fn next(&mut self) {
        self.index += 1;
        self.index %= self.vec.len();
    }
    pub fn prev(&mut self) {
        self.index -= 1;
        self.index %= self.vec.len();
    }
    pub fn wrapping_seek(&mut self, index: usize) {
        self.index = index;
        self.index %= self.vec.len();
    }
    pub fn try_get_at(&self, index: usize) -> Option<&T> {
        self.vec.get(index)
    }
    pub fn push(&mut self, item: T) {
        self.vec.push(item)
    }
}
