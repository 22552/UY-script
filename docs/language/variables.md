# Variables

## Declaration/Assignment

There are two ways to declare a variable:

### 1. Declare using a top-level `var` statement

```goboscript
var variable_name = 10; # variable is 10 when project first loads
var type_name variable_name; # initialized to struct defaults or zeros
var type_name variable_name = type_name { ... }; # explicit default values
```

UY-script also recognizes primitive type annotations:

```goboscript
var Int score = 0;
var Number speed = 1.5;
var String message = "hello";
var Bool enabled = true;
```

Primitive defaults are checked at compile time. `Int` is accepted where `Number` is expected, while untyped variables remain dynamically typed.

See [Types, ownership, and borrowing](types-and-ownership.md) for the complete UY-specific rules.

### 2. Declare by assigning a value to the variable

The first assignment to a variable is considered its declaration.

```goboscript
variable_name = value;
```

```goboscript
type_name variable_name = value;
```

In UY-script, `type_name` may also be a primitive such as `Int`, `Number`, `String`, or `Bool`.

### Variables for all sprites

If a variable is assigned to in `stage.gs`, it will be declared as **for all sprites**.

### Variables for this sprite only

Variables are by default declared as **for this sprite only**. If you want to declare a variable **for all sprites**, assign to it in `stage.gs`.

## Local Variables

Local variables are accessible only within the procedure they are declared in.

The first assignment with the `local` keyword will declare a local variable; all further uses of the variable refer to the local variable. If a normal variable with the same name exists, it is shadowed.

```goboscript
proc my_procedure {
    local x = 0;
    x = x + 1;
}
```

UY primitive annotations are also valid on local declarations:

```goboscript
proc my_procedure {
    local String message = "hello";
    say message;
}
```

In the compiled Scratch project, a local variable such as `x` is named `my_procedure:x`.

!!! note
    Local variables have undefined behavior if the procedure is recursive, or is NOT a run-without-screen-refresh procedure.

## Ownership in UY-script

`String` and struct values are currently ownership-checked. Assigning one owned variable into a different owned destination moves the source:

```goboscript
var String a = "hello";
var String b = "";

b = a;
# say a; # compile-time error: a was moved
```

Reassigning `a` creates a fresh value and makes it usable again. Numeric and boolean primitives are not move-checked.

Use an `&T` procedure/function parameter when a call should borrow an owned value rather than move it.

## Compound Assignment

| Operator   | Implementation                                       |
|------------|------------------------------------------------------|
| `x++;`     | ![](../assets/increment.png){width="150"}            |
| `x--;`     | ![](../assets/decrement.png){width="150"}            |
| `x += y;`  | ![](../assets/assign_add.png){width="150"}           |
| `x -= y;`  | ![](../assets/assign_subtract.png){width="150"}      |
| `x *= y;`  | ![](../assets/assign_multiply.png){width="150"}      |
| `x /= y;`  | ![](../assets/assign_divide.png){width="150"}        |
| `x //= y;` | ![](../assets/assign_floor_divide.png){width="150"}  |
| `x %= y;`  | ![](../assets/assign_mod.png){width="150"}           |
| `x &= y;`  | ![](../assets/assign_join.png){width="150"}          |

## Show/Hide Variable Monitor

```goboscript
show variable_name;
```

```goboscript
hide variable_name;
```