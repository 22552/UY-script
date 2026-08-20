# Functions

Functions are reusable procedures (custom blocks) that can return values, including primitives or structs. Functions always run in **Run without screen refresh** mode and **must only be called** from other **Run without screen refresh** procedures or functions to prevent undefined behavior.

Each function must **end with a `return` statement**. Using `stop_this_script` inside a function is undefined behavior.

## Declaring a Function

Use the `func` keyword to define a function.

```goboscript
func my_function(x, y) {
    return $x + $y;
}
```

Struct return types can be specified after the parameter list:

```goboscript
func my_function(x, y) MyStruct {
    return MyStruct { ... };
}
```

## Static Primitive Types (UY-script)

UY-script allows primitive types in parameter and return positions:

```goboscript
func add(Int x, Int y) Int {
    return $x + $y;
}

func label(String value) String {
    return $value;
}
```

Calls and return expressions are checked at compile time. `Int` is accepted when a `Number` is expected.

`String` and struct arguments passed by value are ownership-checked. Returning an owned variable by value also moves it.

## Reference Parameters (UY-script)

Functions can borrow arguments using the same `&T` and `&mut T` syntax as procedures:

```goboscript
func text_length(&String text) Int {
    return length $text;
}
```

Call a reference parameter with a borrow expression:

```goboscript
length = text_length(&message);
```

A mutable parameter requires `&mut value`. Borrow conflicts are checked for the duration of each call.

!!! warning
    UY references are currently compile-time metadata. They are erased before the Scratch backend and do not create runtime pointers or aliases.

See [Types, ownership, and borrowing](types-and-ownership.md).

## Returning Struct Variables

Functions can return struct variables by specifying the struct type as the return type.

```goboscript
struct Vector {
    x,
    y
}

func vec_add(Vector lhs, Vector rhs) Vector {
    return Vector {
        x: $lhs.x + $rhs.x,
        y: $lhs.y + $rhs.y
    };
}
```

Using the returned struct:

```goboscript
Vector vec1 = Vector { x: 10, y: 20 };
Vector vec2 = Vector { x: 5, y: 15 };
Vector result = vec_add(vec1, vec2);

say result.x;
say result.y;
```

!!! note
    When returning struct variables from functions, the return type must be explicitly specified.

## Default Argument Values

Function parameters can have **default values**, allowing callers to omit them:

```goboscript
func greet(name = "world") {
    return "Hello, " & $name & "!";
}
```

- `greet()` returns `"Hello, world!"`
- `greet("aspizu")` returns `"Hello, aspizu!"`

## Calling a Function

Functions are called by name with argument values:

```goboscript
say my_function(1, 2);
```

## Keyword Arguments

Functions can be called using keyword arguments:

```goboscript
greet(name: "aspizu")
```

This is especially useful with defaults or many parameters:

```goboscript
func introduce(name, title = "developer", location = "unknown") {
    return $name & " is a " & $title & " from " & $location;
}
```

```goboscript
introduce(name: "aspizu", location: "India")
```

Keyword arguments can be used in any order as long as required parameters are provided.