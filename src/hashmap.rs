use alloc::vec::Vec;

pub struct Tuple<K, V>
where
    K: Hashable,
{
    key: K,
    value: V,
}

impl<K, V> Tuple<K, V>
where
    K: Hashable + PartialEq,
{
    fn matches(&self, key: &K) -> bool {
        self.key == *key
    }

    pub fn to_v(&self) -> &V {
        &self.value
    }
}

impl<K, V> PartialEq for Tuple<K, V>
where
    K: Hashable,
    K: PartialEq,
{
    fn eq(&self, other: &Tuple<K, V>) -> bool {
        self.key == other.key
    }
}

pub trait Hashable {
    fn hash(&self) -> usize;
}

pub struct HashMap<K, V>
where
    K: Hashable,
{
    buckets: Vec<Vec<Tuple<K, V>>>,
}

impl<K, V> HashMap<K, V>
where
    K: Hashable,
    K: PartialEq,
{
    pub fn new() -> HashMap<K, V> {
        let mut n = HashMap {
            buckets: Vec::new(),
        };

        for _ in 0..128 {
            n.buckets.push(Vec::new())
        }
        n
    }

    pub fn get(&self, key: K) -> Option<&V> {
        let hash = key.hash() % self.buckets.len();
        if self.buckets[hash].is_empty() {
            return None;
        } else {
            for element in self.buckets[hash].iter() {
                if element.matches(&key) {
                    return Some(element.to_v());
                }
            }
            return None;
        }
    }

    pub fn put(&mut self, key: K, value: V) {
        let hash = key.hash();

        self.buckets[hash].push(Tuple { key, value });
    }

    pub fn remove(&mut self, key: K, value: V) {
        let hash = key.hash();
        let r = Tuple { key, value };

        self.buckets[hash].retain(|x| *x != r);
    }
}
