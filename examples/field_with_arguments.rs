#![allow(unused_variables, unused_imports, dead_code)]

#[macro_use]
extern crate diesel;

use chrono::prelude::*;
use juniper::{Executor, FieldResult};
use juniper_eager_loading::{prelude::*, EagerLoading, HasMany};
use juniper_from_schema::graphql_schema;
use std::{error::Error, pin::Pin};

// the examples all use Diesel, but this library is data store agnostic
use diesel::prelude::*;

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
      countries: [Country!]! @juniper(ownership: "owned", async: true)
    }

    type User {
        id: Int!
    }

    type Country {
        id: Int!
        users(activeSince: DateTimeUtc!): [User!]!
    }

    scalar DateTimeUtc
}

mod db_schema {
    table! {
        users {
            id -> Integer,
            country_id -> Integer,
            active_since -> Timestamptz,
        }
    }

    table! {
        countries {
            id -> Integer,
        }
    }
}

mod models {
    use super::CountryUsersArgs;
    use chrono::prelude::*;
    use diesel::prelude::*;

    #[derive(Clone, Debug, Queryable)]
    pub struct User {
        pub id: i32,
        pub country_id: i32,
        pub active_since: DateTime<Utc>,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Country {
        pub id: i32,
    }

    #[async_trait::async_trait]
    impl<'a> juniper_eager_loading::LoadFrom<Country, CountryUsersArgs<'a>> for User {
        type Error = diesel::result::Error;
        type Context = super::Context;

        async fn load(
            countries: &[Country],
            field_args: &CountryUsersArgs<'a>,
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::users::dsl::*;
            use diesel::pg::expression::dsl::any;

            let country_ids = countries
                .iter()
                .map(|country| country.id)
                .collect::<Vec<_>>();

            users
                .filter(country_id.eq(any(country_ids)))
                .filter(active_since.gt(&field_args.active_since()))
                .load::<User>(&*ctx.db.lock().unwrap())
        }
    }
}

pub struct Query;

#[async_trait::async_trait]
impl QueryFields for Query {
    async fn field_countries<'s, 'r, 'a>(
        &'s self,
        executor: &Executor<'r, 'a, Context>,
        trail: &QueryTrail<'r, Country, Walked>,
    ) -> FieldResult<Vec<Country>> {
        let ctx = executor.context();
        let country_models =
            db_schema::countries::table.load::<models::Country>(&*ctx.db.lock().unwrap())?;
        let country = Country::eager_load_each(&country_models, ctx, trail).await?;

        Ok(country)
    }
}

pub struct Context {
    db: std::sync::Mutex<PgConnection>,
}

impl juniper::Context for Context {}

#[derive(Clone)]
// #[derive(EagerLoading)]
// #[eager_loading(context = Context, error = diesel::result::Error)]
pub struct User {
    user: models::User,
}

impl UserFields for User {
    fn field_id<'s, 'r, 'a>(&'s self, executor: &Executor<'r, 'a, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }
}

#[derive(Clone)]
// #[derive(EagerLoading)]
// #[eager_loading(context = Context, error = diesel::result::Error)]
pub struct Country {
    country: models::Country,

    // #[has_many(
    //     root_model_field = user,
    //     field_arguments = CountryUsersArgs,
    //  // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ The important line
    // )]
    users: HasMany<User>,
}

impl CountryFields for Country {
    fn field_id(&self, executor: &Executor<Context>) -> FieldResult<&i32> {
        Ok(&self.country.id)
    }

    fn field_users<'r, 'a>(
        &self,
        executor: &Executor<'r, 'a, Context>,
        trail: &QueryTrail<'r, User, Walked>,
        _active_since: DateTime<Utc>,
    ) -> FieldResult<&Vec<User>> {
        self.users.try_unwrap().map_err(From::from)
    }
}

fn main() {}

#[async_trait::async_trait]
impl<'a> juniper_eager_loading::EagerLoading<'a> for User {
    type Model = models::User;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;
    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: std::clone::Clone::clone(model),
        }
    }
    async fn eager_load_each(
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &juniper_from_schema::QueryTrail<'_, Self, juniper_from_schema::Walked>,
    ) -> Result<Vec<Self>, Self::Error> {
        let mut nodes = Self::from_db_models(models);
        Ok(nodes)
    }
}
#[async_trait::async_trait]
impl<'a> juniper_eager_loading::EagerLoading<'a> for Country {
    type Model = models::Country;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;
    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            country: std::clone::Clone::clone(model),
            users: std::default::Default::default(),
        }
    }
    async fn eager_load_each(
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &juniper_from_schema::QueryTrail<'_, Self, juniper_from_schema::Walked>,
    ) -> Result<Vec<Self>, Self::Error> {
        let mut nodes = Self::from_db_models(models);
        if let Some(child_trail) = trail.users().walk() {
            let field_args = trail.users_args();
            EagerLoadChildrenOfType ::
            < User, EagerLoadingContextCountryForUsers, _ > ::
            eager_load_children(& mut nodes, models, & ctx, & child_trail, &
                                field_args,) . await ? ;
        }
        Ok(nodes)
    }
}
#[allow(missing_docs, dead_code)]
struct EagerLoadingContextCountryForUsers;
#[async_trait::async_trait]
impl<'a>
    juniper_eager_loading::EagerLoadChildrenOfType<'a, User, EagerLoadingContextCountryForUsers, ()>
    for Country
{
    type FieldArguments = CountryUsersArgs<'a>;
    #[allow(unused_variables)]
    fn load_children(
        models: &[<Self as juniper_eager_loading::EagerLoading<'a>>::Model],
        field_args: &Self::FieldArguments,
        ctx: &<Self as juniper_eager_loading::EagerLoading<'a>>::Context,
    ) -> Pin<
        Box<
            dyn std::future::Future<
                Output = Result<
                    juniper_eager_loading::LoadChildrenOutput<
                        <User as juniper_eager_loading::EagerLoading<'a>>::Model,
                        (),
                    >,
                    <Self as juniper_eager_loading::EagerLoading<'a>>::Error,
                >,
            > + 'a + Send,
        >,
    > {
        Box::pin(async move {
            let child_models: Vec<<User as juniper_eager_loading::EagerLoading<'a>>::Model> =
                juniper_eager_loading::LoadFrom::load(&models, field_args, ctx).await?;
            Ok(juniper_eager_loading::LoadChildrenOutput::ChildModels(
                child_models,
            ))
        })
    }
    fn is_child_of(
        node: &Self,
        child: &User,
        join_model: &(),
        _field_args: &Self::FieldArguments,
        context: &<Self as juniper_eager_loading::EagerLoading<'a>>::Context,
    ) -> bool {
        node.country.id == child.user.country_id
    }
    fn association(node: &mut Self) -> &mut dyn juniper_eager_loading::Association<User> {
        &mut node.users
    }
}
