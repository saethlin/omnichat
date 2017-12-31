use std::collections::HashMap;

pub struct BiMapBuilder<'a> {
    pub id: &'a Vec<String>,
    pub human: &'a Vec<String>,
}

#[derive(Clone)]
pub struct BiMap {
    id_to_human: HashMap<String, String>,
    human_to_id: HashMap<String, String>,
}

impl BiMap {
    pub fn new(builder: BiMapBuilder) -> Self {
        let mut id_to_human = HashMap::new();
        let mut human_to_id = HashMap::new();

        for (id, human) in builder
            .id
            .iter()
            .cloned()
            .zip(builder.human.iter().cloned())
        {
            id_to_human.insert(id.clone(), human.clone());
            human_to_id.insert(human, id);
        }

        BiMap {
            id_to_human,
            human_to_id,
        }
    }

    pub fn get_human(&self, id: &str) -> Option<&String> {
        self.id_to_human.get(id)
    }

    pub fn get_id(&self, human: &str) -> Option<&String> {
        self.human_to_id.get(human)
    }
}
