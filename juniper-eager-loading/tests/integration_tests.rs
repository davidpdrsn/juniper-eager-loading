use assert_json_diff::assert_json_eq;
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{
    DbEdge, EagerLoadAllChildren, EagerLoadChildrenOfType, GraphqlNodeForModel,
};
use juniper_from_schema::{graphql_schema, Walked};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

graphql_schema! {
    schema {
      query: Query
      mutation: Mutation
    }

    type Query {
      users: [User!]! @juniper(ownership: "owned")
    }

    type Mutation {
      noop: Boolean!
    }

    type User {
        id: Int!
        country: Country!
    }

    type Country {
        id: Int!
    }
}

mod models {
    #[derive(Clone)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
    }

    #[derive(Clone)]
    pub struct Country {
        pub id: i32,
    }
}

pub struct Db {
    users: StatsHash<i32, models::User>,
    countries: StatsHash<i32, models::Country>,
}

pub struct Context {
    db: Db,
}

impl juniper::Context for Context {}

pub struct Query;

impl QueryFields for Query {
    fn field_users<'a>(
        &self,
        executor: &Executor<'a, Context>,
        trail: &QueryTrail<'a, User, Walked>,
    ) -> FieldResult<Vec<User>> {
        let db = &executor.context().db;

        let mut user_models = db
            .users
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        user_models.sort_by_key(|user| user.id);

        let mut users = User::from_db_models(&user_models);
        User::eager_load_all_children_for_each(&mut users, &user_models, db, trail)?;

        Ok(users)
    }
}

pub struct Mutation;

impl MutationFields for Mutation {
    fn field_noop(&self, executor: &Executor<'_, Context>) -> FieldResult<&bool> {
        Ok(&true)
    }
}

#[derive(Clone)]
pub struct User {
    user: models::User,
    country: DbEdge<Country>,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_country(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

#[derive(Clone)]
pub struct Country {
    id: i32,
}

impl CountryFields for Country {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.id)
    }
}

impl<'a, T> juniper_eager_loading::GenericQueryTrail<T, juniper_from_schema::Walked>
    for QueryTrail<'a, T, juniper_from_schema::Walked>
{
}

impl juniper_eager_loading::GraphqlNodeForModel for User {
    type Model = models::User;
    type Id = i32;
    type Connection = Db;
    type Error = Box<dyn std::error::Error>;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: model.clone(),
            country: DbEdge::NotLoaded,
        }
    }
}

impl<'a> EagerLoadAllChildren<QueryTrail<'a, Self, juniper_from_schema::Walked>> for User {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrail<'a, Self, Walked>,
    ) -> Result<(), Self::Error> {
        if let Some(trail) = trail.country().walk() {
            EagerLoadChildrenOfType::<Country, _>::eager_load_children(nodes, models, db, &trail)?;
        }

        Ok(())
    }
}

impl<'a> EagerLoadChildrenOfType<Country, QueryTrail<'a, Country, Walked>> for User {
    type ChildModel = models::Country;
    type ChildId = Self::Id;

    fn child_id(model: &Self::Model) -> Self::ChildId {
        model.country_id
    }

    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
    ) -> Result<Vec<Self::ChildModel>, Self::Error> {
        let countries = db
            .countries
            .all_values()
            .into_iter()
            .filter(|country| ids.contains(&country.id))
            .cloned()
            .collect::<Vec<_>>();
        Ok(countries)
    }

    fn is_child_of(node: &Self, country: &Country) -> bool {
        node.user.country_id == country.id
    }

    fn loaded_or_missing_child(node: &mut Self, child: Option<&Country>) {
        node.country.loaded_or_failed(child.cloned())
    }
}

impl juniper_eager_loading::GraphqlNodeForModel for Country {
    type Model = models::Country;
    type Id = i32;
    type Connection = Db;
    type Error = Box<dyn std::error::Error>;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            id: model.id,
        }
    }
}

impl<'a> EagerLoadAllChildren<QueryTrail<'a, Self, juniper_from_schema::Walked>> for Country {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &QueryTrail<'a, Self, Walked>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn loading_users() {
    let (json, counts) = run_query("query Test { users { id } }");

    assert_eq!(1, counts.user_reads);

    assert_json_eq!(
        json!({
            "users": [
                { "id": 1 },
                { "id": 2 },
                { "id": 3 },
            ]
        }),
        json,
    );
}

#[test]
fn loading_users_and_countries() {
    let (json, counts) = run_query("query Test { users { id country { id } } }");

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);

    assert_json_eq!(
        json!({
            "users": [
                { "id": 1, "country": { "id": 1 } },
                { "id": 2, "country": { "id": 1 } },
                { "id": 3, "country": { "id": 1 } },
            ]
        }),
        json,
    );
}

struct DbStats {
    user_reads: usize,
    country_reads: usize,
}

fn run_query(query: &str) -> (Value, DbStats) {
    let mut countries = StatsHash::default();
    let country = models::Country { id: 1 };
    let country_id = country.id;
    countries.insert(country_id, country);

    let mut users = StatsHash::default();
    users.insert(1, models::User { id: 1, country_id });
    users.insert(2, models::User { id: 2, country_id });
    users.insert(3, models::User { id: 3, country_id });

    let ctx = Context {
        db: Db { users, countries },
    };

    let (result, errors) = juniper::execute(
        query,
        None,
        &Schema::new(Query, Mutation),
        &juniper::Variables::new(),
        &ctx,
    )
    .unwrap();

    if !errors.is_empty() {
        panic!("GraphQL errors: {:?}", errors);
    }

    let json: Value = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();

    println!("{}", serde_json::to_string_pretty(&json).unwrap());

    (
        json,
        DbStats {
            user_reads: ctx.db.users.reads_count(),
            country_reads: ctx.db.countries.reads_count(),
        },
    )
}

struct StatsHash<K: Hash + Eq, V>(HashMap<K, V>, AtomicUsize);

impl<K: Hash + Eq, V> Default for StatsHash<K, V> {
    fn default() -> Self {
        StatsHash(HashMap::default(), AtomicUsize::default())
    }
}

impl<K: Hash + Eq, V> StatsHash<K, V> {
    fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.increment_reads_count();
        self.0.get(k)
    }

    fn all_values(&self) -> Vec<&V> {
        self.increment_reads_count();
        self.0.iter().map(|(_, v)| v).collect()
    }

    fn reads_count(&self) -> usize {
        self.1.load(Ordering::SeqCst)
    }

    fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.0.insert(k, v)
    }

    fn increment_reads_count(&self) {
        self.1.fetch_add(1, Ordering::SeqCst);
    }
}
