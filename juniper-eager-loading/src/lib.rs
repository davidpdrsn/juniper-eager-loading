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

#[derive(Debug, Copy, Clone)]
pub enum DbEdgeError {
    NotLoaded,
    LoadFailed,
}

impl fmt::Display for DbEdgeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DbEdgeError::NotLoaded => {
                write!(f, "`DbEdge` should have been eager loaded, but wasn't")
            }
            DbEdgeError::LoadFailed => write!(f, "Failed to load `DbEdge`"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DbEdge<T> {
    Loaded(T),
    NotLoaded,
    LoadFailed,
}

impl<T> Default for DbEdge<T> {
    fn default() -> Self {
        DbEdge::NotLoaded
    }
}

impl<T> DbEdge<T> {
    pub fn try_unwrap(&self) -> Result<&T, DbEdgeError> {
        match self {
            DbEdge::Loaded(inner) => Ok(inner),
            DbEdge::NotLoaded => Err(DbEdgeError::NotLoaded),
            DbEdge::LoadFailed => Err(DbEdgeError::LoadFailed),
        }
    }

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
    Loaded(Option<T>),
    NotLoaded,
    LoadFailed,
}

impl<T> Default for OptionDbEdge<T> {
    fn default() -> Self {
        OptionDbEdge::NotLoaded
    }
}

impl<T> OptionDbEdge<T> {
    pub fn as_ref(&self) -> OptionDbEdge<&T> {
        match self {
            OptionDbEdge::Loaded(Some(inner)) => OptionDbEdge::Loaded(Some(&inner)),
            OptionDbEdge::Loaded(None) => OptionDbEdge::Loaded(None),
            OptionDbEdge::NotLoaded => OptionDbEdge::NotLoaded,
            OptionDbEdge::LoadFailed => OptionDbEdge::LoadFailed,
        }
    }

    pub fn try_unwrap(&self) -> Result<&Option<T>, DbEdgeError> {
        match self {
            OptionDbEdge::Loaded(inner) => Ok(inner),
            OptionDbEdge::NotLoaded => Err(DbEdgeError::NotLoaded),
            OptionDbEdge::LoadFailed => Err(DbEdgeError::LoadFailed),
        }
    }

    pub fn loaded_or_failed(&mut self, inner: Option<Option<T>>) {
        if let Some(inner) = inner {
            std::mem::replace(self, OptionDbEdge::Loaded(inner));
        } else {
            std::mem::replace(self, OptionDbEdge::LoadFailed);
        }
    }
}

pub trait GraphqlNodeForModel: Sized {
    type Model;
    type Id: Clone;
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

    fn loaded_or_missing_child(node: &mut Self, child: Option<&Child>);

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
            Self::loaded_or_missing_child(node, child);
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

// TODO: Add derive for this what works with Diesel
pub trait LoadFromIds: Sized {
    type Id;
    type Error;
    type Connection;

    fn load(
        ids: &[Self::Id],
        db: &Self::Connection,
    ) -> Result<Vec<Self>, Self::Error>;
}
