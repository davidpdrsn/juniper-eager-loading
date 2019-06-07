#![deny(
    // missing_docs,
    dead_code,
    missing_copy_implementations,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_imports,
    unused_must_use,
    unused_qualifications,
    unused_variables,
)]

use juniper_from_schema::Walked;
use std::{fmt, hash::Hash};

pub use juniper_eager_loading_code_gen::EagerLoading;

/// Helpers related to Diesel. If you don't use Diesel you can ignore this.
pub mod diesel {
    pub use juniper_eager_loading_code_gen::LoadFrom;
}

/// Re-exports the traits needed for doing eager loading. Meant to be glob imported.
pub mod prelude {
    pub use super::EagerLoadAllChildren;
    pub use super::EagerLoadChildrenOfType;
    pub use super::GraphqlNodeForModel;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum AssociationType {
    HasOne,
    OptionHasOne,
    HasMany,
    HasManyThrough,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum HasOne<T> {
    Loaded(T),
    NotLoaded,
    LoadFailed,
}

impl<T> Default for HasOne<T> {
    fn default() -> Self {
        HasOne::NotLoaded
    }
}

impl<T> HasOne<T> {
    pub fn try_unwrap(&self) -> Result<&T, Error> {
        match self {
            HasOne::Loaded(inner) => Ok(inner),
            HasOne::NotLoaded => Err(Error::NotLoaded(AssociationType::HasOne)),
            HasOne::LoadFailed => Err(Error::LoadFailed(AssociationType::HasOne)),
        }
    }

    pub fn loaded_or_failed(&mut self, inner: T) {
        std::mem::replace(self, HasOne::Loaded(inner));
    }

    pub fn assert_loaded_otherwise_failed(&mut self) {
        match self {
            HasOne::NotLoaded => {
                std::mem::replace(self, HasOne::LoadFailed);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum OptionHasOne<T> {
    Loaded(Option<T>),
    NotLoaded,
}

impl<T> Default for OptionHasOne<T> {
    fn default() -> Self {
        OptionHasOne::NotLoaded
    }
}

impl<T> OptionHasOne<T> {
    pub fn try_unwrap(&self) -> Result<&Option<T>, Error> {
        match self {
            OptionHasOne::Loaded(inner) => Ok(inner),
            OptionHasOne::NotLoaded => Err(Error::NotLoaded(AssociationType::OptionHasOne)),
        }
    }

    pub fn loaded_or_failed(&mut self, inner: T) {
        std::mem::replace(self, OptionHasOne::Loaded(Some(inner)));
    }

    pub fn assert_loaded_otherwise_failed(&mut self) {
        match self {
            OptionHasOne::Loaded(_) => {}
            OptionHasOne::NotLoaded => {
                std::mem::replace(self, OptionHasOne::Loaded(None));
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum HasMany<T> {
    Loaded(Vec<T>),
    NotLoaded,
}

impl<T> Default for HasMany<T> {
    fn default() -> Self {
        HasMany::NotLoaded
    }
}

impl<T> HasMany<T> {
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        match self {
            HasMany::Loaded(inner) => Ok(inner),
            HasMany::NotLoaded => Err(Error::NotLoaded(AssociationType::HasMany)),
        }
    }

    pub fn loaded_or_failed(&mut self, inner: T) {
        match self {
            HasMany::Loaded(models) => models.push(inner),
            HasMany::NotLoaded => {
                let loaded = HasMany::Loaded(vec![inner]);
                std::mem::replace(self, loaded);
            }
        }
    }

    pub fn assert_loaded_otherwise_failed(&mut self) {
        match self {
            HasMany::Loaded(_) => {}
            HasMany::NotLoaded => {
                let loaded = HasMany::Loaded(vec![]);
                std::mem::replace(self, loaded);
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum HasManyThrough<T> {
    Loaded(Vec<T>),
    NotLoaded,
}

impl<T> Default for HasManyThrough<T> {
    fn default() -> Self {
        HasManyThrough::NotLoaded
    }
}

impl<T> HasManyThrough<T> {
    pub fn try_unwrap(&self) -> Result<&Vec<T>, Error> {
        match self {
            HasManyThrough::Loaded(inner) => Ok(inner),
            HasManyThrough::NotLoaded => Err(Error::NotLoaded(AssociationType::HasManyThrough)),
        }
    }

    pub fn loaded_or_failed(&mut self, inner: T) {
        match self {
            HasManyThrough::Loaded(models) => models.push(inner),
            HasManyThrough::NotLoaded => {
                let loaded = HasManyThrough::Loaded(vec![inner]);
                std::mem::replace(self, loaded);
            }
        }
    }

    pub fn assert_loaded_otherwise_failed(&mut self) {
        match self {
            HasManyThrough::Loaded(_) => {}
            HasManyThrough::NotLoaded => {
                let loaded = HasManyThrough::Loaded(vec![]);
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

pub trait EagerLoadChildrenOfType<Child, QueryTrailT, Context, JoinModel = ()>
where
    Self: GraphqlNodeForModel,
    Child: GraphqlNodeForModel<
            Model = Self::ChildModel,
            Connection = Self::Connection,
            Error = Self::Error,
            Id = Self::Id,
        > + EagerLoadAllChildren<QueryTrailT>
        + Clone,
    QueryTrailT: GenericQueryTrail<Child, Walked>,
    JoinModel: 'static + Clone + ?Sized,
{
    type ChildModel: Clone;
    type ChildId: Hash + Eq;

    fn child_ids(
        models: &[Self::Model],
        db: &Self::Connection,
    ) -> Result<LoadResult<Self::ChildId, (Self::ChildModel, JoinModel)>, Self::Error>;

    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
    ) -> Result<Vec<Self::ChildModel>, Self::Error>;

    fn is_child_of(node: &Self, child: &(Child, &JoinModel)) -> bool;

    fn loaded_or_failed_child(node: &mut Self, child: Child);

    fn assert_loaded_otherwise_failed(node: &mut Self);

    fn eager_load_children(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrailT,
    ) -> Result<(), Self::Error> {
        let child_models = match Self::child_ids(models, db)? {
            LoadResult::Ids(child_ids) => {
                assert!(same_type::<JoinModel, ()>());

                let loaded_models = Self::load_children(&child_ids, db)?;
                loaded_models
                    .into_iter()
                    .map(|model| {
                        #[allow(unsafe_code)]
                        let join_model = unsafe {
                            // This branch will only ever be called if `JoinModel` is `()`. That
                            // happens for all the `Has*` types except `HasManyThrough`.
                            //
                            // `HasManyThrough` requires something to join the two types on,
                            // therefore `child_ids` will return a variant of `LoadResult::Models`
                            std::mem::transmute_copy::<(), JoinModel>(&())
                        };

                        (model, join_model)
                    })
                    .collect::<Vec<_>>()
            }
            LoadResult::Models(model_and_join_pairs) => model_and_join_pairs,
        };

        let children = child_models
            .iter()
            .map(|child_model| (Child::new_from_model(&child_model.0), child_model.1.clone()))
            .collect::<Vec<_>>();

        let mut children_without_join_models =
            children.iter().map(|x| x.0.clone()).collect::<Vec<_>>();

        let child_models_without_join_models =
            child_models.iter().map(|x| x.0.clone()).collect::<Vec<_>>();

        let len_before = child_models_without_join_models.len();

        Child::eager_load_all_children_for_each(
            &mut children_without_join_models,
            &child_models_without_join_models,
            db,
            trail,
        )?;

        assert_eq!(len_before, child_models_without_join_models.len());

        let children = children_without_join_models
            .into_iter()
            .enumerate()
            .map(|(idx, child)| {
                let join_model = &children[idx].1;
                (child, join_model)
            })
            .collect::<Vec<_>>();

        for node in nodes {
            let matching_children = children
                .iter()
                .filter(|child_model| Self::is_child_of(node, child_model))
                .cloned()
                .collect::<Vec<_>>();

            for child in matching_children {
                Self::loaded_or_failed_child(node, child.0);
            }

            Self::assert_loaded_otherwise_failed(node);
        }

        Ok(())
    }
}

fn same_type<A: 'static, B: 'static>() -> bool {
    use std::any::TypeId;
    TypeId::of::<A>() == TypeId::of::<B>()
}

#[derive(Debug)]
pub enum LoadResult<A, B> {
    Ids(Vec<A>),
    Models(Vec<B>),
}

pub trait EagerLoadAllChildren<QueryTrailT>
where
    Self: GraphqlNodeForModel,
{
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrailT,
    ) -> Result<(), Self::Error>;

    fn eager_load_all_chilren(
        node: Self,
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrailT,
    ) -> Result<Self, Self::Error> {
        let mut nodes = vec![node];
        Self::eager_load_all_children_for_each(&mut nodes, models, db, trail)?;

        // This is safe because we just made a vec with exactly one element and
        // eager_load_all_children_for_each doesn't remove things from the vec
        Ok(nodes.remove(0))
    }
}

pub trait LoadFrom<Id>: Sized {
    type Error;

    type Connection;

    fn load(ids: &[Id], db: &Self::Connection) -> Result<Vec<Self>, Self::Error>;
}

#[derive(Debug)]
#[allow(missing_copy_implementations)]
pub enum Error {
    NotLoaded(AssociationType),
    LoadFailed(AssociationType),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NotLoaded(kind) => {
                write!(f, "`{:?}` should have been eager loaded, but wasn't", kind)
            }
            Error::LoadFailed(kind) => write!(f, "Failed to load `{:?}`", kind),
        }
    }
}

impl std::error::Error for Error {}
