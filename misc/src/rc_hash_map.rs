use std::{collections::HashMap, sync::{atomic::AtomicU64, Arc, Mutex, Weak}, hash::Hash, ops::Deref};


// Steps for dereferencing
// 1. Decrement the count of references.
// 2. If it's zero after, lock the hash map's mutex and check the count again
// ... if still zero, remove the entry.

// Steps for looking up
// 1. Lock the hash map's mutex.
// 2. Look up the entry. Return a reference to it, incrementing the reference count.

// A hash map with reference counting per key.
pub struct ArcHashMap<K, V> {
    map: Arc<Mutex<HashMap<K, Arc<V>>>>
}
pub struct ArcHashMapEntry<K: Hash + Eq, V> {
    entry: Option<Arc<V>>,
    map: Weak<Mutex<HashMap<K, Arc<V>>>>,
}
impl<K: Hash + Eq, V> ArcHashMap<K, V> {
    // value should not panic.
    pub fn get_or_else(&mut self, key: K, value: impl FnOnce() -> V) -> ArcHashMapEntry<K, V> {
        let entry = {
            let mut lock = self.map.lock().unwrap();
            lock.entry(key).or_insert(Arc::new(value())).clone()
        };
        ArcHashMapEntry { entry: Some(entry), map: Arc::downgrade(&self.map) }
    }
}
impl<K: Hash + Eq, V> Deref for ArcHashMapEntry<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        // Will always exist until dropped.
        &*self.entry.unwrap()
    }
}
impl<K: Hash + Eq, V> Drop for ArcHashMapEntry<K, V> {
    fn drop(&mut self) {
       self.entry = None; 
        if let Some(map) = self.map.upgrade() {
            let mut lock = map.lock();
        }
    }
}