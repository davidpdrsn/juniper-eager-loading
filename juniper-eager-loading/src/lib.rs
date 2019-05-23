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

pub use juniper_eager_loading_code_gen::EagerLoading;

/// Helpers related to Diesel. If you don't use Diesel you can ignore this.
pub mod diesel {
    pub use juniper_eager_loading_code_gen::LoadFromIds;
}

/// Re-exports the traits needed for doing eager loading. Meant to be glob imported.
pub mod prelude {
    pub use super::EagerLoadAllChildren;
    pub use super::EagerLoadChildrenOfType;
    pub use super::GraphqlNodeForModel;
}

#[derive(Debug, Clone)]
pub enum DbEdge<T> {
    /// The associated value was loaded.
    Loaded(T),

    /// The associated value has not yet been loaded.
    NotLoaded,

    /// The associated value should have been loaded, but something went wrong.
    LoadFailed,
}

/// Defaults to `DbEdge::NotLoaded`
impl<T> Default for DbEdge<T> {
    fn default() -> Self {
        DbEdge::NotLoaded
    }
}

impl<T> DbEdge<T> {
    /// Borrow the loaded value or get an error if something went wrong.
    pub fn try_unwrap(&self) -> Result<&T, Error> {
        match self {
            DbEdge::Loaded(inner) => Ok(inner),
            DbEdge::NotLoaded => Err(Error::NotLoaded),
            DbEdge::LoadFailed => Err(Error::LoadFailed),
        }
    }

    /// Assign some potentially loaded value.
    ///
    /// If `inner` is a `Some` it will change `self` to `DbEdge::Loaded`, otherwise
    /// `DbEdge::LoadFailed`.
    pub fn loaded_or_failed(&mut self, inner: Option<T>) {
        if let Some(inner) = inner {
            std::mem::replace(self, DbEdge::Loaded(inner));
        } else {
            std::mem::replace(self, DbEdge::LoadFailed);
        }
    }
}

#[derive(Debug, Clone)]
pub enum OptionDbEdge<T> {
    /// The associated value was loaded.
    Loaded(Option<T>),

    /// The associated value has not yet been loaded.
    NotLoaded,
}

impl<T> Default for OptionDbEdge<T> {
    fn default() -> Self {
        OptionDbEdge::NotLoaded
    }
}

impl<T> OptionDbEdge<T> {
    /// Borrow the loaded value or get an error if something went wrong.
    pub fn try_unwrap(&self) -> Result<&Option<T>, Error> {
        match self {
            OptionDbEdge::Loaded(inner) => Ok(inner),
            OptionDbEdge::NotLoaded => Err(Error::NotLoaded),
        }
    }

    /// Assign some potentially loaded value.
    ///
    /// If `inner` is a `Some` it will change `self` to `OptionDbEdge::Loaded(Some(_))`, otherwise
    /// `OptionDbEdge::Loaded(None)`. This means it ignores loads that failed.
    pub fn loaded_or_failed(&mut self, inner: Option<T>) {
        std::mem::replace(self, OptionDbEdge::Loaded(inner));
    }
}

impl<T> Default for VecDbEdge<T> {
    fn default() -> Self {
        VecDbEdge::NotLoaded
    }
}

#[derive(Debug, Clone)]
pub enum VecDbEdge<T> {
    /// The associated values were loaded.
    Loaded(Vec<T>),

    /// The associated values has not yet been loaded.
    NotLoaded,
}

impl<T> VecDbEdge<T> {
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        match self {
            VecDbEdge::Loaded(inner) => Ok(inner),
            VecDbEdge::NotLoaded => Err(Error::NotLoaded),
        }
    }

    pub fn loaded_or_failed(&mut self, inner: Option<T>) {
        match self {
            VecDbEdge::Loaded(models) => {
                if let Some(inner) = inner {
                    models.push(inner)
                }
            }
            VecDbEdge::NotLoaded => {
                let loaded = if let Some(inner) = inner {
                    VecDbEdge::Loaded(vec![inner])
                } else {
                    VecDbEdge::Loaded(vec![])
                };
                std::mem::replace(self, loaded);
            }
        }
    }
}

pub trait GraphqlNodeForModel: Sized {
    type Model;
    type Id;
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

pub trait EagerLoadChildrenOfType<Child, Q, C = ()>
where
    Self: GraphqlNodeForModel,
    Child: GraphqlNodeForModel<
            Model = Self::ChildModel,
            Connection = Self::Connection,
            Error = Self::Error,
        > + EagerLoadAllChildren<Q>,
    Q: GenericQueryTrail<Child, Walked>,
{
    type ChildModel;
    type ChildId;

    fn child_id(child: &Self::Model) -> Self::ChildId;

    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
    ) -> Result<Vec<Self::ChildModel>, Self::Error>;

    fn is_child_of(node: &Self, child: &Child) -> bool;

    fn loaded_or_failed_child(node: &mut Self, child: Option<&Child>);

    fn eager_load_children(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &Q,
    ) -> Result<(), Self::Error> {
        let child_ids = models
            .iter()
            .map(|model| Self::child_id(model))
            .collect::<Vec<_>>();

        let child_models = Self::load_children(&child_ids, db)?;

        let mut children = child_models
            .iter()
            .map(|child_model| Child::new_from_model(child_model))
            .collect::<Vec<_>>();

        Child::eager_load_all_children_for_each(&mut children, &child_models, db, trail)?;

        for node in nodes {
            let child = children
                .iter()
                .find(|child_model| Self::is_child_of(node, child_model));
            Self::loaded_or_failed_child(node, child);
        }

        Ok(())
    }
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
    ) -> Result<(), Self::Error>;

    fn eager_load_all_chilren(
        node: Self,
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &Q,
    ) -> Result<Self, Self::Error> {
        let mut nodes = vec![node];
        Self::eager_load_all_children_for_each(&mut nodes, models, db, trail)?;

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

#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub enum Error {
    NotLoaded,
    LoadFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NotLoaded => write!(f, "`DbEdge` should have been eager loaded, but wasn't"),
            Error::LoadFailed => write!(f, "Failed to load `DbEdge`"),
        }
    }
}

impl std::error::Error for Error {}

use std::any::{Any, TypeId};
use std::{collections::HashMap, hash::Hash};

#[derive(Debug)]
pub struct DynamicCache<ValueKey: Hash + Eq>(HashMap<(Box<TypeId>, ValueKey), Box<Any>>);

impl<ValueKey: Hash + Eq> DynamicCache<ValueKey> {
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
        let mut cache = DynamicCache::<&'static str>::new();

        cache.insert::<i32, _>("key", 123);
        cache.insert::<bool, _>("key", "bool value".to_string());

        assert_eq!(Some(&123), cache.get::<i32, _>("key"));
        assert_eq!(Some(&"bool value".to_string()), cache.get::<bool, _>("key"));
    }
}
