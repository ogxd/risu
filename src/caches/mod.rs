pub mod lru;
pub use lru::LruCache as LruCache;

pub mod probatory;
pub use probatory::ProbatoryCache as ProbatoryCache;

pub mod sharded;
pub use sharded::ShardedCache as ShardedCache;

use std::sync::Arc;

trait Cache<K, V> {
    fn try_add(&mut self, key: K, value: Arc<V>) -> bool;

    fn try_get(&mut self, key: &K) -> Option<Arc<V>>;

    // Todo: Handle I/O errors
    // Todo: Handle item key VS hash key (there might be 2 factories in this case)
    // Todo: Pass the item key to the factory
    async fn get_or_add<F, Fut>(&mut self, key: K, factory: F) -> Arc<V>
    where
        K: Clone,
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = V>,
    {
        match self.try_get(&key) {
            Some(value) => return value,
            None => {
                let value = factory().await;
                let a_value = Arc::new(value);
                // This might fail if the key was added by another thread, but we don't care
                self.try_add(key.clone(), a_value.clone());
                a_value
            }
        }
    }
}