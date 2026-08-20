# Getting Started

UY-script currently keeps the upstream `goboscript` command name for compatibility.
The compiler is a command-line program that can create and build Scratch projects, with
UY's static type, ownership, and borrow checks running before the inherited Scratch
backend.

You can create a new project using the `new` command. Run `goboscript new --help` for
more information.

## Create a new project

Create a new folder, and make sure that your working directory is set to that folder.

```bash
goboscript new
```

This creates a project with a structure similar to:

```
.
├── assets
│   └── blank.svg
├── .git
├── .gitignore
├── goboscript.toml
├── main.gs
├── playground.sb3
└── stage.gs
```

Each `.gs` file holds the code for a sprite; the sprite name is the filename without the
`.gs` extension.

`stage.gs` holds the code for the Stage. Scratch does not allow you to name a sprite
`Stage`, so creating a file named `Stage.gs` is invalid. Because goboscript uses
`stage.gs` for the Stage, you also cannot name a sprite `stage` in lowercase.

`blank.svg` is a blank costume. Both the main sprite and the Stage can contain:

```goboscript
costumes "assets/blank.svg";
```

This adds a costume to a sprite (or the Stage). See
[language/costumes](../language/costumes.md) for more information.

By default, a new git repository is created unless the `-G` option is used.

Use `-m` to create a Makefile.

## Try UY static types

Primitive annotations can be added without changing the Scratch output model:

```goboscript
var Int score = 0;
var String message = "hello";

onflag {
    score += 1;
    say message;
}
```

For ownership and `&T` / `&mut T` reference rules, see
[Types, ownership, and borrowing](../language/types-and-ownership.md).

## Compile the project

To compile the project, run:

```bash
goboscript build
# or
goboscript b
```

This compiles the project into a `.sb3` file in the project directory. The `.sb3` file
has the same name as the project directory.

If compilation fails with errors, the generated `.sb3` file is invalid and should not
be opened in Scratch.

Run `goboscript build --help` for more information.
