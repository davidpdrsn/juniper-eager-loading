use assert_json_diff::{assert_json_eq, assert_json_include};
use juniper::{EmptyMutation, Executor, FieldResult, ID};
use juniper_eager_loading::{
    prelude::*, EagerLoading, HasMany, HasManyThrough, HasOne, OptionHasOne,
};
use juniper_from_schema::graphql_schema;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
      users: [User!]! @juniper(ownership: "owned")
    }

    type User {
        id: Int!
        country: Country!
    }

    type Country {
        id: ID! @juniper(ownership: "owned")
    }
}

mod models {
    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub id: i32,
        pub country_id: i64,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Country {
        pub id: i64,
    }

    impl juniper_eager_loading::LoadFrom<i32> for User {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let models = db
                .users
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<i64> for Country {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i64], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let countries = db
                .countries
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            Ok(countries)
        }
    }
}

pub struct Db {
    users: StatsHash<i32, models::User>,
    countries: StatsHash<i64, models::Country>,
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

// The default values are commented out
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(connection = "Db", error = "Box<dyn std::error::Error>")]
pub struct User {
    user: models::User,

    #[has_one(default)]
    country: HasOne<Country>,
}

impl UserFields for User {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_country(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

// #[derive(Clone, Eq, PartialEq, Debug)]
#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd, EagerLoading)]
#[eager_loading(
    model = "models::Country",
    connection = "Db",
    id = "i64",
    error = "Box<dyn std::error::Error>",
    root_model_field = "country"
)]
pub struct Country {
    country: models::Country,
}

impl CountryFields for Country {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<ID> {
        Ok(self.country.id.to_string().into())
    }
}

#[test]
fn loading_users_and_associations() {
    let mut countries = StatsHash::new("countries");
    let mut users = StatsHash::new("users");

    let country = models::Country { id: 10 };

    countries.insert(country.id, country.clone());

    users.insert(
        1,
        models::User {
            id: 1,
            country_id: country.id,
        },
    );

    let db = Db { users, countries };

    let (json, counts) = run_query(
        r#"
        query Test {
            users {
                id
                country {
                    id
                }
            }
        }
    "#,
        db,
    );

    assert_json_include!(
        expected: json!({
            "users": [
                {
                    "id": 1,
                    "country": {
                        "id": country.id.to_string(),
                    },
                },
            ]
        }),
        actual: json.clone(),
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(1, counts.country_reads);
}

struct DbStats {
    user_reads: usize,
    country_reads: usize,
}

fn run_query(query: &str, db: Db) -> (Value, DbStats) {
    let ctx = Context { db };

    let (result, errors) = juniper::execute(
        query,
        None,
        &Schema::new(Query, EmptyMutation::new()),
        &juniper::Variables::new(),
        &ctx,
    )
    .unwrap();

    if !errors.is_empty() {
        panic!(
            "GraphQL errors\n{}",
            serde_json::to_string_pretty(&errors).unwrap()
        );
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

struct StatsHash<K: Hash + Eq, V> {
    map: HashMap<K, V>,
    count: AtomicUsize,
    name: &'static str,
}

impl<K: Hash + Eq, V> StatsHash<K, V> {
    fn new(name: &'static str) -> Self {
        StatsHash {
            map: HashMap::default(),
            count: AtomicUsize::default(),
            name,
        }
    }

    #[allow(dead_code)]
    fn get<Q>(&self, k: &Q) -> Option<&V>
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

    fn all_values(&self) -> Vec<&V> {
        self.increment_reads_count();
        self.map.iter().map(|(_, v)| v).collect()
    }

    fn reads_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    fn increment_reads_count(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

trait SortedExtension {
    fn sorted(self) -> Self;
}

impl<T: std::cmp::Ord> SortedExtension for Vec<T> {
    fn sorted(mut self) -> Self {
        self.sort();
        self
    }
}
