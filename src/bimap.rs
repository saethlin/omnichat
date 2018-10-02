use std::collections::hash_map::{IntoIter, Iter};
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone)]
pub struct BiMap<L, R> {
    left_to_right: HashMap<L, R>,
    right_to_left: HashMap<R, L>,
}

impl<L, R> BiMap<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self {
            left_to_right: HashMap::new(),
            right_to_left: HashMap::new(),
        }
    }

    pub fn insert<IL: Into<L>, IR: Into<R>>(&mut self, left: IL, right: IR) {
        let the_left = left.into();
        let the_right = right.into();
        self.left_to_right
            .insert(the_left.clone(), the_right.clone());
        self.right_to_left.insert(the_right, the_left);
    }

    pub fn get_left<Q: ?Sized>(&self, right: &Q) -> Option<&L>
    where
        R: ::std::borrow::Borrow<Q>,
        Q: Hash + Eq,
    {
        self.right_to_left.get(right)
    }

    pub fn get_right<Q: ?Sized>(&self, left: &Q) -> Option<&R>
    where
        L: ::std::borrow::Borrow<Q>,
        Q: Hash + Eq,
    {
        self.left_to_right.get(left)
    }

    pub fn iter(&self) -> Iter<L, R> {
        self.left_to_right.iter()
    }
}

impl<L, R> IntoIterator for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
{
    type Item = (L, R);
    type IntoIter = IntoIter<L, R>;

    fn into_iter(self) -> IntoIter<L, R> {
        self.left_to_right.into_iter()
    }
}
