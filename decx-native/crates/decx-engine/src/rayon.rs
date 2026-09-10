//! Minimal sequential `rayon` fallback for decx-engine (original decx-native
//! code). Folded in from the former standalone shim crate.
//! Implements only the surface vendored dexdec/rusty-dex actually use:
//! `use rayon::prelude::*` with par_iter/par_iter_mut/into_par_iter/par_chunks*
//! plus `rayon::join`. Everything runs single-threaded.

/// rayon::join — sequential evaluation (a first, then b).
pub fn join<A, B>(a: impl FnOnce() -> A, b: impl FnOnce() -> B) -> (A, B) {
    (a(), b())
}

pub mod prelude {
    pub use super::join;

    /// `self.into_par_iter()`
    pub trait IntoParallelIterator {
        type Item;
        type Iter: Iterator<Item = Self::Item>;
        fn into_par_iter(self) -> Self::Iter;
    }

    impl<T> IntoParallelIterator for Vec<T> {
        type Item = T;
        type Iter = std::vec::IntoIter<T>;
        fn into_par_iter(self) -> Self::Iter {
            self.into_iter()
        }
    }

    impl<A, B> IntoParallelIterator for (A, B)
    where
        A: IntoParallelIterator,
        B: IntoParallelIterator,
    {
        type Item = (A::Item, B::Item);
        type Iter = std::iter::Zip<A::Iter, B::Iter>;
        fn into_par_iter(self) -> Self::Iter {
            self.0.into_par_iter().zip(self.1.into_par_iter())
        }
    }

    /// `&x.par_iter()` — `Item` mirrors the rayon item type (maps yield
    /// `(&K, &V)` for `Item = (K, V)`), so `Iter` is not bound to `&Item`.
    pub trait IntoParallelRefIterator<'data> {
        type Iter: Iterator;
        type Item: 'data;
        fn par_iter(&'data self) -> Self::Iter;
    }

