# Architecture

## Crates list

1. `command-args`: Trait for parsing `&[<&str>]` to proper Rust struct that represent a Redis command.
1. `command-args-derive`: Proc-macro crate to auto generate impl `CommandArgs` trait via `#[derive(CommandArgsBlock)]` attribute, 
supports `#[argtoken("TOKEN")]` helper attribute to let user specify command/block token.
1. `memds`: Binary of this project, a Tokio-based async server.

## Command handler

Trait `CommandHandler` specify how a command being handled and what's its output type is.
New Command added need to be listed in [memds/src/command/mod.rs](/memds/src/command/mod.rs#L74)

## Data structure

Datastructure handling source need to be put in [memds](/memds/src/memds/mod.rs) module. Keeping command handling light.
