# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

* Create and destroy entities at runtime. ([#40])
* `sync_components`, `read_components`, `sync_resources`, and `read_resources`
  macros have been added for registering many components/resources at once. ([#43])

### Removed

* The `type_set` macro and the related `SyncEditorBundle` methods have been
  removed. ([#43])
* `EditorSyncBundle::get_connection` has been made private. ([#43])

### Fixed

* Read-only components no longer require `Deserialize`. ([#38])

### Breaking Changes

### Upgraded to Amethyst 0.10 ([#46])

Updated to depend on version 0.10 of amethyst. If your project uses a previous
version of Amethyst, you'll need to upgrade to amethyst 0.10 in order to use
the latest version of amethyst-editor-sync.

### Changed API for Registering Components/Resources ([#43])

The `type_set` macro has been removed, `SyncEditorBundle` no longer directly
exposes a builder pattern. `sync_component` and the other registration
methods take the same parameters, but they no longer return `&mut Self` and
there are no variants that take a type set as a paramenter. Instead, we now
provide some helper macros for easily registering many types at once. We
recommend combining these with the [tap] crate to get a builder-like way of
chaining method calls to create the bundle.

For example, this setup logic:

```rust
let components = type_set![Ball, Paddle];
let resources = type_set![ScoreBoard];
let editor_sync_bundle = SyncEditorBundle::new()
    .sync_default_types()
    .sync_components(&components)
    .sync_resources(&resources);
```

Would now be written like this:

```rust
let editor_sync_bundle = SyncEditorBundle::new()
    .tap(SyncEditorBundle::sync_default_types)
    .tap(|bundle| sync_components!(bundle, Ball, Paddle))
    .tap(|bundle| sync_resources!(bundle, ScoreBoard));
```

If you prefer to not use [tap] (or method chaining in general), you may also
make your `SyncEditorBundle` mutable and modify it directly:

```rust
let mut bundle = SyncEditorBundle::new();
bundle.sync_default_types();
sync_components!(bundle, Ball, Paddle);
sync_resources!(bundle, ScoreBoard);
```

### Setting Up Log Output ([#43])

`EditorSyncBundle::get_connection` has been made private. Instead of calling
`get_connection` and passing the output to `EditorLogger::new`, you can pass
a reference to the bundle directly:

```rust
EditorLogger::new(&editor_sync_bundle).start();
```

[#38]: https://github.com/randomPoison/amethyst-editor-sync/issues/38
[#40]: https://github.com/randomPoison/amethyst-editor-sync/pull/40
[#43]: https://github.com/randomPoison/amethyst-editor-sync/pull/43
[#46]: https://github.com/randomPoison/amethyst-editor-sync/pull/46
[tap]: https://crates.io/crates/tap

## [0.3.0] - 2018-10-26

### Added

* `sync_components` and `sync_resources` methods in `SyncEditorBundle` to synchronize all types
  in a `TypeSet`. `TypeSets` can be created through the `type_set!` macro to reduce the verbosity
  of synchronizing many types. ([#19])
* `sync_default_types` method in `SyncEditorBundle` to easily synchronize some commonly used
  engine types. ([#20])
* :tada: Support for editing `Resource` values! :tada: ([#25])
* `read_resource` and `read_resources` methods in `SyncEditorBundle` to register resources that
  don't implement `DeserializeOwned`. ([#33])
* `read_component` and `read_components` methods in `SyncEditorBundle` to register components that
  don't implement `DeserializeOwned` ([#37])

### Breaking Changes

* `SyncEditorBundle` type format. If the type is explicitly given, it will need to be updated. ([#21])
* Resources registered via `SyncEditorBundle::sync_resource` must now be `DeserializeOwned` (as
  well as `Serialize`). This enables support for applying changes made in the editor. If you have
  a `Resource` that only implements `Serialize`, register it with `SyncEditorBundle::read_resource`
  instead. ([#25])
* Components registered via `SyncEditorBundle::sync_component` must now be `DeserializeOwned` (as
  well as `Serialize`). ([#37])
* `SyncResourceSystem` has been removed. If your code was directly registering the sync systems
  with your dispatcher, please update to using `SyncEditorBundle` instead. ([#25])

[#19]: https://github.com/randomPoison/amethyst-editor-sync/issues/19
[#20]: https://github.com/randomPoison/amethyst-editor-sync/issues/20
[#21]: https://github.com/randomPoison/amethyst-editor-sync/pull/21
[#25]: https://github.com/randomPoison/amethyst-editor-sync/pull/25
[#33]: https://github.com/randomPoison/amethyst-editor-sync/pull/33
[#37]: https://github.com/randomPoison/amethyst-editor-sync/pull/37

## [0.2.0] - 2018-10-14

### Fixed

* Panic if resource is missing. ([#14])
* Panic on Linux if no editor is running. ([#15])
* Panic if sync messages are too large. ([#17])

### Added

* `SyncEditorBundle::with_interval` to configure how frequently the full state is sent. ([#23])

### Breaking Changes

The state messages sent may now omit some or all of the data fields. Editors should be updated to
handle this case by not attempting to update their corresponding local data.

[#14]: https://github.com/randomPoison/amethyst-editor-sync/pull/14
[#15]: https://github.com/randomPoison/amethyst-editor-sync/issues/15
[#17]: https://github.com/randomPoison/amethyst-editor-sync/pull/17
[#23]: https://github.com/randomPoison/amethyst-editor-sync/pull/23

## [0.1.0] - 2018-10-04

### Added

* `SyncEditorBundle` with methods `sync_component` and `sync_resource` for setting up editor syncing. ([#8])
* `SerializableEntity` as a temporary solution for allowing components that contain `Entity` values to be serialized.
* Send log output with `EditorLogger`. ([#11])

[#8]: https://github.com/randomPoison/amethyst-editor-sync/pull/8
[#11]: https://github.com/randomPoison/amethyst-editor-sync/pull/11

[Unreleased]: https://github.com/randomPoison/amethyst-editor-sync/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/randomPoison/amethyst-editor-sync/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/randomPoison/amethyst-editor-sync/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/randomPoison/amethyst-editor-sync/compare/a1a710124bd7d2a132e49433596ee48420729e69...v0.1.0
