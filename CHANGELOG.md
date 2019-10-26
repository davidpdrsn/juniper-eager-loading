# Change Log

All user visible changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/), as described
for Rust libraries in [RFC #1105](https://github.com/rust-lang/rfcs/blob/master/text/1105-api-evolution.md)

## Unreleased

- The [examples](https://github.com/davidpdrsn/juniper-eager-loading/tree/master/examples) has been much improved.

### Breaking changes

None.

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

[0.4.0]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.2...0.2.0
[0.1.2]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.1...0.1.2
[0.1.1]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.0...0.1.1
