use alloc::vec::Vec;

pub struct Tuple<T, V>
where
    T: Hashable,
{
    key: T,
    value: V,
}

impl<T, V> Tuple<T, V>
where
    T: Hashable,
    T: PartialEq,
{
    fn matches(&self, key: &T) -> bool {
        self.key == *key
    }

    pub fn toV(&self) -> &V {
        &self.value
    }
}

pub trait Hashable {
    fn hash(&self) -> usize;
}

pub struct HashMap<T, V>
where
    T: Hashable,
{
    buckets: Vec<Vec<Tuple<T, V>>>,
}

impl<T, V> HashMap<T, V>
where
    T: Hashable,
    T: PartialEq,
{
    pub fn new() -> HashMap<T, V> {
        let mut n = HashMap {
            buckets: Vec::new(),
        };

        for _ in 0..128 {
            n.buckets.push(Vec::new())
        }
        n
    }

    pub fn get(&self, key: T) -> Option<&Tuple<T, V>> {
        let hash = key.hash();
        if self.buckets[hash].is_empty() {
            return None;
        } else {
            for element in self.buckets[hash].iter() {
                if element.matches(&key) {
                    return Some(element);
                }
            }
            return None;
        }
    }

    pub fn put(&mut self, key: T, value: V) {
        let hash = key.hash();

        self.buckets[hash].push(Tuple { key, value });
    }
}
