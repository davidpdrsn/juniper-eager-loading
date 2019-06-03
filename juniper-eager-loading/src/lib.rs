#![deny(
    // missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    unused_must_use
)]

use juniper_from_schema::Walked;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use juniper_eager_loading_code_gen::EagerLoading;

/// Helpers related to Diesel. If you don't use Diesel you can ignore this.
pub mod diesel {
    pub use crate::impl_LoadFromModels;
    pub use juniper_eager_loading_code_gen::{LoadFromIds, LoadFromModels};
}

#[macro_export]
macro_rules! impl_LoadFromModels {
    (
        error = $error:path,
        connection = $connection:path,

        $(
            $from:ident -> $to:ident
            ( $model_key:ident -> $table:ident . $foreign_key:ident ),
        )*
    ) => {
        $(
            impl juniper_eager_loading::LoadFromModels<$from> for $to {
                type Error = $error;
                type Connection = $connection;

                fn load(
                    models: &[$from],
                    db: &Self::Connection,
                ) -> Result<Vec<$to>, Self::Error> {
                    use diesel::pg::expression::dsl::any;

                    let model_ids = models
                        .iter()
                        .map(|model| model.$model_key)
                        .collect::<Vec<_>>();

                    let res = $table::table
                        .filter($table::$foreign_key.eq(any(model_ids)))
                        .load::<$to>(db)?;

                    Ok(res)
                }
            }
        )*
    }
}

/// Re-exports the traits needed for doing eager loading. Meant to be glob imported.
pub mod prelude {
    pub use super::EagerLoadAllChildren;
    pub use super::EagerLoadChildrenOfType;
    pub use super::GraphqlNodeForModel;
}

#[derive(Debug, Clone)]
pub enum HasOne<T> {
    /// The associated value was loaded.
    Loaded(T),

    /// The associated value has not yet been loaded.
    NotLoaded,

    /// The associated value should have been loaded, but something went wrong.
    LoadFailed,
}

/// Defaults to `HasOne::NotLoaded`
impl<T> Default for HasOne<T> {
    fn default() -> Self {
        HasOne::NotLoaded
    }
}

impl<T> HasOne<T> {
    /// Borrow the loaded value or get an error if something went wrong.
    pub fn try_unwrap(&self) -> Result<&T, Error> {
        match self {
            HasOne::Loaded(inner) => Ok(inner),
            HasOne::NotLoaded => Err(Error::NotLoaded),
            HasOne::LoadFailed => Err(Error::LoadFailed),
        }
    }

    /// Assign some potentially loaded value.
    ///
    /// If `inner` is a `Some` it will change `self` to `HasOne::Loaded`, otherwise
    /// `HasOne::LoadFailed`.
    pub fn loaded_or_failed(&mut self, inner: Option<T>) {
        if let Some(inner) = inner {
            std::mem::replace(self, HasOne::Loaded(inner));
        } else {
            std::mem::replace(self, HasOne::LoadFailed);
        }
    }
}

#[derive(Debug, Clone)]
pub enum OptionHasOne<T> {
    /// The associated value was loaded.
    Loaded(Option<T>),

    /// The associated value has not yet been loaded.
    NotLoaded,
}

impl<T> Default for OptionHasOne<T> {
    fn default() -> Self {
        OptionHasOne::NotLoaded
    }
}

impl<T> OptionHasOne<T> {
    /// Borrow the loaded value or get an error if something went wrong.
    pub fn try_unwrap(&self) -> Result<&Option<T>, Error> {
        match self {
            OptionHasOne::Loaded(inner) => Ok(inner),
            OptionHasOne::NotLoaded => Err(Error::NotLoaded),
        }
    }

    /// Assign some potentially loaded value.
    ///
    /// If `inner` is a `Some` it will change `self` to `OptionHasOne::Loaded(Some(_))`, otherwise
    /// `OptionHasOne::Loaded(None)`. This means it ignores loads that failed.
    pub fn loaded_or_failed(&mut self, inner: Option<T>) {
        std::mem::replace(self, OptionHasOne::Loaded(inner));
    }
}

