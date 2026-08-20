# UY-script

**UY-script** is an experimental fork of [goboscript](https://github.com/aspizu/goboscript) that keeps the goboscript → Scratch compiler pipeline while adding a stricter compile-time language layer.

The current UY layer adds:

- primitive static types: `Number`, `Int`, `String`, and `Bool`;
- compile-time type checking for variables, lists, procedure/function arguments, and function returns;
- ownership-style move checking for `String` and struct values;
- temporary shared and mutable borrow syntax with `&T` and `&mut T` parameters;
- move/borrow conflict diagnostics before Scratch code generation;
- type/reference metadata erasure so the existing goboscript Scratch backend stays compatible.

> UY-script is experimental. The static analysis is intentionally stricter than normal goboscript, but references are currently a **compile-time checking feature**, not runtime pointers or aliases in Scratch.

## Example

```goboscript
var Int score = 0;
var String message = "hello";

proc show_message &String text {
    say $text;
}

proc consume String text {
    say $text;
}

show_message &message; # shared borrow for this call
consume message;       # moves the String
# say message;         # compile-time error: message was moved
```

Mutable reference parameters use `&mut T` and calls use `&mut value`:

```goboscript
proc exclusive &mut String text {
    say $text;
}

exclusive &mut message;
```

The borrow checker currently treats a borrow as temporary for the duration of the call. Multiple shared borrows are allowed, while a mutable borrow conflicts with any other borrow of the same value in that call.

## Types and ownership

| Type | Notes | Ownership checked? |
| --- | --- | --- |
| `Int` | integer values | No |
| `Number` | numeric values; accepts `Int` | No |
| `Bool` / `Boolean` | boolean values | No |
| `String` | text values | **Yes** |
| structs | user-defined goboscript structs | **Yes** |
| `Any` / `Value` / untyped | dynamic compatibility mode | No |

An owned `String` or struct is moved when it is consumed by another owned destination, passed to an owned parameter, inserted into an owned list, or returned by value. Reassigning the moved variable creates a fresh value and makes it usable again.

See **[Types, ownership, and borrowing](docs/language/types-and-ownership.md)** for the current rules and limitations.

## Installation

UY-script currently keeps the `goboscript` Cargo package/binary name for compatibility with the upstream toolchain.

```bash
git clone https://github.com/22552/UY-script.git
cd UY-script
cargo +nightly install --path .
```

Then use the normal goboscript CLI:

```bash
goboscript --help
```

See [docs/install.md](docs/install.md) for source and Nix installation instructions.

## Documentation

- [Install](docs/install.md)
- [Getting started](docs/getting-started/index.md)
- [Types, ownership, and borrowing](docs/language/types-and-ownership.md)
- [Variables](docs/language/variables.md)
- [Lists](docs/language/lists.md)
- [Custom blocks](docs/language/custom-blocks.md)
- [Functions](docs/language/functions.md)
- [Changelog](CHANGELOG.md)

Most of the language and Scratch backend are inherited from goboscript, so the existing goboscript documentation remains relevant unless a UY-specific page says otherwise.

## Current scope

UY-script is currently focused on compile-time safety while preserving the existing Scratch output model. In particular:

- reference annotations are erased before the Scratch backend runs;
- references cannot currently be stored or returned as first-class values;
- `&mut` expresses exclusive compile-time access but does not create pointer-style mutation semantics in Scratch;
- untyped goboscript remains supported through the dynamic `Value`/`Any` behavior;
- the JavaScript transpilation/backend idea is not implemented yet.

## Upstream and attribution

UY-script is based on [aspizu/goboscript](https://github.com/aspizu/goboscript) and retains its MIT license and existing attribution. This fork adds experimental UY-specific static analysis on top of that codebase.

For upstream goboscript resources, see:

- [goboscript repository](https://github.com/aspizu/goboscript)
- [goboscript documentation](https://aspiz.uk/goboscript/docs/)
- [goboscript standard library](https://github.com/goboscript/std)

## Contributing

Issues and changes specific to this fork should go to the [UY-script repository](https://github.com/22552/UY-script/issues).

When changing UY's type or ownership rules, please update `docs/language/types-and-ownership.md` and add regression tests for both accepted and rejected programs.
