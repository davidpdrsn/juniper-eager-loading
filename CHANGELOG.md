# Change Log

All user visible changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/), as described
for Rust libraries in [RFC #1105](https://github.com/rust-lang/rfcs/blob/master/text/1105-api-evolution.md)

## Unreleased

### Added

- Support juniper-from-schema ^0.3.
- Allow specifying foreign key for `has_many_through`.

### Changed

- Renamed `impl_LoadFrom_for_diesel` to `impl_load_from_for_diesel`.

### Removed

N/A

### Fixed

N/A

## [0.1.2] - 2019-06-18

### Fixed

* Fixed spelling mistake in `eager_load_all_children` (from `eager_load_all_chilren`). [#11](https://github.com/davidpdrsn/juniper-eager-loading/pull/11<Paste>)
* Previously, using mixed ID types between parent and child types would not compile. This now actually works. [#10](https://github.com/davidpdrsn/juniper-eager-loading/pull/10)

## [0.1.1]

### Added

* Support for optional foreign keys when using `HasMany` by using the `foreign_key_optional` attribute.

## [0.1.0]

Initial release.

[0.1.2]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.1...0.1.2
[0.1.1]: https://github.com/davidpdrsn/juniper-eager-loading/compare/0.1.0...0.1.1