impl<T> Default for HasMany<T> {
    fn default() -> Self {
        HasMany::NotLoaded
    }
}

#[derive(Debug, Clone)]
pub enum HasMany<T> {
    /// The associated values were loaded.
    Loaded(Vec<T>),

    /// The associated values has not yet been loaded.
    NotLoaded,
}

impl<T> HasMany<T> {
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        match self {
            HasMany::Loaded(inner) => Ok(inner),
            HasMany::NotLoaded => Err(Error::NotLoaded),
        }
    }

    pub fn loaded_or_failed(&mut self, inner: Option<T>) {
        match self {
            HasMany::Loaded(models) => {
                if let Some(inner) = inner {
                    models.push(inner)
                }
            }
            HasMany::NotLoaded => {
                let loaded = if let Some(inner) = inner {
                    HasMany::Loaded(vec![inner])
                } else {
                    HasMany::Loaded(vec![])
                };
                std::mem::replace(self, loaded);
            }
        }
    }
}

pub trait GraphqlNodeForModel: Sized {
    type Model;
    type Id: 'static + Hash + Eq;
    type Connection;
    type Error;

    fn new_from_model(model: &Self::Model) -> Self;

    fn from_db_models(models: &[Self::Model]) -> Vec<Self> {
        models
            .iter()
            .map(|model| Self::new_from_model(model))
            .collect::<Vec<_>>()
    }
}

pub trait GenericQueryTrail<T, K> {}

pub trait EagerLoadChildrenOfType<Child, Q, C>
where
    Self: GraphqlNodeForModel,
    Child: GraphqlNodeForModel<
            Model = Self::ChildModel,
            Connection = Self::Connection,
            Error = Self::Error,
            Id = Self::Id,
        > + EagerLoadAllChildren<Q>,
    Q: GenericQueryTrail<Child, Walked>,
{
    type ChildModel;
    type ChildId: Hash + Eq;

    fn child_ids(
        models: &[Self::Model],
        db: &Self::Connection,
    ) -> Result<LoadResult<Self::ChildModel, Self::ChildId>, Self::Error>;

    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
    ) -> Result<Vec<Self::ChildModel>, Self::Error>;

    fn is_child_of(node: &Self, child: &Child) -> bool;

    fn loaded_or_failed_child(node: &mut Self, child: Option<&Child>);

    fn load_from_cache(
        ids: &[Self::ChildId],
        cache: &Cache<Self::Id>,
    ) -> Vec<CacheLoadResult<Self::ChildModel, Self::ChildId>>;

    fn store_in_cache(child: &Self::ChildModel, cache: &mut Cache<Self::Id>);

    fn eager_load_children(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &Q,
        cache: &mut Cache<Self::Id>,
    ) -> Result<(), Self::Error> {
        let child_models = match Self::child_ids(models, db)? {
            LoadResult::Models(models) => {
                for model in &models {
                    Self::store_in_cache(model, cache);
                }
                models
            }

            LoadResult::Ids(child_ids) => {
                let cached_child_models = Self::load_from_cache(&child_ids, &cache);
                let mut child_models = vec![];
                let mut ids_not_in_cache = vec![];
                for result in cached_child_models {
                    match result {
                        CacheLoadResult::Loaded(model) => child_models.push(model),
                        CacheLoadResult::Missing(id) => ids_not_in_cache.push(id),
                    }
                }
                ids_not_in_cache = unique(ids_not_in_cache);

                if !ids_not_in_cache.is_empty() {
                    let loaded_models = Self::load_children(&ids_not_in_cache, db)?;
                    for model in &loaded_models {
                        Self::store_in_cache(model, cache);
                    }
                    child_models.extend(loaded_models);
                }

                child_models
            }
        };

        let mut children = child_models
            .iter()
            .map(|child_model| Child::new_from_model(child_model))
            .collect::<Vec<_>>();

        Child::eager_load_all_children_for_each(&mut children, &child_models, db, trail, cache)?;

        for node in nodes {
            let child = children
                .iter()
                .find(|child_model| Self::is_child_of(node, child_model));
            Self::loaded_or_failed_child(node, child);
        }

        Ok(())
    }
}

