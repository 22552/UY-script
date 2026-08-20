use std::{
    cell::RefCell,
    collections::HashMap,
    io::{self, Cursor},
    path::{Path, PathBuf},
    rc::Rc,
};

use libgoboscript::{
    lua_macro::prepare,
    parser,
    translation_unit::TranslationUnit,
    vfs::VFS,
};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn lua_macro_generates_source() {
    let prepared = prepare(
        r#"%lua {
for i = 1, 3 do
    uy.emit("var Int generated_" .. i .. " = " .. i .. ";")
end
}
"#,
    );

    assert!(prepared.diagnostics.is_empty(), "{:#?}", prepared.diagnostics);
    assert_eq!(prepared.expansions.len(), 1);
    assert_eq!(
        prepared.expansions[0].source,
        "var Int generated_1 = 1;\nvar Int generated_2 = 2;\nvar Int generated_3 = 3;\n"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn multiple_lua_macros_expand_in_source_order() {
    let source = r#"%lua { uy.emit("var Int first = 1;") }
var Int middle = 2;
%lua { uy.emit("var Int last = 3;") }
"#;
    let prepared = prepare(source);

    assert!(prepared.diagnostics.is_empty(), "{:#?}", prepared.diagnostics);
    assert_eq!(prepared.expansions.len(), 2);
    assert!(prepared.expansions[0].span.start < prepared.expansions[1].span.start);
    assert_eq!(prepared.expansions[0].source, "var Int first = 1;\n");
    assert_eq!(prepared.expansions[1].source, "var Int last = 3;\n");
    assert!(prepared.masked_source.contains("var Int middle = 2;"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn braces_inside_lua_strings_comments_and_long_strings_do_not_end_block() {
    let source = r#"%lua {
local quoted = "}" -- this brace is data
-- { and } in a comment are also data
local generated = [=[
onflag {
    say "}";
}
]=]
uy.emit(generated)
}
var Int after = 1;
"#;
    let prepared = prepare(source);

    assert!(prepared.diagnostics.is_empty(), "{:#?}", prepared.diagnostics);
    assert_eq!(prepared.expansions.len(), 1);
    assert!(prepared.expansions[0].source.contains("onflag {"));
    assert!(prepared.expansions[0].source.contains("say \"}\";"));
    assert!(prepared.masked_source.contains("var Int after = 1;"));
}

#[test]
fn lua_directive_must_start_a_line() {
    let source = "var x = 1; %lua { this is not a macro }\n";
    let prepared = prepare(source);

    assert!(prepared.diagnostics.is_empty());
    assert!(prepared.expansions.is_empty());
    assert_eq!(prepared.masked_source, source);
}

#[test]
fn missing_opening_brace_is_reported() {
    let prepared = prepare("%lua nope\n");

    assert_eq!(prepared.diagnostics.len(), 1);
    assert!(prepared.expansions.is_empty());
}

#[test]
fn unterminated_lua_block_is_reported() {
    let prepared = prepare("%lua {\nuy.emit(\"var x = 1;\")\n");

    assert_eq!(prepared.diagnostics.len(), 1);
    assert!(prepared.expansions.is_empty());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn lua_runtime_errors_become_compiler_diagnostics() {
    let prepared = prepare("%lua { error(\"boom\") }\n");

    assert_eq!(prepared.diagnostics.len(), 1);
    assert!(prepared.expansions.is_empty());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn lua_host_access_is_sandboxed() {
    let prepared = prepare(
        "%lua { assert(os == nil and io == nil and package == nil and debug == nil and require == nil and dofile == nil and loadfile == nil) }\n",
    );

    assert!(prepared.diagnostics.is_empty(), "{:#?}", prepared.diagnostics);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn uy_emit_adds_one_trailing_newline() {
    let prepared = prepare(
        r#"%lua {
uy.emit("var Int a = 1;")
uy.emit("var Int b = 2;\n")
}
"#,
    );

    assert!(prepared.diagnostics.is_empty(), "{:#?}", prepared.diagnostics);
    assert_eq!(
        prepared.expansions[0].source,
        "var Int a = 1;\nvar Int b = 2;\n"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn empty_lua_output_creates_no_expansion() {
    let prepared = prepare("%lua { local x = 1 + 2 }\n");

    assert!(prepared.diagnostics.is_empty(), "{:#?}", prepared.diagnostics);
    assert!(prepared.expansions.is_empty());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn generated_source_reaches_the_normal_parser() {
    let unit = translation_unit(
        r#"%lua {
uy.emit("var Int generated = 42;")
}
onflag {
    say generated;
}
"#,
    );

    let (sprite, diagnostics) = parser::parse(&unit);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert!(sprite.vars.keys().any(|name| name.as_str() == "generated"));
    assert_eq!(sprite.events.len(), 1);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn generated_source_keeps_preprocessor_support() {
    let unit = translation_unit(
        r#"%define ANSWER 42
%lua {
uy.emit("var Int generated = ANSWER;")
}
"#,
    );

    let (sprite, diagnostics) = parser::parse(&unit);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert!(sprite.vars.keys().any(|name| name.as_str() == "generated"));
}

fn translation_unit(source: &str) -> TranslationUnit {
    let path = PathBuf::from("/project/stage.gs");
    let fs = Rc::new(RefCell::new(TestFs(HashMap::from([(
        path.clone(),
        source.as_bytes().to_vec(),
    )]))));

    TranslationUnit::new(fs, path).expect("test translation unit should load")
}

struct TestFs(HashMap<PathBuf, Vec<u8>>);

impl VFS for TestFs {
    fn read_dir(&mut self, _path: &Path) -> io::Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }

    fn read_file<'a>(&'a mut self, path: &Path) -> io::Result<Box<dyn io::Read + 'a>> {
        let content = self
            .0
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.display().to_string()))?
            .clone();
        Ok(Box::new(Cursor::new(content)))
    }

    fn is_dir(&self, _path: &Path) -> bool {
        false
    }

    fn is_file(&self, path: &Path) -> bool {
        self.0.contains_key(path)
    }

    fn glob(&mut self, _pattern: &str) -> io::Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }
}
