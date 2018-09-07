# amethyst-editor-sync

[![Join us on Discord](https://img.shields.io/discord/425678876929163284.svg?logo=discord)](https://discord.gg/GnP5Whs)
[![MIT/Apache](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](COPYING.txt)

A crate that allows an [Amethyst] game to communicate with an editor over [IPC]. This is being
developed in conjunction with [an Electron editor][editor], but is intended to be general enough to
facilitate development of other editor front-ends.

> NOTE: This project is not an official part of the Amethyst project. It is being built in
> coordination with the Amethyst developers, but the Amethyst core team takes no responsibility
> for any nonsense happening here.

## Setup and Usage

Here's an example of how to setup an Amethyst game to communicate with the editor:

```rust
extern crate amethyst_editor_sync;

use amethyst_editor_sync::*;

// Create a root `SyncEditorSystem` to coordinate sending all data to the editor.
let editor_system = SyncEditorSystem::new();

let game_data = GameDataBuilder::default()
    // Register the systems for your game first.

    // Insert a barrier to ensure that the editor syncing runs after all
    // other systems have finished.
    .with_barrier()

    // Register any engine-specific components you want to visualize.
    .with(
        SyncComponentSystem::<Transform>::new("Transform", &editor_system),
        "editor_transform",
        &[],
    )

    // Register any custom components that you use in your game.
    .with(
        SyncComponentSystem::<Foo>::new("Foo", &editor_system),
        "editor_foo",
        &[],
    )

    // Register the `SyncEditorSystem` as thread local to ensure it runs last,
    // after the other systems have had a chance to serialize the state data
    // for all components/resources.
    .with_thread_local(editor_system);

// Make sure you enable serialization for your custom components and resources!
#[derive(Serialize, Deserialize)]
struct Foo {
    bar: usize,
    baz: String,
}
```

Once your game is setup using `amethyst-editor-sync`, it will automatically connect to any running
editor instance on your machine. You can use [the Electron editor][editor] for visualization once
this is setup.

## Motivation and Philosophy

The goal of this project is to provide a functional 80% solution to the problem of sending arbitrary
state data from an Amethyst game to an editor/visualizer tool. It doesn't attempt to perform any
\~\~magic\~\~ in order to detect user-defined components, instead requiring that the developer
explicitly list all components that they want to see in the editor. This introduces fairly heavy
boilerplate when setting up editor support, but means that we have a functional solution that works
*today*. The goal is that this project will act as a placeholder to get us going until we can
put together a less boilerplate-heavy solution for registering user-defined components.

This project is also built around a multi-process architecture, where the game runs independently
of the editor and the two communicate via IPC. This is a deliberate design choice in order to
increase the robustness of the editor: If the game crashes, it cannot crash or corrupt the editor.

## Contributing

You'll need a stable version of [Rust] installed, which can be done via [rustup]. Install the
latest stable toolchain (if you don't already have a stable toolchain installed) and then clone
the repository. You can run the pong example for testing by running `cargo run --examples pong`.
If you need to test functionality against an editor, you can use [the Electron editor][editor].

For any feature requests, please open an issue in the GitHub issue tracker. Pull requests are also
greatly appreciated :heart:

All contributors are expected to follow the [Rust Code of Conduct].

## Status

This project is very early in development and should be treated as expirimental.

Currently it supports communicating with an editor application over UDP. The entire world state
is serialized as a JSON string and broadcast to the editor every frame. It currently supports
sending arbitrary components and resources to an editor, and can do so for any component or
resource that implements [`Serialize`]. Users must manually setup syncing for every component and
resource type that they wish to visualize in the editor, though.

The goal is to support communication over IPC in order to minimize overhead, and to use a binary
protocol to speed up serialization and improve throughput. The current architecture allows for
serialization of each component/resource type to happen in parallel, so we should try to keep that
property going forward.

[Amethyst]: https://www.amethyst.rs/
[IPC]: https://en.wikipedia.org/wiki/Inter-process_communication
[editor]: https://github.com/randomPoison/amethyst-editor
[Rust]: https://www.rust-lang.org/
[rustup]: https://rustup.rs/
[Rust Code of Conduct]: https://www.rust-lang.org/conduct.html
[`Serialize`]: https://docs.rs/serde/*/serde/trait.Serialize.html