fn unique<T: Hash + Eq>(ts: Vec<T>) -> Vec<T> {
    use std::collections::HashSet;
    let set = ts.into_iter().collect::<HashSet<_>>();
    set.into_iter().collect()
}

#[derive(Debug)]
pub enum LoadResult<A, B> {
    Models(Vec<A>),
    Ids(Vec<B>),
}

#[derive(Debug)]
pub enum CacheLoadResult<A, B> {
    Loaded(A),
    Missing(B),
}

pub trait EagerLoadAllChildren<Q>
where
    Self: GraphqlNodeForModel,
{
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &Q,
        cache: &mut Cache<Self::Id>,
    ) -> Result<(), Self::Error>;

    fn eager_load_all_children_for_each_without_cache(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &Q,
    ) -> Result<(), Self::Error> {
        let mut cache = Cache::disabled();
        Self::eager_load_all_children_for_each(nodes, models, db, trail, &mut cache)
    }

    fn eager_load_all_chilren(
        node: Self,
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &Q,
        cache: &mut Cache<Self::Id>,
    ) -> Result<Self, Self::Error> {
        let mut nodes = vec![node];
        Self::eager_load_all_children_for_each(&mut nodes, models, db, trail, cache)?;

        // This is safe because we just made a vec with exactly one element and
        // eager_load_all_children_for_each doesn't remove things from the vec
        Ok(nodes.remove(0))
    }
}

/// Given a list of ids how should they be loaded from the data store?
///
/// If you're using Diesel and PostgreSQL this could for example be implemented using [`any`] (or
/// derived, see below).
///
/// ### `#[derive(LoadFromIds)]`
///
/// TODO
///
/// [`any`]: http://docs.diesel.rs/diesel/pg/expression/dsl/fn.any.html
pub trait LoadFromIds: Sized {
    /// The primary key type your model uses.
    ///
    /// If you're using Diesel this will normally be i32 or i64 but can be whatever you need.
    type Id;

    /// The error type the operation uses.
    ///
    /// If you're using Diesel this should be [`diesel::result::Error`].
    ///
    /// [`diesel::result::Error`]: http://docs.diesel.rs/diesel/result/enum.Error.html
    type Error;

    /// The connection type you use.
    ///
    /// If you're using Diesel this will could for example be [`PgConnection`].
    ///
    /// [`PgConnection`]: http://docs.diesel.rs/diesel/pg/struct.PgConnection.html
    type Connection;

