# Macros

UY-script inherits goboscript's C-like preprocessor and adds compile-time Lua procedural macros.
This allows you to define token macros, include files, and generate UY-script source before it is
parsed and type checked.

!!! note
    The preprocessor directives start with a `%` character. The `%` character must
    always appear at the start of a line. There cannot be any indentation before the
    `%` character.

## Include

Include the contents of a file.

```goboscript
%include path/to/file.gs
```

The `.gs` extension is optional. If not specified (recommended), the file extension will
be added automatically.

If the include path is a directory, the file inside the directory with the same name as
the directory will be included.

By default, the include path is relative to the project root directory. To include a
file relative to the current file, use `./` or `../`

!!! tip
    [`bkpk.py`](https://gist.github.com/aspizu/c81452bfb7a333d0819f0279e51e078a) is a small
    Python script that lets you include files from the internet using `%include` directives.

    ```goboscript
    # run `./bkpk.py` to compile your project, instead of `goboscript build`
    %include https://github.com/username/repo/branchname/filename.gs
    ```

## Define

Define a macro. That identifier will be substituted with the subsequent text.

```goboscript
%define macro_name replacement text
```

## Define with arguments

Define a macro with arguments. The arguments will be substituted with the tokens from
the callsite.

```goboscript
%define macro_name(arg1, arg2) replacement text
```

Since `()` are interpreted as function parameter brackets, use double parentheses to include them in the expansion:

```goboscript
%define foo ((1 + 2))
```

This expands to `((1 + 2))`, allowing you to control operator precedence in macro substitutions.

Use `\` at the end of a line to continue the replacement text across multiple lines:

```goboscript
%define long_macro this is a very long \
                   replacement text that spans \
                   multiple lines
```

## Define with overloaded arguments

Macros with arguments can be overloaded by defining multiple versions with different
numbers of arguments. The correct version will be selected based on the number of
arguments passed at the callsite.
```goboscript
%define MACRO(A) "MACRO(A)"
%define MACRO(A, B) "MACRO(A, B)"

onflag {
    say MACRO(1);      # expands to "MACRO(A)"
    say MACRO(1, 1);   # expands to "MACRO(A, B)"
}
```

Each overload is stored independently, so defining `MACRO` with one argument does not
affect the definition of `MACRO` with two arguments. Using `%undef macro_name` removes
all overloads for that name at once.

## Remove a macro definition

```goboscript
%undef macro_name
```

## Conditional compilation

```goboscript
%if macro_name
    code
%endif
```

```goboscript
%if not macro_name
    code
%endif
```

## Compile-time Lua

UY-script can execute Lua 5.4 while compiling a project. A Lua block starts with `%lua`
at the beginning of a line and uses braces around the Lua body:

```goboscript
%lua {
for i = 1, 3 do
    uy.emit("var Int generated_" .. i .. " = " .. i .. ";")
end
}
```

`uy.emit(source)` appends generated UY-script source at the position of the macro. The
example above behaves as if the source contained:

```goboscript
var Int generated_1 = 1;
var Int generated_2 = 2;
var Int generated_3 = 3;
```

Generated source is lexed and parsed normally, so it also passes UY-script's static type,
ownership, and borrow checks.

Lua long strings are useful for generating larger blocks of code:

```goboscript
%lua {
for i = 1, 4 do
    uy.emit(string.format([[
proc generated_%d Int x {
    say x + %d;
}
]], i, i))
end
}
```

This also works well for lookup tables that are expensive or awkward to construct in
Scratch itself. For example, a small Unicode table can be generated at compile time:

```goboscript
%lua {
local values = {}
for cp = 0x20, 0x7e do
    values[#values + 1] = utf8.char(cp)
end

uy.emit("list String ascii;")
uy.emit("onflag {")
for _, value in ipairs(values) do
    -- %q creates a quoted Lua string; for complex escaping prefer a helper generator.
    uy.emit(string.format("    add %q to ascii;", value))
end
uy.emit("}")
}
```

### Lua sandbox

Compile-time Lua is intentionally restricted. `os`, `io`, `package`, `debug`, `require`,
`dofile`, and `loadfile` are unavailable, so a project cannot use a Lua macro to directly
run host commands or read arbitrary files. Each block is also limited to roughly
10,000,000 Lua VM instructions and 8 MiB of emitted source.

!!! note
    Lua macros currently run only in the native UY-script CLI. The current
    `wasm32-unknown-unknown` compiler build reports an error when `%lua` is used.

!!! note
    `%include` and `%if` are translation-unit directives and are resolved before Lua
    code generation. Generate ordinary UY-script code (and token-level macros such as
    `%define`) with `uy.emit`; do not generate `%include` from Lua.

## Concatenate Tokens

```goboscript
CONCAT(prefix, suffix) # becomes prefixsuffix
```

## Stringify Tokens

`STRINGIFY` is a built-in macro that converts its argument tokens into a string literal.
All tokens inside the parentheses are joined with spaces and produced as a single string value.

```goboscript
STRINGIFY(hello world) # becomes "hello world"
```

This is useful when you need to turn a macro expansion or a sequence of tokens into a
string at compile time:

```goboscript
%define VERSION 1 2 3

onflag {
    say STRINGIFY(VERSION); # says "1 2 3"
}
```

Nested parentheses are supported and are included verbatim in the resulting string:

```goboscript
STRINGIFY(foo(bar, baz)) # becomes "foo ( bar , baz )"
```
