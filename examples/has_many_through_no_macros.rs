#![allow(unused_variables, unused_imports, dead_code)]
#![allow(clippy::let_unit_value)]

#[macro_use]
extern crate diesel;

use juniper::{Executor, FieldResult};
use juniper_eager_loading::{
    prelude::*, EagerLoading, HasManyThrough, LoadChildrenOutput, LoadFrom,
};
use juniper_from_schema::graphql_schema;
use std::error::Error;

// the examples all use Diesel, but this library is data store agnostic
use diesel::prelude::*;

graphql_schema! {
    schema {
      query: Query
    }

    type Query {
      users: [User!]! @juniper(ownership: "owned")
    }

    type User {
        id: Int!
        companies: [Company!]!
    }

    type Company {
        id: Int!
    }
}

mod db_schema {
    table! {
        users {
            id -> Integer,
        }
    }

    table! {
        companies {
            id -> Integer,
        }
    }

    table! {
        employments {
            id -> Integer,
            user_id -> Integer,
            company_id -> Integer,
        }
    }
}

mod models {
    use diesel::prelude::*;

    #[derive(Clone, Debug, Queryable)]
    pub struct User {
        pub id: i32,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Company {
        pub id: i32,
    }

    #[derive(Clone, Debug, Queryable)]
    pub struct Employment {
        pub id: i32,
        pub user_id: i32,
        pub company_id: i32,
    }

    impl juniper_eager_loading::LoadFrom<Employment> for Company {
        type Error = diesel::result::Error;
        type Context = super::Context;

        fn load(
            employments: &[Employment],
            _field_args: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::companies::dsl::*;
            use diesel::pg::expression::dsl::any;

            let company_ids = employments
                .iter()
                .map(|employent| employent.company_id)
                .collect::<Vec<_>>();

            companies
                .filter(id.eq(any(company_ids)))
                .load::<Company>(&ctx.db)
        }
    }

    impl juniper_eager_loading::LoadFrom<User> for Employment {
        type Error = diesel::result::Error;
        type Context = super::Context;

        fn load(
            users: &[User],
            _field_args: &(),
            ctx: &Self::Context,
        ) -> Result<Vec<Self>, Self::Error> {
            use crate::db_schema::employments::dsl::*;
            use diesel::pg::expression::dsl::any;

            let user_ids = users.iter().map(|user| user.id).collect::<Vec<_>>();

            employments
                .filter(user_id.eq(any(user_ids)))
                .load::<Employment>(&ctx.db)
        }
    }
}

pub struct Query;

impl QueryFields for Query {
    fn field_users(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, User, Walked>,
    ) -> FieldResult<Vec<User>> {
        let ctx = executor.context();
        let country_models = db_schema::users::table.load::<models::User>(&ctx.db)?;
        let mut country = User::from_db_models(&country_models);
        User::eager_load_all_children_for_each(&mut country, &country_models, ctx, trail)?;

        Ok(country)
    }
}

pub struct Context {
    db: PgConnection,
}

impl juniper::Context for Context {}

#[derive(Clone)]
pub struct User {
    user: models::User,
    companies: HasManyThrough<Company>,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.user.id)
    }

    fn field_companies(
        &self,
        executor: &Executor<'_, Context>,
        trail: &QueryTrail<'_, Company, Walked>,
    ) -> FieldResult<&Vec<Company>> {
        self.companies.try_unwrap().map_err(From::from)
    }
}

#[derive(Clone)]
pub struct Company {
    company: models::Company,
}

impl CompanyFields for Company {
    fn field_id(&self, executor: &Executor<'_, Context>) -> FieldResult<&i32> {
        Ok(&self.company.id)
    }
}

impl GraphqlNodeForModel for User {
    type Model = models::User;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            user: model.clone(),
            companies: Default::default(),
        }
    }
}

impl EagerLoadAllChildren for User {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &juniper_from_schema::QueryTrail<'_, Self, juniper_from_schema::Walked>,
    ) -> Result<(), Self::Error> {
        if let Some(child_trail) = trail.companies().walk() {
            let field_args = trail.companies_args();

            EagerLoadChildrenOfType::<
                Company,
                EagerLoadingContextUserForCompanies,
                _
            >::eager_load_children(nodes, models, ctx, &child_trail, &field_args)?;
        }

        Ok(())
    }
}

#[allow(missing_docs, dead_code)]
struct EagerLoadingContextUserForCompanies;

impl<'a>
    EagerLoadChildrenOfType<'a, Company, EagerLoadingContextUserForCompanies, models::Employment>
    for User
{
    type FieldArguments = ();

    #[allow(unused_variables)]
    fn load_children(
        models: &[Self::Model],
        field_args: &Self::FieldArguments,
        ctx: &Self::Context,
    ) -> Result<LoadChildrenOutput<models::Company, models::Employment>, Self::Error> {
        let join_models: Vec<models::Employment> = LoadFrom::load(&models, field_args, ctx)?;
        let child_models: Vec<models::Company> = LoadFrom::load(&join_models, field_args, ctx)?;

        let mut child_and_join_model_pairs = Vec::new();

        for join_model in join_models {
            for child_model in &child_models {
                if join_model.company_id == child_model.id {
                    let pair = (child_model.clone(), join_model.clone());
                    child_and_join_model_pairs.push(pair);
                }
            }
        }

        Ok(LoadChildrenOutput::ChildAndJoinModels(
            child_and_join_model_pairs,
        ))
    }

    fn is_child_of(
        node: &Self,
        child: &Company,
        join_model: &models::Employment,
        _field_args: &Self::FieldArguments,
        _ctx: &Self::Context,
    ) -> bool {
        node.user.id == join_model.user_id && join_model.company_id == child.company.id
    }

    fn association(node: &mut Self) -> &mut dyn Association<Company> {
        &mut node.companies
    }
}

impl GraphqlNodeForModel for Company {
    type Model = models::Company;
    type Id = i32;
    type Context = Context;
    type Error = diesel::result::Error;

    fn new_from_model(model: &Self::Model) -> Self {
        Self {
            company: model.clone(),
        }
    }
}

impl EagerLoadAllChildren for Company {
    fn eager_load_all_children_for_each(
        nodes: &mut [Self],
        models: &[Self::Model],
        ctx: &Self::Context,
        trail: &juniper_from_schema::QueryTrail<'_, Self, juniper_from_schema::Walked>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn main() {}
