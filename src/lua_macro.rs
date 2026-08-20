use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};

use crate::diagnostic::{
    Diagnostic,
    DiagnosticKind,
};

const DIRECTIVE: &[u8] = b"%lua";
const MAX_EMITTED_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LuaExpansion {
    pub span: Range<usize>,
    pub source: String,
}

#[derive(Debug)]
pub struct PreparedLuaMacros {
    pub masked_source: String,
    pub expansions: Vec<LuaExpansion>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
struct LuaBlock {
    full_span: Range<usize>,
    body_span: Range<usize>,
}

/// Find and execute UY-script `%lua { ... }` procedural macros.
///
/// Macro bodies are replaced with whitespace of the same byte length so ordinary
/// source positions remain valid. Generated source is tokenized separately by the
/// parser and all generated tokens are mapped back to the macro callsite span.
pub fn prepare(source: &str) -> PreparedLuaMacros {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut expansions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut cursor = 0;

    while let Some(start) = find_next_lua_directive(bytes, cursor) {
        match parse_block(bytes, start) {
            Ok(block) => {
                for byte in &mut masked[block.full_span.clone()] {
                    if *byte != b'\n' {
                        *byte = b' ';
                    }
                }

                let body = &source[block.body_span.clone()];

                #[cfg(not(target_arch = "wasm32"))]
                match execute_lua(body) {
                    Ok(generated) => {
                        if !generated.is_empty() {
                            expansions.push(LuaExpansion {
                                span: block.full_span.clone(),
                                source: generated,
                            });
                        }
                    }
                    Err(error) => diagnostics.push(lua_error(
                        format!("Lua macro failed: {error}"),
                        block.full_span.clone(),
                        Some("the error came from compile-time Lua inside `%lua { ... }`"),
                    )),
                }

                #[cfg(target_arch = "wasm32")]
                diagnostics.push(lua_error(
                    "Lua macros are not available in the wasm32 build",
                    block.full_span.clone(),
                    Some("build this project with the native UY-script CLI"),
                ));

                cursor = block.full_span.end;
            }
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                break;
            }
        }
    }

    PreparedLuaMacros {
        masked_source: String::from_utf8(masked).expect("source was valid UTF-8"),
        expansions,
        diagnostics,
    }
}

fn lua_error(
    error: impl ToString,
    span: Range<usize>,
    help: Option<&str>,
) -> Diagnostic {
    Diagnostic {
        kind: DiagnosticKind::io_error(error, help),
        span,
    }
}

fn find_next_lua_directive(bytes: &[u8], mut i: usize) -> Option<usize> {
    while i + DIRECTIVE.len() <= bytes.len() {
        if bytes[i] == b'%'
            && (i == 0 || bytes[i - 1] == b'\n')
            && bytes[i..].starts_with(DIRECTIVE)
            && bytes
                .get(i + DIRECTIVE.len())
                .is_some_and(|byte| (*byte).is_ascii_whitespace() || *byte == b'{')
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_block(bytes: &[u8], start: usize) -> Result<LuaBlock, Diagnostic> {
    let mut i = start + DIRECTIVE.len();

    while bytes
        .get(i)
        .is_some_and(|byte| matches!(*byte, b' ' | b'\t' | b'\r'))
    {
        i += 1;
    }

    if bytes.get(i) != Some(&b'{') {
        let end = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset)
            .unwrap_or(bytes.len());
        return Err(lua_error(
            "expected `{` after `%lua`",
            start..end.max(start + DIRECTIVE.len()),
            Some("use `%lua { ... }` with the directive at the start of a line"),
        ));
    }

    let open = i;
    let close = find_matching_brace(bytes, open).ok_or_else(|| {
        lua_error(
            "unterminated `%lua` block",
            start..bytes.len(),
            Some("add the closing `}` for the compile-time Lua block"),
        )
    })?;

    Ok(LuaBlock {
        full_span: start..close + 1,
        body_span: open + 1..close,
    })
}

fn find_matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let mut i = open + 1;
    let mut depth = 1usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                i = skip_quoted(bytes, i)?;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                if let Some(end) = skip_long_bracket(bytes, i + 2) {
                    i = end;
                } else {
                    i += 2;
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
            }
            b'[' => {
                if let Some(end) = skip_long_bracket(bytes, i) {
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }

    None
}

fn skip_quoted(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = bytes[start];
    let mut i = start + 1;

    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i = (i + 2).min(bytes.len()),
            byte if byte == quote => return Some(i + 1),
            _ => i += 1,
        }
    }

    None
}

