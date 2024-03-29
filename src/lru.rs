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
    map:HashMap<H, LruCacheEntry<V>>,
    expiration: Duration,
    expiration_type: ExpirationType,
    factory: dyn Fn(&K) -> H,
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
    pub fn try_add(&mut self, key: K, value: V) -> bool {
        let key = (self.factory)(&key);
        let mut added = false;

        self.map.entry(key).or_insert_with_key(|k| {
            added = true;
            LruCacheEntry {
                node_index: self.lru_list.add_last(k.clone()).unwrap(),
                insertion: Instant::now(),
                value,
            }
        });

        if added {
            // Release space
        }

        return added;
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
                    None
                } else {
                    if self.expiration_type == ExpirationType::Sliding {
                        // Refresh duration
                        entry.get_mut().insertion = Instant::now();
                    }
        
                    // Move to the end of the list
                    lru_list.remove(entry.get().node_index).expect("Failed to remove node, cache is likely corrupted");
                    entry.get_mut().node_index = lru_list.add_last(key.clone()).unwrap();
        
                    let value_ref = &entry.get().value;
                    Some(value_ref) // ERROR: cannot return value referencing local variable `entry`
                }
            },
            Entry::Vacant(_) => {
                None
            }
        }
    }
}