# Change Log

All user visible changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/), as described
for Rust libraries in [RFC #1105](https://github.com/rust-lang/rfcs/blob/master/text/1105-api-evolution.md)

## Unreleased

- Support generating code for fields that take arguments with
  - `#[has_one(field_arguments = YourArgType)]`
  - `#[option_has_one(field_arguments = YourArgType)]`
  - `#[has_many(field_arguments = YourArgType)]`
  - `#[has_many_through(field_arguments = YourArgType)]`

### Breaking changes

None.

## [0.5.0] - 2019-11-27

### Breaking changes

**Rename `GraphqlNodeForModel::Connection` to `Context`**

You might need more than just a database connection to eager load data, for example the currently logged in user or other data about the HTTP request. Since `Connection` was previously generic it was technically possible but it was awkward in practice. `Connection` is now renamed to `Context` and is supposed to be your Juniper context which can contain whatever data you need.

**`impl_load_from_for_diesel_(pg|mysql|sqlite)` syntax changed**

The `impl_load_from_for_diesel_(pg|mysql|sqlite)` macro now requires `context = YourContextType` rather than `connection = YourConnectionType`.

**Require `Context` to have `db` method for Diesel macro**

`impl_load_from_for_diesel_(pg|mysql|sqlite)` now requires that your context type has a method called `db` that returns a reference to a Diesel connection.

For example:

```rust
struct Context {
    db: PgConnection,
}

impl Context {
    fn db(&self) -> &PgConnection {
        &self.db
    }
}

// Whatever the method returns has to work with Diesel's `load` method
users::table
    .filter(users::id.eq(any(user_ids)))
    .load::<User>(ctx.db())
```

This is _only_ necessary if you're using the `impl_load_from_for_diesel_(pg|mysql|sqlite)`.

**Attribute values should no longer be surrounded by quotes**

Before:

```rust
#[derive(Clone, EagerLoading)]
#[eager_loading(
    context = "Context",
    error = "Box<dyn Error>",
    model = "models::User",
    id = "i32",
    root_model_field = "user,"
)]
pub struct User {
    user: models::User,
    #[has_one(
        foreign_key_field = "country_id,"
        root_model_field = "country,"
        graphql_field = "country,"
    )]
    country: HasOne<Country>,
}
```

After:

```rust
#[derive(Clone, EagerLoading)]
#[eager_loading(
    context = Context,
    error = Box<dyn Error>,
    model = models::User,
    id = i32,
    root_model_field = user,
)]
pub struct User {
    user: models::User,
    #[has_one(
        foreign_key_field = country_id,
        root_model_field = country,
        graphql_field = country,
    )]
    country: HasOne<Country>,
}
```

This change is made for all attributes:
- `#[has_one]`
- `#[option_has_one]`
- `#[has_many]`
- `#[has_many_through]`

**Remove `join_model_field` option from `HasManyThrough`**

Turns out it wasn't being used and therefore didn't do anything.

## [0.4.2] - 2019-11-14

- Support recursive types for `HasOne` and `OptionHasOne` associations. You can now use `HasOne<Box<T>>` or `OptionHasOne<Box<T>>` in your GraphQL types. `HasMany` and `HasManyThrough` already support recursive types because they're backed by `Vec`s.

## [0.4.1] - 2019-10-29

- The [examples](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples) has been much improved.
- Remove warning about this library being experimental. It is safe to use in production (:

## [0.4.0] - 2019-10-23

- Move `impl_load_from_for_diesel_{pg|mysql|sqlite}!` to proc-macros. Are fully backwards compatible but will give better errors.
- Tweak docs for `impl_load_from_for_diesel_{pg|mysql|sqlite}!`.
- `Association` trait has been added to abstraction over `HasOne`, `OptionHasOne`, `HasMany`, and `HasManyThrough` associations.

### Breaking changes

- `EagerLoadChildrenOfType::child_ids` has been removed. Use `EagerLoadChildrenOfType::load_children` instead. See [#27](https://github.com/davidpdrsn/juniper-eager-loading/issues/27) for more context.
- `EagerLoadChildrenOfType::ChildId` has been removed. It was only used by `child_ids` and was therefore no longer necessary.
- `LoadResult` has been renamed to `LoadChildrenOutput`. Including `Result` in the name made it seem like it might related to errors, which it wasn't.
    - `LoadResult::Ids` has been renamed to `LoadChildrenOutput::ChildModel` to match the changes to `EagerLoadChildrenOfType::load_children`.
    - `LoadResult::Models` has been renamed to `LoadChildrenOutput::ChildAndJoinModels` for the same reason.
    - The second type parameter (used for the join model) now defaults to `()`.
- The signature of `EagerLoadChildrenOfType::is_child_of` has been changed to `parent: &Self, child: &Child, join_model: &JoinModel`. Manually pulling things out of the tuple was tedious.
- `EagerLoadChildrenOfType::association` has been added. This methods allows for some boilerplate to be removed from `EagerLoadChildrenOfType`.
- `EagerLoadChildrenOfType::loaded_child` and `EagerLoadChildrenOfType::assert_loaded_otherwise_failed` has been removed and implemented generically using the new `Association` trait.
- The deprecated macro `impl_load_from_for_diesel` has been removed completely. Use `impl_load_from_for_diesel_{pg|mysql|sqlite}` instead.
- Support eager loading GraphQL fields that take arguments. See the docs for more information and examples.
  - Add `EagerLoadChildrenOfType::FieldArguments`
  - The following methods take the arguments:
    - `EagerLoadChildrenOfType::load_children`
    - `EagerLoadChildrenOfType::is_child_of`
    - `EagerLoadChildrenOfType::eager_load_children`
  - Add second generic argument to `LoadFrom` which will be the arguments and accept argument of that type in `LoadFrom::load`.

If you're using the derive macros for everything in your app you shouldn't have to care about any of these changes. The generated code will automatically handle them.

## [0.3.1] - 2019-10-09

### Added

- Add specific versions of `impl_load_from_for_diesel_*` for each backend supported by Diesel:
    - `impl_load_from_for_diesel_pg` (formerly `impl_load_from_for_diesel`)
    - `impl_load_from_for_diesel_sqlite`
    - `impl_load_from_for_diesel_mysql`

### Changed

- Deprecate `impl_load_from_for_diesel`. `impl_load_from_for_diesel_pg` should be used instead. `impl_load_from_for_diesel` will be removed in 0.4.0.

## [0.3.0] - 2019-10-05

### Added

- Documentation section about eager loading interface or union types. [#19](https://github.com/davidpdrsn/juniper-eager-loading/pull/19)

### Removed

- `GenericQueryTrail` has been removed since it is no longer necessary thanks to <https://github.com/davidpdrsn/juniper-from-schema/pull/82>. This also lead to the removal of the `QueryTrail` type parameter on `EagerLoadChildrenOfType` and `EagerLoadAllChildren`. [#20](https://github.com/davidpdrsn/juniper-eager-loading/pull/20)

### Fixed

- Fixed "mutable_borrow_reservation_conflict" warnings.

## [0.2.0] - 2019-06-30

### Added

- Support juniper-from-schema ^0.3.
- Allow specifying foreign key for `has_many_through`.

### Changed

- Renamed `impl_LoadFrom_for_diesel` to `impl_load_from_for_diesel`.

### Removed

- The associated type `ChildModel` on `EagerLoadChildrenOfType` has been removed because it wasn't necessary.

## [0.1.2] - 2019-06-18

### Fixed

* Fixed spelling mistake in `eager_load_all_children` (from `eager_load_all_chilren`). [#11](https://github.com/davidpdrsn/juniper-eager-loading/pull/11<Paste>)
* Previously, using mixed ID types between parent and child types would not compile. This now actually works. [#10](https://github.com/davidpdrsn/juniper-eager-loading/pull/10)

## [0.1.1]

### Added

* Support for optional foreign keys when using `HasMany` by using the `foreign_key_optional` attribute.

## [0.1.0]

Initial release.

[0.5.0]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.4.2...0.5.0
[0.4.2]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.2...0.2.0
[0.1.2]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.1...0.1.2
[0.1.1]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.0...0.1.1
