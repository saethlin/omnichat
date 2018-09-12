/// A continer backed by a Vec with a cursor that always points to a valid element,
/// and therefore it is always possible to get the current element.
/// The backing container must never be empty.
pub struct CursorVec<T> {
    index: usize,
    vec: Vec<T>,
}

impl<T> CursorVec<T> {
    /// Construct a CursorVec from a single element
    pub fn new(first: T) -> CursorVec<T> {
        Self {
            index: 0,
            vec: vec![first],
        }
    }

    pub fn get(&self) -> &T {
        unsafe { self.vec.get_unchecked(self.index) }
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { self.vec.get_unchecked_mut(self.index) }
    }

    pub fn next(&mut self) {
        self.index += 1;
        self.index %= self.vec.len();
    }

    pub fn prev(&mut self) {
        if self.index == 0 {
            self.index = self.vec.len() - 1;
        } else {
            self.index -= 1;
            self.index %= self.vec.len();
        }
    }

    pub fn get_first_mut(&mut self) -> &mut T {
        unsafe { self.vec.get_unchecked_mut(0) }
    }

    pub fn push(&mut self, item: T) {
        self.vec.push(item)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.vec.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.vec.iter_mut()
    }

    pub fn tell(&self) -> usize {
        self.index
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn sort_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.vec.sort_by_key(f);
    }
}
