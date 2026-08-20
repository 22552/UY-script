# Custom Blocks

Custom blocks, also known as **procedures**, can take input arguments, but unlike functions they do **not return values**.

## Declaring a Custom Block

Use the `proc` keyword to define a custom block. List argument names separated by commas.

```goboscript
proc my_procedure arg1, arg2 {
    say $arg1;
    say $arg2;
}
```

Use the `nowarp` keyword before `proc` to make the custom block *run without screen refresh* **unchecked**.

```goboscript
nowarp proc my_procedure arg1, arg2 {
    say $arg1;
    say $arg2;
}
```

## Typed Arguments

As in goboscript, struct arguments can specify their type before the argument name:

```goboscript
proc process_item Item item_data {
    say $item_data.name;
}
```

UY-script extends the same syntax with primitive types:

```goboscript
proc show_score Int score {
    say $score;
}

proc show_text String text {
    say $text;
}
```

Arguments are checked at compile time. Passing an owned `String` or struct by value moves a simple source variable.

## Reference Arguments (UY-script)

Use `&T` when a procedure should borrow a value instead of taking ownership:

```goboscript
proc inspect &String text {
    say $text;
}

inspect &message;
```

Use `&mut T` for an exclusive borrow:

```goboscript
proc exclusive &mut String text {
    say $text;
}

exclusive &mut message;
```

The current borrow lifetime is the call itself. Multiple shared borrows are allowed; mutable borrows conflict with all other borrows of the same value in that call.

!!! warning
    `&T` and `&mut T` are currently compile-time checking annotations. They are erased before Scratch code generation and do not create runtime pointer/alias semantics.

See [Types, ownership, and borrowing](types-and-ownership.md) for details and limitations.

## Default Argument Values

Procedures support default argument values. This allows a caller to skip certain arguments when calling the block.

```goboscript
proc greet name = "world" {
    say "Hello, " & $name & "!";
}
```

- `greet` says `"Hello, world!"`
- `greet "aspizu"` says `"Hello, aspizu!"`

## Keyword Arguments

Procedures can also be called using keyword arguments, specifying each parameter by name.

```goboscript
proc introduce name, title = "developer", location = "unknown" {
    say $name & " is a " & $title & " from " & $location;
}
```

Call it using keyword arguments:

```goboscript
introduce name: "aspizu", location: "India";
```

Keyword arguments can be used in any order as long as required arguments are provided:

```goboscript
introduce location: "Berlin", name: "Kai";
```

## Calling Custom Blocks

Call a procedure using positional or keyword arguments:

```goboscript
my_procedure "hello", 3;
my_procedure arg2: 3, arg1: "hello";
```

Use `$argname` inside the block to access arguments.