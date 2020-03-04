#![allow(unused_variables, unused_imports, dead_code, unused_mut)]

mod helpers;

use assert_json_diff::{assert_json_eq, assert_json_include};
use helpers::{SortedExtension, StatsHash};
use juniper::{Executor, FieldError, FieldResult};
use juniper_eager_loading::{
    prelude::*, EagerLoading, HasMany, HasManyThrough, HasOne, OptionHasOne,
};
use juniper_from_schema::graphql_schema;
use serde_json::{json, Value};

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
        foo: Boolean!
    }

    type User {
        id: Int!
        country: Country!
        countryMaybe: Country
        countries: [Country!]!
        countriesMaybe: [Country!]!
        companies: [Company!]!
    }

    type Country {
        id: Int!
    }

    type Company {
        id: Int!
    }
}

mod models {
    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct User {
        pub own_user_id: i32,
        pub referenced_country_id: i32,
        pub referenced_country_maybe_id: Option<i32>,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Country {
        pub own_country_id: i32,
        pub referenced_user_id: i32,
        pub referenced_user_id_maybe: Option<i32>,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Employment {
        pub referenced_user_id: i32,
        pub referenced_company_id: i32,
    }

    #[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
    pub struct Company {
        pub own_company_id: i32,
    }

    impl juniper_eager_loading::LoadFrom<i32> for Country {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[i32], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            todo!()
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Country {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[User], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            todo!()
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Employment {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[User], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            todo!()
        }
    }

    impl juniper_eager_loading::LoadFrom<Employment> for Company {
        type Error = Box<dyn std::error::Error>;
        type Context = super::Context;

        fn load(ids: &[Employment], _: &(), ctx: &Self::Context) -> Result<Vec<Self>, Self::Error> {
            todo!()
        }
    }
}

pub struct Context;

impl juniper::Context for Context {}

pub struct Query;

impl QueryFields for Query {
    // Query has to have at least one field
    fn field_foo<'a>(&self, executor: &Executor<'a, Context>) -> FieldResult<&bool> {
        todo!()
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(
    error = Box<dyn std::error::Error>,
    context = Context,
    primary_key_field = own_user_id,
)]
pub struct User {
    user: models::User,

    #[has_one(
        child_primary_key_field = own_country_id,
        foreign_key_field = referenced_country_id,
    )]
    country: HasOne<Country>,

    #[option_has_one(
        child_primary_key_field = own_country_id,
        foreign_key_field = referenced_country_maybe_id,
        root_model_field = country,
    )]
    country_maybe: OptionHasOne<Country>,

    #[has_many(
        root_model_field = country,
        foreign_key_field = referenced_user_id,
    )]
    countries: HasMany<Country>,

    #[has_many(
        root_model_field = country,
        foreign_key_optional,
        foreign_key_field = referenced_user_id_maybe,
    )]
    countries_maybe: HasMany<Country>,

    #[has_many_through(
        join_model = models::Employment,
        child_primary_key_field = own_company_id,
        foreign_key_field = referenced_user_id,
        child_primary_key_field_on_join_model = referenced_company_id,
    )]
    companies: HasManyThrough<Company>,
}

impl UserFields for User {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        todo!()
    }

    fn field_country(
        &self,
        _executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Country> {
        todo!()
    }

    fn field_country_maybe(
        &self,
        _executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Option<Country>> {
        todo!()
    }

    fn field_countries(
        &self,
        _executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Vec<Country>> {
        todo!()
    }

    fn field_countries_maybe(
        &self,
        _executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Country, Walked>,
    ) -> FieldResult<&Vec<Country>> {
        todo!()
    }

    fn field_companies(
        &self,
        _executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<&Vec<Company>> {
        todo!()
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(
    error = Box<dyn std::error::Error>,
    context = Context,
)]
pub struct Country {
    country: models::Country,
}

impl CountryFields for Country {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        todo!()
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, EagerLoading)]
#[eager_loading(
    error = Box<dyn std::error::Error>,
    context = Context,
)]
pub struct Company {
    company: models::Company,
}

impl CompanyFields for Company {
    fn field_id(&self, _executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        todo!()
    }
}
