use crate::ArenaLinkedList;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(PartialEq)]
enum ExpirationType {
    Absolute,
    Sliding
}

pub struct LruCache<K, H, V> {
    lru_list: ArenaLinkedList<H>,
    map: HashMap<H, LruCacheEntry<V>>,
    expiration: Duration,
    expiration_type: ExpirationType,
    max_size: usize,
    factory: Box<dyn Fn(&K) -> H>,
}

pub struct LruCacheEntry<V> {
    node_index: usize,
    insertion: Instant,
    value: V,
}

impl<K, H, V> LruCache<K, H, V>
where
    H: Eq + std::hash::Hash + Clone,
{
    pub fn new(factory: Box<dyn Fn(&K) -> H>) -> Self {
        Self {
            lru_list: ArenaLinkedList::new_with_capacity(4),
            map: HashMap::new(),
            expiration: Duration::MAX,
            expiration_type: ExpirationType::Absolute,
            max_size: 4,
            factory: factory
        }
    }

    pub fn try_add(&mut self, key: K, value: V) -> bool {
        let key = (self.factory)(&key);
        let mut added = false;

        self.map.entry(key).or_insert_with_key(|k| {
            added = true;
            LruCacheEntry {
                node_index: self.lru_list.add_last(k.clone()).expect("Failed to add node to list"),
                insertion: Instant::now(),
                value,
            }
        });

        if added {
            self.trim();
        }

        return added;
    }

    fn trim(&mut self) {
        let mut index = self.lru_list.get_first_index().unwrap_or(usize::MAX);
        while index != usize::MAX {
            let node = self.lru_list.get(index).expect("Failed to get node, cache is likely corrupted");
            let key = node.get_value().as_ref().expect("Node has no value, cache is likely corrupted").clone();
            let entry = self.map.get(&key).expect("Node not found in map, cache is likely corrupted");
            let next_index = node.get_after_index();
            if self.lru_list.count() > self.max_size
            || Instant::now() - entry.insertion > self.expiration {
                self.map.remove(&key);
                self.lru_list.remove(index).expect("Failed to remove node, cache is likely corrupted");
            } else {
                break;
            }
            index = next_index;
        }
    }

    pub fn try_get(&mut self, key: &H) -> Option<&V> {
        // If found in the map, remove from the lru list and reinsert at the end
        let lru_list = &mut self.lru_list;

        match self.map.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                if Instant::now() - entry.get().insertion > self.expiration {
                    // Entry has expired, we remove it and pretend it's not in the cache
                    lru_list.remove(entry.get().node_index).expect("Failed to remove node, cache is likely corrupted");
                    entry.remove_entry();
                    println!("Expired key");
                    None
                } else {
                    if self.expiration_type == ExpirationType::Sliding {
                        // Refresh duration
                        entry.get_mut().insertion = Instant::now();
                    }
        
                    // Move to the end of the list
                    lru_list.remove(entry.get().node_index).expect("Failed to remove node, cache is likely corrupted");
                    entry.get_mut().node_index = lru_list.add_last(key.clone()).expect("Failed to add node to list, cache is likely corrupted");

                    Some(&entry.into_mut().value)
                }
            },
            Entry::Vacant(_) => {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut lru = LruCache::<u32, u32, &str>::new(Box::new(|x: &u32| x.clone()));
        assert!(lru.try_get(&1).is_none());
        assert!(lru.try_add(1, "hello"));
        assert!(!lru.try_add(1, "hello"));
        assert!(lru.try_get(&1).is_some());
    }

    #[test]
    fn trimming() {
        let mut lru = LruCache::<u32, u32, &str>::new(Box::new(|x: &u32| x.clone()));
        assert!(lru.try_add(1, "h"));
        assert!(lru.try_add(2, "e"));
        assert!(lru.try_add(3, "l"));
        assert!(lru.try_add(4, "l"));
        // Max size is reached, next insertion should evict oldest entry, which is 1
        assert!(lru.try_add(5, "o"));
        assert!(lru.try_get(&1).is_none());
        assert!(lru.try_get(&2).is_some());
        assert!(lru.try_get(&3).is_some());
        assert!(lru.try_get(&4).is_some());
        assert!(lru.try_get(&5).is_some());
    }

    #[test]
    fn reordering() {
        let mut lru = LruCache::<u32, u32, &str>::new(Box::new(|x: &u32| x.clone()));
        assert!(lru.try_add(1, "h"));
        assert!(lru.try_add(2, "e"));
        assert!(lru.try_add(3, "l"));
        assert!(lru.try_add(4, "l"));
        assert!(lru.try_get(&1).is_some());
        // Max size is reached, next insertion should evict oldest entry, which is 2, because 1 was just accessed
        assert!(lru.try_add(5, "o"));
        assert!(lru.try_get(&1).is_some());
        assert!(lru.try_get(&2).is_none());
        assert!(lru.try_get(&3).is_some());
        assert!(lru.try_get(&4).is_some());
        assert!(lru.try_get(&5).is_some());
    }
}