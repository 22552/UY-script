# Contributing

UY-script welcomes contributions in the form of Pull Requests. No LLM generated code
will be accepted.

UY-script is written in Rust and is based on goboscript. You'll need to install the
[Rust toolchain](https://www.rust-lang.org/tools/install) for development.

## Setup

Fork your own copy of the UY-script repository and clone it to your local machine.

```bash
git clone https://github.com/$USER/UY-script.git
cd UY-script
```

Install and set the default Rust toolchain to nightly:

```bash
rustup toolchain install nightly
rustup default nightly
```

## Development

Before submitting compiler changes, run:

```bash
cargo check
cargo test
```

UY-specific type, ownership, or borrow changes should include regression tests for both
accepted and rejected behavior, and the corresponding rules should be kept in sync with
[`language/types-and-ownership.md`](language/types-and-ownership.md).

To make development easier, and to validate the generated Scratch project, use the
`tools/run.py` script:

```sh
tools/run.py --validate # or `-v`
```

This assumes that you have set up a testing project at `playground/`. You can create a
testing project by running `goboscript new -G playground`. It will compile the project
and validate it using the schemas from `scratch-parser`. If validation fails, Scratch
will refuse to load the project.

To further debug a project, extract the generated `project.json` from the `.sb3` file in
the `playground/` directory:

```bash
tools/sb3.py playground/playground.sb3
```

If the generated `project.json` is fixed by hand, add it back to the `.sb3` file with:

```bash
# assuming project.json is present at `playground/playground.json`
tools/sb3.py playground/playground.sb3 --patch # or `-p`
```

To validate any `.sb3` file:

```bash
tools/sb3.py path/to/project.sb3 --validate # or `-v`
```

## Conventions

We use conventional commits. See `git log --oneline` for examples.