    impl<'data, T: 'data + Sync> IntoParallelRefIterator<'data> for [T] {
        type Iter = std::slice::Iter<'data, T>;
        type Item = T;
        fn par_iter(&'data self) -> Self::Iter {
            self.iter()
        }
    }

    impl<'data, T: 'data + Sync> IntoParallelRefIterator<'data> for Vec<T> {
        type Iter = std::slice::Iter<'data, T>;
        type Item = T;
        fn par_iter(&'data self) -> Self::Iter {
            self[..].iter()
        }
    }

    /// `x.par_iter_mut()` — maps yield `(&K, &mut V)`.
    pub trait IntoParallelRefMutIterator<'data> {
        type Iter: Iterator;
        type Item: 'data;
        fn par_iter_mut(&'data mut self) -> Self::Iter;
    }

    impl<'data, T: 'data + Send> IntoParallelRefMutIterator<'data> for [T] {
        type Iter = std::slice::IterMut<'data, T>;
        type Item = T;
        fn par_iter_mut(&'data mut self) -> Self::Iter {
            self.iter_mut()
        }
    }

    impl<'data, T: 'data + Send> IntoParallelRefMutIterator<'data> for Vec<T> {
        type Iter = std::slice::IterMut<'data, T>;
        type Item = T;
        fn par_iter_mut(&'data mut self) -> Self::Iter {
            self[..].iter_mut()
        }
    }

    /// `x.par_chunks(n)`
    pub trait ParChunks<'data> {
        type Iter: Iterator<Item = &'data [Self::Item]>;
        type Item: 'data;
        fn par_chunks(&'data self, size: usize) -> Self::Iter;
        fn par_chunks_exact(&'data self, size: usize) -> Self::Iter {
            self.par_chunks(size.max(1))
        }
    }

    impl<'data, T: 'data + Sync> ParChunks<'data> for [T] {
        type Iter = std::slice::Chunks<'data, T>;
        type Item = T;
        fn par_chunks(&'data self, size: usize) -> Self::Iter {
            self.chunks(size.max(1))
        }
    }

    impl<'data, T: 'data + Sync> ParChunks<'data> for Vec<T> {
        type Iter = std::slice::Chunks<'data, T>;
        type Item = T;
        fn par_chunks(&'data self, size: usize) -> Self::Iter {
            self[..].chunks(size.max(1))
        }
    }

    /// `x.par_chunks_mut(n)`
    pub trait ParChunksMut<'data> {
        type Iter: Iterator<Item = &'data mut [Self::Item]>;
        type Item: 'data;
        fn par_chunks_mut(&'data mut self, size: usize) -> Self::Iter;
    }

    impl<'data, T: 'data + Send> ParChunksMut<'data> for [T] {
        type Iter = std::slice::ChunksMut<'data, T>;
        type Item = T;
        fn par_chunks_mut(&'data mut self, size: usize) -> Self::Iter {
            self.chunks_mut(size.max(1))
        }
    }

    impl<'data, T: 'data + Send> ParChunksMut<'data> for Vec<T> {
        type Iter = std::slice::ChunksMut<'data, T>;
        type Item = T;
        fn par_chunks_mut(&'data mut self, size: usize) -> Self::Iter {
            self[..].chunks_mut(size.max(1))
        }
    }

    // Ranges: dexdec iterates ranges in parallel occasionally
    impl IntoParallelIterator for std::ops::Range<usize> {
        type Item = usize;
        type Iter = std::ops::Range<usize>;
        fn into_par_iter(self) -> Self::Iter {
            self
        }
    }

    impl IntoParallelIterator for std::ops::Range<u32> {
        type Item = u32;
        type Iter = std::ops::Range<u32>;
        fn into_par_iter(self) -> Self::Iter {
            self
        }
    }

    impl IntoParallelIterator for std::ops::Range<i32> {
        type Item = i32;
        type Iter = std::ops::Range<i32>;
        fn into_par_iter(self) -> Self::Iter {
            self
        }
    }

    impl IntoParallelIterator for std::ops::Range<i64> {
        type Item = i64;
        type Iter = std::ops::Range<i64>;
        fn into_par_iter(self) -> Self::Iter {
            self
        }
    }

    /// opt-in marker mirroring rayon::ParallelBridge as sequential
    pub trait ParallelBridge: Iterator + Sized {
        fn par_bridge(self) -> Self {
            self
        }
    }

    impl<T: Iterator> ParallelBridge for T {}

    /// rayon adapter surface that is a no-op sequentially:
    /// `flat_map_iter` maps to `flat_map`, `with_min_len` is identity.
    pub trait ParIterExt: Iterator + Sized {
        fn flat_map_iter<F, U>(self, f: F) -> std::iter::FlatMap<Self, U, F>
        where
            F: FnMut(Self::Item) -> U,
            U: IntoIterator,
        {
            self.flat_map(f)
        }

        fn with_min_len(self, _min: usize) -> Self {
            self
        }

        fn with_max_len(self, _max: usize) -> Self {
            self
        }
    }

    impl<I: Iterator> ParIterExt for I {}

    // ---- collections: HashMap / BTreeMap / HashSet / BTreeSet ----

    impl<'data, K: 'data, V: 'data> IntoParallelRefIterator<'data> for HashMap<K, V> {
        type Iter = std::collections::hash_map::Iter<'data, K, V>;
        type Item = (K, V);
        fn par_iter(&'data self) -> Self::Iter {
            self.iter()
        }
    }

    impl<'data, K: 'data, V: 'data> IntoParallelRefIterator<'data> for BTreeMap<K, V> {
        type Iter = std::collections::btree_map::Iter<'data, K, V>;
        type Item = (K, V);
        fn par_iter(&'data self) -> Self::Iter {
            self.iter()
        }
    }

    impl<'data, T: 'data> IntoParallelRefIterator<'data> for HashSet<T> {
        type Iter = std::collections::hash_set::Iter<'data, T>;
        type Item = T;
        fn par_iter(&'data self) -> Self::Iter {
            self.iter()
        }
    }

    impl<'data, T: 'data> IntoParallelRefIterator<'data> for BTreeSet<T> {
        type Iter = std::collections::btree_set::Iter<'data, T>;
        type Item = T;
        fn par_iter(&'data self) -> Self::Iter {
            self.iter()
        }
    }

    impl<'data, K: 'data, V: 'data> IntoParallelRefMutIterator<'data> for HashMap<K, V> {
        type Iter = std::collections::hash_map::IterMut<'data, K, V>;
        type Item = (K, V);
        fn par_iter_mut(&'data mut self) -> Self::Iter {
            self.iter_mut()
        }
    }

    impl<'data, K: 'data, V: 'data> IntoParallelRefMutIterator<'data> for BTreeMap<K, V> {
        type Iter = std::collections::btree_map::IterMut<'data, K, V>;
        type Item = (K, V);
        fn par_iter_mut(&'data mut self) -> Self::Iter {
            self.iter_mut()
        }
    }

    impl<K, V> IntoParallelIterator for HashMap<K, V> {
        type Item = (K, V);
        type Iter = std::collections::hash_map::IntoIter<K, V>;
        fn into_par_iter(self) -> Self::Iter {
            self.into_iter()
        }
    }

    impl<K, V> IntoParallelIterator for BTreeMap<K, V> {
        type Item = (K, V);
        type Iter = std::collections::btree_map::IntoIter<K, V>;
        fn into_par_iter(self) -> Self::Iter {
            self.into_iter()
        }
    }

    impl<T> IntoParallelIterator for HashSet<T> {
        type Item = T;
        type Iter = std::collections::hash_set::IntoIter<T>;
        fn into_par_iter(self) -> Self::Iter {
            self.into_iter()
        }
    }

    impl<T> IntoParallelIterator for BTreeSet<T> {
        type Item = T;
        type Iter = std::collections::btree_set::IntoIter<T>;
        fn into_par_iter(self) -> Self::Iter {
            self.into_iter()
        }
    }

    use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
}
