# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

* Panic if resource is missing. ([#14])
* Panic on Linux if no editor is running. ([#15])

[#14]: https://github.com/randomPoison/amethyst-editor-sync/pull/14
[#15]: https://github.com/randomPoison/amethyst-editor-sync/issues/15

## [0.1.0] - 2018-10-04

### Added

* `SyncEditorBundle` with methods `sync_component` and `sync_resource` for setting up editor syncing. ([#8])
* `SerializableEntity` as a temporary solution for allowing components that contain `Entity` values to be serialized.
* Send log output with `EditorLogger`. ([#11])

[#8]: https://github.com/randomPoison/amethyst-editor-sync/pull/8
[#11]: https://github.com/randomPoison/amethyst-editor-sync/pull/11

[Unreleased]: https://github.com/randomPoison/amethyst-editor-sync/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/randomPoison/amethyst-editor-sync/compare/a1a710124bd7d2a132e49433596ee48420729e69...v0.1.0
