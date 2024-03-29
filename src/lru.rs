use crate::ArenaLinkedList;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(PartialEq)]
enum ExpirationType {
    Absolute,
    Sliding
}

pub struct LruCacheData<H, V> {
    lru_list: ArenaLinkedList<H>,
    map: HashMap<H, LruCacheEntry<V>>,
}


pub struct LruCache<K, H, V> {
    lru_list: Arc<Mutex<ArenaLinkedList<H>>>,
    map: Arc<Mutex<HashMap<H, LruCacheEntry<V>>>>,
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
    pub fn try_get(&self, key: &H) -> Option<&V> {
        // If found in the map, remove from the lru list and reinsert at the end
        let mut lru_list = self.lru_list.lock().unwrap();
        let mut map = self.map.lock().unwrap();

        if let Some(entry) = map.get_mut(key) {
            if Instant::now() - entry.insertion > self.expiration {
                // Entry has expired, we remove it and pretend it's not in the cache
                // todo
                return None;
            }

            if self.expiration_type == ExpirationType::Sliding {
                // Refresh duration
                entry.insertion = Instant::now();
            }

            // Move to the end of the list
            lru_list.remove(entry.node_index);
            entry.node_index = lru_list.add_last(key.clone()).unwrap();

            None
            //Some(&entry.value) // cannot return value referencing local variable `map` returns a value referencing data owned by the current function
        } else {
            None
        }
    }
}