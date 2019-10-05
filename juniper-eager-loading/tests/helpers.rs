#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

pub struct StatsHash<K: Hash + Eq, V> {
    map: HashMap<K, V>,
    count: AtomicUsize,
    name: &'static str,
}

impl<K: Hash + Eq, V> StatsHash<K, V> {
    pub fn new(name: &'static str) -> Self {
        StatsHash {
            map: HashMap::default(),
            count: AtomicUsize::default(),
            name,
        }
    }

    #[allow(dead_code)]
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.increment_reads_count();
        self.map.get(k)
    }

    #[allow(dead_code)]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.increment_reads_count();
        self.map.get_mut(k)
    }

    pub fn all_values(&self) -> Vec<&V> {
        self.increment_reads_count();
        self.map.iter().map(|(_, v)| v).collect()
    }

    pub fn reads_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    pub fn increment_reads_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

pub trait SortedExtension {
    fn sorted(self) -> Self;
}

impl<T: std::cmp::Ord> SortedExtension for Vec<T> {
    fn sorted(mut self) -> Self {
        self.sort();
        self
    }
}
