/// This macro will implement [`LoadFrom`][] for Diesel models.
///
/// [`LoadFrom`]: trait.LoadFrom.html
///
/// # Example usage
///
/// ```
/// #[macro_use]
/// extern crate diesel;
///
/// use diesel::pg::PgConnection;
/// use diesel::prelude::*;
/// use juniper_eager_loading::impl_load_from_for_diesel;
/// #
/// # fn main() {}
///
/// table! {
///     users (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     companies (id) {
///         id -> Integer,
///     }
/// }
///
/// table! {
///     employments (id) {
///         id -> Integer,
///         user_id -> Integer,
///         company_id -> Integer,
///     }
/// }
///
/// #[derive(Queryable)]
/// struct User {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Company {
///     id: i32,
/// }
///
/// #[derive(Queryable)]
/// struct Employment {
///     id: i32,
///     user_id: i32,
///     company_id: i32,
/// }
///
/// impl_load_from_for_diesel! {
///     (
///         error = diesel::result::Error,
///         connection = PgConnection,
///     ) => {
///         i32 -> (users, User),
///         i32 -> (companies, Company),
///         i32 -> (employments, Employment),
///
///         User.id -> (employments.user_id, Employment),
///         Company.id -> (employments.company_id, Employment),
///
///         Employment.company_id -> (companies.id, Company),
///         Employment.user_id -> (users.id, User),
///     }
/// }
/// ```
///
/// # Syntax
///
/// First you specify your error and connection type with
///
/// ```text
/// (
///     error = diesel::result::Error,
///     connection = PgConnection,
/// ) => {
///     // ...
/// }
/// ```
///
/// Then you define each model type you want to implement [`LoadFrom`] for and which columns and
/// tables to use. There are two possible syntaxes for different purposes.
///
/// ```text
/// i32 -> (users, User),
/// ```
///
/// The first syntax implements `LoadFrom<i32> for User`, meaning from a `Vec<i32>` we can load a
/// `Vec<User>`. It just takes the id type, the table, and the model struct.
///
/// ```text
/// User.id -> (employments.user_id, Employment),
/// ```
///
/// This syntax is required when using [`HasMany`][] and [`HasManyThrough`][]. In this case it
/// implements `LoadFrom<User> for Employment`, meaning from a `Vec<User>` we can get
/// `Vec<Employment>`. It does this by loading the users, mapping the list to the user ids,
/// then finding the employments with those ids.
///
/// [`HasMany`]: trait.HasMany.html
/// [`HasManyThrough`]: trait.HasManyThrough.html
///
/// # What gets generated
///
/// The two syntaxes generates code like this:
///
/// ```
/// # #[macro_use]
/// # extern crate diesel;
/// # use diesel::pg::PgConnection;
/// # use diesel::prelude::*;
/// # use juniper_eager_loading::impl_load_from_for_diesel;
/// # fn main() {}
/// # table! {
/// #     users (id) {
/// #         id -> Integer,
/// #     }
/// # }
/// # table! {
/// #     companies (id) {
/// #         id -> Integer,
/// #     }
/// # }
/// # table! {
/// #     employments (id) {
/// #         id -> Integer,
/// #         user_id -> Integer,
/// #         company_id -> Integer,
/// #     }
/// # }
/// # #[derive(Queryable)]
/// # struct User {
/// #     id: i32,
/// # }
/// # #[derive(Queryable)]
/// # struct Company {
/// #     id: i32,
/// # }
/// # #[derive(Queryable)]
/// # struct Employment {
/// #     id: i32,
/// #     user_id: i32,
/// #     company_id: i32,
/// # }
/// // i32 -> (users, User),
/// impl juniper_eager_loading::LoadFrom<i32> for User {
///     type Error = diesel::result::Error;
///     type Connection = PgConnection;
///
///     fn load(ids: &[i32], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
///         use diesel::pg::expression::dsl::any;
///
///         users::table
///             .filter(users::id.eq(any(ids)))
///             .load::<User>(db)
///             .map_err(From::from)
///     }
/// }
///
/// // User.id -> (employments.user_id, Employment),
/// impl juniper_eager_loading::LoadFrom<User> for Employment {
///     type Error = diesel::result::Error;
///     type Connection = PgConnection;
///
///     fn load(froms: &[User], db: &Self::Connection) -> Result<Vec<Self>, Self::Error> {
///         use diesel::pg::expression::dsl::any;
///
///         let from_ids = froms.iter().map(|other| other.id).collect::<Vec<_>>();
///         employments::table
///             .filter(employments::user_id.eq(any(from_ids)))
///             .load(db)
///             .map_err(From::from)
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_load_from_for_diesel {
    (
        (
            error = $error:path,
            connection = $connection:path,
        ) => {
            $($inner:tt)*
        }
    ) => {
        $crate::__impl_load_from_for_diesel_inner! {
            error = $error,
            connection = $connection,
            $( $inner )*
        }
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __impl_load_from_for_diesel_inner {
    (
        error = $error:path,
        connection = $connection:path,
    ) => {};

    (
        error = $error:path,
        connection = $connection:path,
        $id_ty:ident -> ($table:ident, $ty:ident),
        $( $rest:tt )*
    ) => {
        impl juniper_eager_loading::LoadFrom<$id_ty> for $ty {
            type Error = $error;
            type Connection = $connection;

            fn load(
                ids: &[$id_ty],
                db: &Self::Connection,
            ) -> Result<Vec<Self>, Self::Error> {
                use diesel::pg::expression::dsl::any;

                $table::table
                    .filter($table::id.eq(any(ids)))
                    .load::<$ty>(db)
                    .map_err(From::from)
            }
        }

        $crate::__impl_load_from_for_diesel_inner! {
            error = $error,
            connection = $connection,
            $($rest)*
        }
    };

    (
        error = $error:path,
        connection = $connection:path,
        $join_ty:ident . $join_from:ident -> ($table:ident . $join_to:ident, $ty:ident),
        $( $rest:tt )*
    ) => {
        impl juniper_eager_loading::LoadFrom<$join_ty> for $ty {
            type Error = $error;
            type Connection = $connection;

            fn load(
                froms: &[$join_ty],
                db: &Self::Connection,
            ) -> Result<Vec<Self>, Self::Error> {
                use diesel::pg::expression::dsl::any;

                let from_ids = froms.iter().map(|other| other.$join_from).collect::<Vec<_>>();
                $table::table
                    .filter($table::$join_to.eq(any(from_ids)))
                    .load(db)
                    .map_err(From::from)
            }
        }

        $crate::__impl_load_from_for_diesel_inner! {
            error = $error,
            connection = $connection,
            $($rest)*
        }
    };
}