    /// Perform the load.
    fn load(ids: &[Self::Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error>;
}

pub trait LoadFromModels<From>: Sized {
    type Error;
    type Connection;

    fn load(models: &[From], db: &Self::Connection) -> Result<Vec<Self>, Self::Error>;
}

#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub enum Error {
    NotLoaded,
    LoadFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NotLoaded => write!(f, "`HasOne` should have been eager loaded, but wasn't"),
            Error::LoadFailed => write!(f, "Failed to load `HasOne`"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub enum Cache<K: 'static + Hash + Eq> {
    #[doc(hidden)]
    NoCaching,
    #[doc(hidden)]
    Cache(CacheInner<K>),
}

impl<K: 'static + Hash + Eq> Cache<K> {
    pub fn new_from<T, F>(models: &[T], f: F) -> Self
    where
        T: 'static,
        F: Fn(&T) -> (K, T),
    {
        let mut cache = Cache::new();
        for model in models {
            let (key, value) = f(model);
            cache.insert::<T, _>(key, value);
        }
        cache
    }

    fn new() -> Self {
        Cache::Cache(CacheInner::default())
    }

    pub fn disabled() -> Self {
        Cache::NoCaching
    }

    pub fn insert<TypeKey, V>(&mut self, key: K, value: V)
    where
        TypeKey: 'static + ?Sized,
        V: 'static,
    {
        match self {
            Cache::NoCaching => {}
            Cache::Cache(cache) => cache.insert::<TypeKey, _>(key, value),
        }
    }

    pub fn get<TypeKey, V>(&self, key: K) -> Option<&V>
    where
        TypeKey: 'static + ?Sized,
        V: 'static,
    {
        match self {
            Cache::NoCaching => None,
            Cache::Cache(cache) => cache.get::<TypeKey, _>(key),
        }
    }

    pub fn hits(&self) -> usize {
        match self {
            Cache::NoCaching => 0,
            Cache::Cache(cache) => cache.hits(),
        }
    }

    pub fn misses(&self) -> usize {
        match self {
            Cache::NoCaching => 0,
            Cache::Cache(cache) => cache.misses(),
        }
    }

    pub fn hit_rate(&self) -> f32 {
        match self {
            Cache::NoCaching => 0.,
            Cache::Cache(_) => {
                let hits = self.hits() as f32;
                let misses = self.misses() as f32;
                if hits == 0. && misses == 0. {
                    0.
                } else {
                    hits / (hits + misses)
                }
            }
        }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct CacheInner<K: Hash + Eq> {
    map: DynamicCache<K>,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl<K: Hash + Eq> Default for CacheInner<K> {
    fn default() -> Self {
        CacheInner {
            map: DynamicCache::new(),
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
        }
    }
}

impl<K: Hash + Eq> CacheInner<K> {
    fn insert<TypeKey, V>(&mut self, key: K, value: V)
    where
        TypeKey: 'static + ?Sized,
        V: 'static,
    {
        self.map.insert::<TypeKey, _>(key, value)
    }

    fn get<TypeKey, V>(&self, key: K) -> Option<&V>
    where
        TypeKey: 'static + ?Sized,
        V: 'static,
    {
        let res = self.map.get::<TypeKey, _>(key);
        if res.is_some() {
            self.hits.fetch_add(1, Ordering::SeqCst);
        } else {
            self.misses.fetch_add(1, Ordering::SeqCst);
        }
        res
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::Relaxed)
    }

    fn misses(&self) -> usize {
        self.misses.load(Ordering::Relaxed)
    }
}

use std::any::{Any, TypeId};
use std::{collections::HashMap, hash::Hash};

#[derive(Debug)]
struct DynamicCache<ValueKey>(HashMap<(Box<TypeId>, ValueKey), Box<Any>>)
where
    ValueKey: Hash + Eq;

impl<ValueKey> DynamicCache<ValueKey>
where
    ValueKey: Hash + Eq,
{
    fn new() -> Self {
        Self(HashMap::new())
    }

    fn insert<TypeKey, V>(&mut self, key: ValueKey, value: V)
    where
        TypeKey: 'static + ?Sized,
        V: 'static,
    {
        let key = (Box::new(TypeId::of::<TypeKey>()), key);
        self.0.insert(key, Box::new(value));
    }

    fn get<TypeKey, V>(&self, key: ValueKey) -> Option<&V>
    where
        TypeKey: 'static + ?Sized,
        V: 'static,
    {
        let key = (Box::new(TypeId::of::<TypeKey>()), key);
        self.0.get(&key).and_then(|value| value.downcast_ref())
    }
}

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_dynamic_cache() {
        let mut cache = DynamicCache::new();

        cache.insert::<i32, _>("key", 123);
        cache.insert::<bool, _>("key", "bool value".to_string());

        assert_eq!(Some(&123), cache.get::<i32, _>("key"));
        assert_eq!(Some(&"bool value".to_string()), cache.get::<bool, _>("key"));
    }
}
