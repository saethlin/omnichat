use std::collections::HashMap;
use std::hash::Hash;
use std::collections::hash_map::Iter;
#[derive(Clone)]
pub struct BiMapBuilder<I, H> {
    pub id: Vec<I>,
    pub human: Vec<H>,
}

#[derive(Clone)]
pub struct BiMap<I, H> {
    id_to_human: HashMap<I, H>,
    human_to_id: HashMap<H, I>,
}

impl<I, H> BiMap<I, H>
where
    I: Eq + Hash + Clone,
    H: Eq + Hash + Clone,
{
    pub fn new(builder: BiMapBuilder<I, H>) -> Self {
        let mut id_to_human = HashMap::new();
        let mut human_to_id = HashMap::new();

        for (id, human) in builder.id.into_iter().zip(builder.human.into_iter()) {
            id_to_human.insert(id.clone(), human.clone());
            human_to_id.insert(human, id);
        }

        BiMap {
            id_to_human,
            human_to_id,
        }
    }

    pub fn get_human(&self, id: &I) -> Option<&H> {
        self.id_to_human.get(id)
    }

    pub fn get_id(&self, human: &H) -> Option<&I> {
        self.human_to_id.get(human)
    }

    pub fn iter(&self) -> Iter<I, H > {
        self.id_to_human.iter()
    }

    pub fn contains_human(&self, val: &H) -> bool {
        self.human_to_id.contains_key(val)
    }

    pub fn contains_id(&self, val: &I) -> bool {
        self.id_to_human.contains_key(val)
    }

}