/// If `start` points at a Lua long-bracket string/comment opener (`[[` or
/// `[=[` etc.), return the byte immediately after its matching closer.
fn skip_long_bracket(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }

    let mut i = start + 1;
    let mut equals = 0usize;
    while bytes.get(i) == Some(&b'=') {
        equals += 1;
        i += 1;
    }
    if bytes.get(i) != Some(&b'[') {
        return None;
    }

    i += 1;
    while i < bytes.len() {
        if bytes[i] == b']' {
            let mut j = i + 1;
            let mut seen = 0usize;
            while seen < equals && bytes.get(j) == Some(&b'=') {
                seen += 1;
                j += 1;
            }
            if seen == equals && bytes.get(j) == Some(&b']') {
                return Some(j + 1);
            }
        }
        i += 1;
    }

    None
}

#[cfg(not(target_arch = "wasm32"))]
fn execute_lua(code: &str) -> Result<String, mlua::Error> {
    use mlua::{
        HookTriggers,
        Lua,
        Value,
        VmState,
    };

    let lua = Lua::new();
    let globals = lua.globals();

    // Compile-time macros are intentionally sandboxed. They can calculate and
    // generate UY source, but cannot directly access the host filesystem/process.
    for name in [
        "io",
        "os",
        "package",
        "debug",
        "require",
        "dofile",
        "loadfile",
    ] {
        globals.set(name, Value::Nil)?;
    }

    // Stop accidental infinite compile-time programs. The hook runs every 10k VM
    // instructions and rejects a macro after roughly ten million instructions.
    let instruction_count = Rc::new(Cell::new(0u64));
    let count = instruction_count.clone();
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(10_000),
        move |_, _| {
            let next = count.get() + 10_000;
            count.set(next);
            if next > 10_000_000 {
                return Err(mlua::Error::RuntimeError(
                    "Lua macro exceeded the 10,000,000 instruction limit".into(),
                ));
            }
            Ok(VmState::Continue)
        },
    )?;

    let output = Rc::new(RefCell::new(String::new()));
    let uy = lua.create_table()?;
    let emitted = output.clone();
    let emit = lua.create_function(move |_, text: String| {
        let mut output = emitted.borrow_mut();
        let extra_newline = usize::from(!text.ends_with('\n'));
        if output.len() + text.len() + extra_newline > MAX_EMITTED_BYTES {
            return Err(mlua::Error::RuntimeError(format!(
                "Lua macro output exceeded {} MiB",
                MAX_EMITTED_BYTES / (1024 * 1024)
            )));
        }
        output.push_str(&text);
        if !text.ends_with('\n') {
            output.push('\n');
        }
        Ok(())
    })?;
    uy.set("emit", emit)?;
    globals.set("uy", uy)?;

    lua.load(code).set_name("UY-script %lua macro").exec()?;
    let generated = output.borrow().clone();
    Ok(generated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_braces_and_long_strings() {
        let source = r#"%lua {
local t = { one = 1, two = { 2, 3 } }
uy.emit([[
onflag {
    say "hello";
}
]])
}
onflag { say 2; }
"#;
        let prepared = prepare(source);
        assert!(prepared.diagnostics.is_empty());
        assert_eq!(prepared.expansions.len(), 1);
        assert!(prepared.masked_source.contains("onflag { say 2; }"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn lua_can_generate_uy_source() {
        let source = r#"%lua {
for i = 1, 3 do
    uy.emit("var Int generated_" .. i .. " = " .. i .. ";")
end
}
"#;
        let prepared = prepare(source);
        assert!(prepared.diagnostics.is_empty());
        let generated = &prepared.expansions[0].source;
        assert!(generated.contains("var Int generated_1 = 1;"));
        assert!(generated.contains("var Int generated_3 = 3;"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn dangerous_host_libraries_are_hidden() {
        let source = "%lua { assert(os == nil and io == nil and package == nil) }\n";
        let prepared = prepare(source);
        assert!(prepared.diagnostics.is_empty());
    }
}
