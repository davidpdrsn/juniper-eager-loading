#![allow(unused_variables, unused_imports, dead_code, unused_mut)]
#![allow(clippy::type_complexity)]

mod helpers;

use assert_json_diff::{assert_json_eq, assert_json_include};
use helpers::{SortedExtension, StatsHash};
use juniper::{Executor, FieldError, FieldResult};
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
      mutation: Mutation
    }

    type Query {
      countries: [Country!]! @juniper(ownership: "owned")
    }

    type Mutation {
      noop: Boolean!
    }

    type User {
        id: Int!
        isAdmin: Boolean!
        country: Country!
    }

    type Country {
        id: Int!
        users(onlyAdmins: Boolean!): [User!]!
    }
}

mod models {
    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
        pub admin: bool,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Country {
        pub id: i32,
    }

    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let mut models = db
                .countries
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            models.sort_by_key(|model| model.id);
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<i32> for User {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(ids: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let mut models = db
                .users
                .all_values()
                .into_iter()
                .filter(|value| ids.contains(&value.id))
                .cloned()
                .collect::<Vec<_>>();
            models.sort_by_key(|model| model.id);
            Ok(models)
        }
    }

    impl juniper_eager_loading::LoadFrom<Country> for User {
        type Error = Box<dyn std::error::Error>;
        type Connection = super::Db;

        fn load(countries: &[Country], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
            let country_ids = countries.iter().map(|c| c.id).collect::<Vec<_>>();
            let mut models = db
                .users
                .all_values()
                .into_iter()
                .filter(|user| country_ids.contains(&user.country_id))
                .cloned()
                .collect::<Vec<_>>();
            models.sort_by_key(|model| model.id);
            Ok(models)
        }
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
    fn field_countries(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<Vec<Country>> {
        let db = &executor.context().db;

        let mut country_models = db
            .countries
            .all_values()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        country_models.sort_by_key(|country| country.id);

        let mut countries = Country::from_db_models(&country_models);
        Country::eager_load_all_children_for_each(&mut countries, &country_models, db, trail)?;

        Ok(countries)
    }
}

pub struct Mutation;

impl MutationFields for Mutation {
    fn field_noop(&self, _executor: &Executor<'_, Context>) -> FieldResult<&bool> {
        Ok(&true)
    }
}

// The default values are commented out
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct User {
    user: models::User,
    country: HasOne<Country>,
}

impl UserFields for User {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_is_admin(&self, _executor: &Executor<'_, Context>) -> FieldResult<&bool> {
        Ok(&self.user.admin)
    }

    fn field_country(
        &self,
        _executor: &Executor<'_, Context>,
        _trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        Ok(self.country.try_unwrap()?)
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct Country {
    country: models::Country,
    users: HasMany<User>,
}

impl CountryFields for Country {
    fn field_users(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
        _only_admins: bool,
    ) -> FieldResult<&Vec<User>> {
        Ok(self.users.try_unwrap()?)
    }

    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }
}

impl juniper_eager_loading::GraphqlNodeForModel for User {
    type Model = models::User;
    type Id = i32;
    type Connection = Db;
    type Error = Box<dyn std::error::Error>;
    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: std::clone::Clone::clone(model),
            country: Default::default(),
        }
    }
}

#[allow(missing_docs, dead_code)]
struct EagerLoadingContextUserForCountry;

impl<'look_ahead, 'query_trail>
    EagerLoadChildrenOfType<
        'look_ahead,
        'query_trail,
        Country,
        EagerLoadingContextUserForCountry,
        (),
    > for User
{
    type ChildId = <Country as juniper_eager_loading::GraphqlNodeForModel>::Id;
    type FieldArguments = ();

    fn child_ids(
        models: &[Self::Model],
        db: &Self::Connection,
    ) -> Result<
        juniper_eager_loading::LoadResult<
            Self::ChildId,
            (
                <Country as juniper_eager_loading::GraphqlNodeForModel>::Model,
                (),
            ),
        >,
        Self::Error,
    > {
        let ids = models
            .iter()
            .map(|model| model.country_id.clone())
            .collect::<Vec<_>>();
        let ids = juniper_eager_loading::unique(ids);
        Ok(juniper_eager_loading::LoadResult::Ids(ids))
    }

    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
    ) -> Result<Vec<<Country as juniper_eager_loading::GraphqlNodeForModel>::Model>, Self::Error>
    {
        juniper_eager_loading::LoadFrom::load(&ids, db)
    }

    fn is_child_of(node: &Self, child: &(Country, &())) -> bool {
        node.user.country_id == (child.0).country.id
    }

    fn loaded_child(node: &mut Self, child: Country) {
        node.country.loaded(child)
    }

