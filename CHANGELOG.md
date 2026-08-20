# Changelog

## UY-script

### 20th August 2026: compile-time Lua procedural macros

- Added `%lua { ... }` blocks executed at compile time by the native CLI.
- Added `uy.emit(source)` for generating UY-script code from Lua.
- Generated source re-enters the normal lexer, token macro, parser, static type, ownership, and borrow-checking pipeline.
- Added a restricted Lua environment with host filesystem/process libraries removed.
- Added instruction and generated-source size limits for compile-time macros.
- Kept the `wasm32-unknown-unknown` build working by reporting Lua macros as unavailable there.

### 20th August 2026: borrow checking and reference syntax

- Added temporary `&T` and `&mut T` procedure/function parameters.
- Added `&value` and `&mut value` borrow expressions for reference arguments.
- Added compile-time shared/exclusive borrow conflict checking.
- Added move/borrow conflict checks and rejection of borrowing moved values.
- UY reference metadata is erased before the existing Scratch backend.

Commit: [`ba3c440`](https://github.com/22552/UY-script/commit/ba3c440d3ac225dc5e89815ddb7788dd803d4f55)

### 20th August 2026: static primitive types and ownership checking

- Added `Number`, `Int`, `String`, and `Bool`/`Boolean` annotations.
- Added compile-time checking for variables, lists, arguments, and function returns.
- Added ownership-style move-after-use checking for `String` and struct values.
- Kept untyped/`Any`/`Value` code as dynamic compatibility mode.
- Primitive UY annotations are erased before ordinary goboscript → Scratch lowering.

Commit: [`ac7472a`](https://github.com/22552/UY-script/commit/ac7472a7faef28b409e36eee2cbb35598d4896d0)

## Inherited goboscript history

### 26th May 2026: `STRINGIFY` built-in macro [(#290)](https://github.com/aspizu/goboscript/pull/290)

```goboscript
STRINGIFY(hello world) # becomes "hello world"
```

### 9th May 2026: `show` and `hide` statements work with struct-typed lists and variables [(#284)](https://github.com/aspizu/goboscript/pull/284)

```goboscript
struct Point {x,y,z}
var Point p;
show p;
hide p;
```
