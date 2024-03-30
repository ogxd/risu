use crate::ProbatoryCache;
use std::{borrow::BorrowMut, collections::HashMap, hash::{DefaultHasher, Hasher}, ops::Deref, sync::{Arc, Mutex}, time::Duration};

use super::lru::ExpirationType;

#[allow(dead_code)]
pub struct ShardedCache<K, V>
{
    shards: Vec<Arc<Mutex<ProbatoryCache<K, V>>>>,
}

#[allow(dead_code)]
impl<K, V> ShardedCache<K, V>
where
    K: Eq + std::hash::Hash + Clone
{
    pub fn new(shards: usize, max_size: usize, expiration: Duration, expiration_type: ExpirationType) -> Self {
        Self {
            shards: (0..shards).map(|_| Arc::new(Mutex::new(ProbatoryCache::new(max_size, expiration, expiration_type)))).collect()
        }
    }

    fn get_shard(&self, key: &K) -> &Arc<Mutex<ProbatoryCache<K, V>>> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish() as usize;
        let shard = hash % self.shards.len();
        &self.shards[shard]
    }

    pub fn try_add(&mut self, key: K, value: V) -> bool {
        self.get_shard(&key).lock().unwrap().try_add(key, value)
    }

    pub fn try_get(&mut self, key: &K) -> Option<&V> {
        let mut shard = self.get_shard(&key).lock().unwrap();
        let value = shard.try_get(key);
        value // ERROR: cannot return value referencing local variable `shard` returns a value referencing data owned by the current function
    }

    // pub fn try_get2(&mut self, key: &K) -> Option<Arc<&V>> {
    //     let mut shard = self.get_shard(&key).lock().unwrap();
    //     shard.try_get(key).map(Arc::new)
    // }

    // pub fn try_get2<'a>(&'a self, key: &'a K) -> Option<&'a V> {
    //     let mut shard = self.get_shard(&key).lock().unwrap();
    //     shard.try_get(key)
    // }
}

pub struct MyStruct1 {
    map: HashMap<u32, Arc<u32>>
}

impl MyStruct1 {
    pub fn get(&self, key: u32) -> Option<&Arc<u32>> {
        self.map.get(&key)
    }
}

pub struct MyStruct2 {
    s: Arc<Mutex<MyStruct1>>
}

impl MyStruct2 {
    pub fn get(&self, key: u32) -> Option<Arc<u32>> {
        let s = self.s.lock().unwrap();
        match s.get(key) {
            Some(value) => Some(value.clone()),
            None => None
        }
    }

    // pub fn get2(&self, key: u32) -> Option<&u32> {
    //     let s = self.s.lock().unwrap();
    //     s.deref().get(key)
    // }

    // pub fn get3(&self, key: u32) -> Option<&u32> {
    //     let s = self.s.lock().unwrap();
    //     s.deref().get(key).map(|value| value)
    // }

    // pub fn get4(&self, key: u32) -> Option<&u32> {
    //     let s = self.s.lock().unwrap();
    //     (*s).get(key)
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut lru = ShardedCache::new(2, 4, Duration::MAX, ExpirationType::Absolute);
        assert!(lru.try_get(&1).is_none());
        assert!(lru.try_add(1, "hello"));
        assert!(lru.try_get(&1).is_none(), "Key should only be in the probatory cache");
        assert!(lru.try_add(1, "hello"));
        assert!(lru.try_get(&1).is_some(), "Key should now be in the resident cache");
        assert!(!lru.try_add(1, "hello"));
    }

    #[test]
    fn trimming() {
        let mut lru = ShardedCache::new(2, 4, Duration::MAX, ExpirationType::Absolute);
        // Add every entry twice for them to enter the resident cache
        assert!(lru.try_add(1, "h"));
        assert!(lru.try_add(1, "h"));
        assert!(lru.try_add(2, "e"));
        assert!(lru.try_add(2, "e"));
        assert!(lru.try_add(3, "l"));
        assert!(lru.try_add(3, "l"));
        assert!(lru.try_add(4, "l"));
        assert!(lru.try_add(4, "l"));
        // Max size is reached, next insertion should evict oldest entry, which is 1
        assert!(lru.try_add(5, "o"));
        // "o" was only accessed once, so it only is in the probatory cache, so no eviction should have happened in the resident cache
        assert!(lru.try_get(&1).is_some());
        assert!(lru.try_add(5, "o"));
        assert!(lru.try_get(&1).is_none());
        assert!(lru.try_get(&2).is_some());
        assert!(lru.try_get(&3).is_some());
        assert!(lru.try_get(&4).is_some());
        assert!(lru.try_get(&5).is_some());
    }
}