    fn assert_loaded_otherwise_failed(node: &mut Self) {
        node.country.assert_loaded_otherwise_failed();
    }
}

impl juniper_eager_loading::EagerLoadAllChildren for User {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &juniper_from_schema::QueryTrail<'_, Self, juniper_from_schema::Walked>,
    ) -> Result<(), Self::Error> {
        if let Some(trail) = trail.country().walk() {
            EagerLoadChildrenOfType::<Country, EagerLoadingContextUserForCountry, _>::eager_load_children(
                nodes,
                models,
                db,
                &trail,
                &(),
            )?;
        }
        Ok(())
    }
}

impl juniper_eager_loading::GraphqlNodeForModel for Country {
    type Model = models::Country;
    type Id = i32;
    type Connection = Db;
    type Error = Box<dyn std::error::Error>;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            country: std::clone::Clone::clone(model),
            users: Default::default(),
        }
    }
}

#[allow(missing_docs, dead_code)]
struct EagerLoadingContextCountryForUsers;

impl<'look_ahead: 'query_trail, 'query_trail>
    EagerLoadChildrenOfType<'look_ahead, 'query_trail, User, EagerLoadingContextCountryForUsers, ()>
    for Country
{
    type ChildId = Vec<<User as juniper_eager_loading::GraphqlNodeForModel>::Id>;
    type FieldArguments = CountryUsersArgs<'query_trail, 'look_ahead>;

    #[allow(unused_variables)]
    fn child_ids(
        models: &[Self::Model],
        db: &Self::Connection,
    ) -> Result<
        juniper_eager_loading::LoadResult<
            Self::ChildId,
            (
                <User as juniper_eager_loading::GraphqlNodeForModel>::Model,
                (),
            ),
        >,
        Self::Error,
    > {
        let child_models = juniper_eager_loading::LoadFrom::load(&models, db)?;
        let child_models = child_models.into_iter().map(|child| (child, ())).collect();
        Ok(juniper_eager_loading::LoadResult::Models(child_models))
    }

    fn load_children(
        ids: &[Self::ChildId],
        db: &Self::Connection,
    ) -> Result<Vec<<User as juniper_eager_loading::GraphqlNodeForModel>::Model>, Self::Error> {
        let ids = ids.iter().flatten().cloned().collect::<Vec<_>>();
        let ids = juniper_eager_loading::unique(ids);
        juniper_eager_loading::LoadFrom::load(&ids, db)
    }

    fn is_child_of(node: &Self, child: &(User, &())) -> bool {
        node.country.id == (child.0).user.country_id
    }

    fn loaded_child(node: &mut Self, child: User) {
        node.users.loaded(child)
    }

    fn assert_loaded_otherwise_failed(node: &mut Self) {
        node.users.assert_loaded_otherwise_failed();
    }
}

impl juniper_eager_loading::EagerLoadAllChildren for Country {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        db: &Self::Connection,
        trail: &juniper_from_schema::QueryTrail<'_, Country, juniper_from_schema::Walked>,
    ) -> Result<(), Self::Error> {
        if let Some(child_trail) = trail.users().walk() {
            let args = trail.users_args();

            EagerLoadChildrenOfType::<User, EagerLoadingContextCountryForUsers, _>::eager_load_children(
                nodes,
                models,
                db,
                &child_trail,
                &args,
            )?;
        }
        Ok(())
    }
}

#[test]
fn loading_user() {
    let mut countries = StatsHash::new("countries");
    let mut users = StatsHash::new("users");

    let mut country = models::Country { id: 10 };
    let country_id = country.id;

    countries.insert(country_id, country.clone());

    let bob = models::User {
        id: 1,
        country_id,
        admin: true,
    };
    let alice = models::User {
        id: 2,
        country_id,
        admin: false,
    };
    users.insert(bob.id, bob.clone());
    users.insert(alice.id, alice.clone());

    let db = Db { users, countries };
    let (json, counts) = run_query(
        r#"
        query Test {
            countries {
                id
                users(onlyAdmins: true) {
                    id
                    isAdmin
                    country {
                        id
                    }
                }
            }
        }
    "#,
        db,
    );

    assert_eq!(1, counts.user_reads);
    assert_eq!(2, counts.country_reads);

    assert_json_eq!(
        json!({
            "countries": [
                {
                    "id": country.id,
                    "users": [
                        {
                            "id": bob.id,
                            "isAdmin": true,
                            "country": { "id": country.id }
                        },
                        {
                            "id": alice.id,
                            "isAdmin": false,
                            "country": { "id": country.id }
                        },
                    ]
                }
            ],
        }),
        json,
    );
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
        &Schema::new(Query, Mutation),
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
