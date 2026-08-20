# Types, ownership, and borrowing

UY-script adds an experimental static-analysis layer on top of goboscript. The checks run before the existing Scratch backend, and UY-specific primitive/reference metadata is erased before normal goboscript lowering.

This page describes the rules implemented today. It deliberately does not promise Rust-style runtime references: UY references are currently compile-time annotations and borrow checks.

## Primitive types

UY-script recognizes these built-in type names:

| Type | Meaning |
| --- | --- |
| `Int` | integer number |
| `Number` | any numeric value |
| `String` | text |
| `Bool` / `Boolean` | boolean |
| `Any` / `Value` | dynamic compatibility type |

User-defined struct names continue to work as types.

`Int` is accepted where `Number` is expected. The reverse is not generally accepted because a `Number` may contain a non-integer value.

Untyped goboscript code remains dynamic, so existing code can be migrated gradually.

## Type annotations

Primitive names can be used in the same places where goboscript already accepts struct type names.

```goboscript
var Int score = 0;
var Number speed = 1.5;
var String title = "UY-script";
var Bool enabled = true;

list Int scores = [1, 2, 3];
```

Local variables can also be annotated:

```goboscript
proc example {
    local String message = "hello";
    say message;
}
```

Procedure and function parameters can be typed:

```goboscript
proc set_score Int score {
    say $score;
}

func add(Int a, Int b) Int {
    return $a + $b;
}
```

Function return values are checked against the declared return type.

## What is checked

The current checker verifies:

- variable defaults and assignments;
- list defaults and inserted/replaced values;
- procedure and function arguments;
- function return values;
- move-after-use for owned values;
- shared/mutable borrow conflicts at calls.

Some expressions also produce useful static types. For example, integer-only `+`, `-`, `*`, `%`, and `//` operations stay `Int`; comparisons produce `Bool`; string join produces `String`; and `/` produces `Number`.

Expressions the checker cannot determine precisely fall back to the dynamic type instead of pretending to know more than it does.

## Ownership

UY-script currently treats these types as **owned**:

- `String`;
- user-defined structs.

`Int`, `Number`, and `Bool` are currently copy-like and are not move-checked.

An owned value can be moved when it is consumed by value. Important cases include:

```goboscript
var String first = "hello";
var String second = "";

second = first; # first is moved into second
# say first;    # compile-time error
```

Passing an owned value to a by-value parameter also moves it:

```goboscript
proc consume String value {
    say $value;
}

consume first;
# say first; # compile-time error
```

Owned values can also be moved into typed lists or returned from functions.

Reassigning a moved variable creates a fresh value, so it becomes usable again:

```goboscript
first = "new value";
say first;
```

For branches, UY-script is conservative: if a value can be moved in either branch, it is treated as moved after the branch.

## Shared references: `&T`

A procedure or function parameter can temporarily borrow a value instead of taking ownership:

```goboscript
proc inspect &String value {
    say $value;
}

var String message = "hello";
inspect &message;
say message; # still available
```

The call must use a borrow expression (`&message`) for an `&String` parameter. Passing `message` by value to that parameter is a type error.

Multiple shared borrows of the same value are allowed in one call:

```goboscript
proc compare &String left, &String right {
    say $left = $right;
}

compare &message, &message;
```

## Mutable references: `&mut T`

Use `&mut T` to request exclusive access during a call:

```goboscript
proc exclusive &mut String value {
    say $value;
}

exclusive &mut message;
```

A mutable reference requires an `&mut` argument. A shared `&message` cannot satisfy an `&mut String` parameter.

Within the same call, UY-script rejects:

- two mutable borrows of the same value;
- a mutable borrow plus a shared borrow of the same value;
- a shared borrow while the same value is already mutably borrowed.

A value cannot be moved while it is borrowed, and a moved value cannot be borrowed.

A mutable borrow of a parameter that was itself received through an immutable `&T` parameter is also rejected.

## Borrow lifetime

Borrow scopes are currently **call-scoped**. The checker creates a temporary borrow scope while checking a procedure/function call and releases it after that call.

That means this is valid:

```goboscript
inspect &message;
inspect &message;
```

The borrow from the first call does not remain active during the second call.

## Important limitations

The current reference implementation is intentionally small:

- references can only be written in procedure/function parameter declarations and as borrow expressions for reference arguments;
- references are not first-class values and cannot currently be stored in variables or lists;
- there is no lifetime syntax or lifetime inference beyond the temporary call scope;
- `&mut` is an exclusivity annotation/check, **not a runtime pointer or aliasing mechanism**;
- UY reference metadata is removed before the Scratch backend, so generated Scratch code still uses the normal goboscript value model;
- diagnostics currently reuse the compiler's type-mismatch diagnostic surface, so move/borrow errors may be phrased as type-state mismatches.

## Dynamic compatibility

`Any`, `Value`, and untyped declarations are escape hatches for compatibility with ordinary goboscript. Dynamic values are accepted by the UY checker when a precise static type is unavailable.

This keeps existing goboscript projects buildable while allowing new code to opt into stricter checking incrementally.

## Compilation pipeline

At a high level, UY-script currently performs:

1. normal parsing and goboscript frontend work;
2. UY borrow/move checking;
3. UY primitive type checking;
4. erasure of UY reference and primitive-type metadata where needed;
5. the existing goboscript passes and Scratch code generation.

Because the safety layer is erased before the backend, the generated `.sb3` remains ordinary Scratch-compatible output.