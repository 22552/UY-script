use fxhash::{FxHashMap, FxHashSet};
use logos::Span;

use crate::{
    ast::*,
    diagnostic::{DiagnosticKind, SpriteDiagnostics},
    misc::SmolStr,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum StaticType {
    Dynamic,
    Number,
    Int,
    String,
    Bool,
    Struct(SmolStr),
}

impl StaticType {
    fn from_ast(type_: &Type) -> Self {
        match type_ {
            Type::Value => Self::Dynamic,
            Type::Struct { name, .. } => match name.as_str() {
                "Number" => Self::Number,
                "Int" => Self::Int,
                "String" => Self::String,
                "Bool" | "Boolean" => Self::Bool,
                "Value" | "Any" => Self::Dynamic,
                _ => Self::Struct(name.clone()),
            },
        }
    }

    fn to_ast(&self, span: Span) -> Type {
        match self {
            Self::Dynamic => Type::Value,
            Self::Number => Type::Struct {
                name: "Number".into(),
                span,
            },
            Self::Int => Type::Struct {
                name: "Int".into(),
                span,
            },
            Self::String => Type::Struct {
                name: "String".into(),
                span,
            },
            Self::Bool => Type::Struct {
                name: "Bool".into(),
                span,
            },
            Self::Struct(name) => Type::Struct {
                name: name.clone(),
                span,
            },
        }
    }

    fn accepts(&self, given: &Self) -> bool {
        if matches!(self, Self::Dynamic) || matches!(given, Self::Dynamic) {
            return true;
        }
        matches!((self, given), (Self::Number, Self::Int)) || self == given
    }

    fn is_owned(&self) -> bool {
        matches!(self, Self::String | Self::Struct(_))
    }
}

struct Checker<'a> {
    sprite: &'a Sprite,
    stage: Option<&'a Sprite>,
    args: Option<&'a [Arg]>,
    locals: Option<&'a FxHashMap<SmolStr, Var>>,
    return_type: Option<StaticType>,
    diagnostics: &'a mut SpriteDiagnostics,
    moved: FxHashSet<SmolStr>,
}

impl<'a> Checker<'a> {
    fn get_var_type(&self, name: &str) -> StaticType {
        self.locals
            .and_then(|locals| locals.get(name))
            .or_else(|| self.sprite.vars.get(name))
            .or_else(|| self.stage.and_then(|stage| stage.vars.get(name)))
            .map(|var| StaticType::from_ast(&var.type_))
            .unwrap_or(StaticType::Dynamic)
    }

    fn get_list_type(&self, name: &str) -> StaticType {
        self.sprite
            .lists
            .get(name)
            .or_else(|| self.stage.and_then(|stage| stage.lists.get(name)))
            .map(|list| StaticType::from_ast(&list.type_))
            .unwrap_or(StaticType::Dynamic)
    }

    fn get_arg_type(&self, name: &str) -> StaticType {
        self.args
            .and_then(|args| args.iter().find(|arg| arg.name == name))
            .map(|arg| StaticType::from_ast(&arg.type_))
            .unwrap_or(StaticType::Dynamic)
    }

    fn source_fragment(&self, span: &Span) -> Option<&str> {
        let bytes = self.diagnostics.translation_unit.text.get(span.clone())?;
        std::str::from_utf8(bytes).ok()
    }

    fn value_type(&self, value: &Value, span: &Span) -> StaticType {
        if let Some(source) = self.source_fragment(span) {
            match source.trim() {
                "true" | "false" => return StaticType::Bool,
                _ => {}
            }
        }
        match value {
            Value::Boolean(_) => StaticType::Bool,
            Value::Number(number) => {
                if number.is_finite() && number.fract() == 0.0 {
                    StaticType::Int
                } else {
                    StaticType::Number
                }
            }
            Value::String(_) => StaticType::String,
        }
    }

    fn report_mismatch(&mut self, expected: &StaticType, given: &StaticType, span: &Span) {
        if expected.accepts(given) {
            return;
        }
        self.diagnostics.report(
            DiagnosticKind::TypeMismatch {
                expected: expected.to_ast(span.clone()),
                given: given.to_ast(span.clone()),
            },
            span,
        );
    }

    fn report_moved(&mut self, name: &SmolStr, type_: &StaticType, span: &Span) {
        let given = match type_ {
            StaticType::Struct(type_name) => StaticType::Struct(
                format!("moved {type_name}").into(),
            ),
            StaticType::String => StaticType::Struct {
                // This branch is intentionally handled below; keeping the match exhaustive.
                // (Constructing the diagnostic through `to_ast` avoids adding a new public
                // diagnostic variant in the first UY-script compatibility release.)
                name: "moved String".into(),
            }
            .into_static(),
            _ => StaticType::Struct(format!("moved {name}").into()),
        };
        self.diagnostics.report(
            DiagnosticKind::TypeMismatch {
                expected: type_.to_ast(span.clone()),
                given: given.to_ast(span.clone()),
            },
            span,
        );
    }

    fn check_name_available(&mut self, name: &Name) {
        let basename = name.basename();
        if self.moved.contains(basename) {
            let type_ = self.get_var_type(basename);
            self.report_moved(basename, &type_, &name.basespan());
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> StaticType {
        match expr {
            Expr::Value { value, span } => self.value_type(value, span),
            Expr::Name(name) => {
                self.check_name_available(name);
                if name.fieldname().is_some() {
                    StaticType::Dynamic
                } else {
                    self.get_var_type(name.basename())
                }
            }
            Expr::Arg(name) => {
                let basename = name.basename();
                if self.moved.contains(basename) {
                    let type_ = self.get_arg_type(basename);
                    self.report_moved(basename, &type_, &name.basespan());
                }
                self.get_arg_type(basename)
            }
            Expr::Dot { lhs, .. } | Expr::Property { object: lhs, .. } => {
                self.infer_expr(lhs);
                StaticType::Dynamic
            }
            Expr::Repr { args, .. } => {
                for arg in args {
                    self.infer_expr(arg);
                }
                StaticType::Dynamic
            }
            Expr::FuncCall {
                name,
                args,
                kwargs,
                ..
            } => {
                let signature = self
                    .sprite
                    .func_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.check_call_args(&signature, args, kwargs);
                self.sprite
                    .funcs
                    .get(name)
                    .map(|func| StaticType::from_ast(&func.type_))
                    .unwrap_or(StaticType::Dynamic)
            }
            Expr::UnOp { op, opr, .. } => {
                let operand = self.infer_expr(opr);
                match op {
                    crate::blocks::UnOp::Not => StaticType::Bool,
                    crate::blocks::UnOp::Length => StaticType::Int,
                    crate::blocks::UnOp::Floor | crate::blocks::UnOp::Ceil => StaticType::Int,
                    crate::blocks::UnOp::Minus if matches!(operand, StaticType::Int) => StaticType::Int,
                    _ => StaticType::Number,
                }
            }
            Expr::BinOp { op, lhs, rhs, .. } => {
                let lhs_type = self.infer_expr(lhs);
                let rhs_type = self.infer_expr(rhs);
                use crate::blocks::BinOp;
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Mod | BinOp::FloorDiv
                        if matches!(lhs_type, StaticType::Int)
                            && matches!(rhs_type, StaticType::Int) =>
                    {
                        StaticType::Int
                    }
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::FloorDiv => StaticType::Number,
                    BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Eq
                    | BinOp::And
                    | BinOp::Or
                    | BinOp::In
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::Ne => StaticType::Bool,
                    BinOp::Join => StaticType::String,
                    BinOp::Of => {
                        if let Expr::Name(name) = lhs.as_ref() {
                            let list_type = self.get_list_type(name.basename());
                            if !matches!(list_type, StaticType::Dynamic) {
                                return list_type;
                            }
                        }
                        StaticType::String
                    }
                }
            }
            Expr::StructLiteral { name, fields, .. } => {
                for field in fields {
                    self.infer_expr(&field.value);
                }
                StaticType::Struct(name.clone())
            }
        }
    }

    fn mark_move(&mut self, expr: &Expr, expected: &StaticType, destination: Option<&Name>) {
        if !expected.is_owned() {
            return;
        }
        let name = match expr {
            Expr::Name(name) | Expr::Arg(name) if name.fieldname().is_none() => name,
            _ => return,
        };
        if destination.is_some_and(|dest| dest.basename() == name.basename()) {
            return;
        }
        self.moved.insert(name.basename().clone());
    }

    fn check_call_args(
        &mut self,
        signature: &[(SmolStr, StaticType)],
        args: &[Expr],
        kwargs: &FxHashMap<SmolStr, (Span, Expr)>,
    ) {
        for ((_, expected), expr) in signature.iter().zip(args) {
            let given = self.infer_expr(expr);
            self.report_mismatch(expected, &given, &expr.span());
            if expected.accepts(&given) {
                self.mark_move(expr, expected, None);
            }
        }
        for (arg_name, expected) in signature {
            if let Some((_, expr)) = kwargs.get(arg_name) {
                let given = self.infer_expr(expr);
                self.report_mismatch(expected, &given, &expr.span());
                if expected.accepts(&given) {
                    self.mark_move(expr, expected, None);
                }
            }
        }
    }

    fn check_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Repeat { times, body } => {
                self.infer_expr(times);
                self.check_stmts(body);
            }
            Stmt::Forever { body, .. } => self.check_stmts(body),
            Stmt::Branch {
                cond,
                if_body,
                else_body,
            } => {
                self.infer_expr(cond);
                let before = self.moved.clone();
                self.check_stmts(if_body);
                let if_moved = self.moved.clone();
                self.moved = before;
                self.check_stmts(else_body);
                self.moved.extend(if_moved);
            }
            Stmt::Until { cond, body } => {
                self.infer_expr(cond);
                self.check_stmts(body);
            }
            Stmt::SetVar {
                name,
                value,
                type_,
                ..
            } => {
                let explicit = StaticType::from_ast(type_);
                let expected = if matches!(explicit, StaticType::Dynamic) {
                    self.get_var_type(name.basename())
                } else {
                    explicit
                };
                let given = self.infer_expr(value);
                self.report_mismatch(&expected, &given, &value.span());
                if expected.accepts(&given) {
                    self.mark_move(value, &expected, Some(name));
                }
                if name.fieldname().is_none() {
                    self.moved.remove(name.basename());
                }
            }
            Stmt::ChangeVar { name, value } => {
                self.check_name_available(name);
                self.infer_expr(value);
            }
            Stmt::Show(name) | Stmt::Hide(name) | Stmt::DeleteList(name) => {
                self.check_name_available(name);
            }
            Stmt::AddToList { name, value } => {
                let expected = self.get_list_type(name.basename());
                let given = self.infer_expr(value);
                self.report_mismatch(&expected, &given, &value.span());
                if expected.accepts(&given) {
                    self.mark_move(value, &expected, None);
                }
            }
            Stmt::DeleteListIndex { index, .. } => {
                self.infer_expr(index);
            }
            Stmt::InsertAtList {
                name,
                index,
                value,
            }
            | Stmt::SetListIndex {
                name,
                index,
                value,
            } => {
                self.infer_expr(index);
                let expected = self.get_list_type(name.basename());
                let given = self.infer_expr(value);
                self.report_mismatch(&expected, &given, &value.span());
                if expected.accepts(&given) {
                    self.mark_move(value, &expected, None);
                }
            }
            Stmt::Block { args, kwargs, .. } => {
                for arg in args {
                    self.infer_expr(arg);
                }
                for (_, arg) in kwargs.values() {
                    self.infer_expr(arg);
                }
            }
            Stmt::ProcCall {
                name,
                args,
                kwargs,
                ..
            } => {
                let signature = self
                    .sprite
                    .proc_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.check_call_args(&signature, args, kwargs);
            }
            Stmt::FuncCall {
                name,
                args,
                kwargs,
                ..
            } => {
                let signature = self
                    .sprite
                    .func_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.check_call_args(&signature, args, kwargs);
            }
            Stmt::Return { value, .. } => {
                let given = self.infer_expr(value);
                if let Some(expected) = self.return_type.clone() {
                    self.report_mismatch(&expected, &given, &value.span());
                }
            }
        }
    }
}

// Small helper used only to build the temporary ownership diagnostic without
// expanding the public diagnostic enum in the compatibility-first release.
trait IntoStaticType {
    fn into_static(self) -> StaticType;
}

impl IntoStaticType for Type {
    fn into_static(self) -> StaticType {
        StaticType::from_ast(&self)
    }
}

fn signature(args: &[Arg]) -> Vec<(SmolStr, StaticType)> {
    args.iter()
        .map(|arg| (arg.name.clone(), StaticType::from_ast(&arg.type_)))
        .collect()
}

fn check_defaults(sprite: &Sprite, d: &mut SpriteDiagnostics) {
    for var in sprite.vars.values() {
        let expected = StaticType::from_ast(&var.type_);
        if matches!(expected, StaticType::Dynamic) {
            continue;
        }
        if let Some(default) = &var.default {
            let given = const_expr_type(default, d);
            if !expected.accepts(&given) {
                d.report(
                    DiagnosticKind::TypeMismatch {
                        expected: expected.to_ast(default.span()),
                        given: given.to_ast(default.span()),
                    },
                    &default.span(),
                );
            }
        }
    }

    for list in sprite.lists.values() {
        let expected = StaticType::from_ast(&list.type_);
        if matches!(expected, StaticType::Dynamic) {
            continue;
        }
        if let Some(ListDefault::Values(values)) = &list.default {
            for value in values {
                let given = const_expr_type(value, d);
                if !expected.accepts(&given) {
                    d.report(
                        DiagnosticKind::TypeMismatch {
                            expected: expected.to_ast(value.span()),
                            given: given.to_ast(value.span()),
                        },
                        &value.span(),
                    );
                }
            }
        }
    }
}

fn const_expr_type(expr: &ConstExpr, d: &SpriteDiagnostics) -> StaticType {
    match expr {
        ConstExpr::Value { value, span } => {
            if let Some(bytes) = d.translation_unit.text.get(span.clone()) {
                if let Ok(source) = std::str::from_utf8(bytes) {
                    if matches!(source.trim(), "true" | "false") {
                        return StaticType::Bool;
                    }
                }
            }
            match value {
                Value::Boolean(_) => StaticType::Bool,
                Value::Number(number) if number.is_finite() && number.fract() == 0.0 => {
                    StaticType::Int
                }
                Value::Number(_) => StaticType::Number,
                Value::String(_) => StaticType::String,
            }
        }
        ConstExpr::EnumVariant { .. } => StaticType::Dynamic,
        ConstExpr::StructLiteral { name, .. } => StaticType::Struct(name.clone()),
    }
}

fn check_sprite(sprite: &Sprite, stage: Option<&Sprite>, d: &mut SpriteDiagnostics) {
    check_defaults(sprite, d);

    for proc in sprite.procs.values() {
        let mut checker = Checker {
            sprite,
            stage,
            args: sprite.proc_args.get(&proc.name).map(Vec::as_slice),
            locals: sprite.proc_locals.get(&proc.name),
            return_type: None,
            diagnostics: d,
            moved: FxHashSet::default(),
        };
        if let Some(body) = sprite.proc_definitions.get(&proc.name) {
            checker.check_stmts(body);
        }
    }

    for func in sprite.funcs.values() {
        let mut checker = Checker {
            sprite,
            stage,
            args: sprite.func_args.get(&func.name).map(Vec::as_slice),
            locals: sprite.func_locals.get(&func.name),
            return_type: Some(StaticType::from_ast(&func.type_)),
            diagnostics: d,
            moved: FxHashSet::default(),
        };
        if let Some(body) = sprite.func_definitions.get(&func.name) {
            checker.check_stmts(body);
        }
    }

    for event in &sprite.events {
        let mut checker = Checker {
            sprite,
            stage,
            args: None,
            locals: None,
            return_type: None,
            diagnostics: d,
            moved: FxHashSet::default(),
        };
        checker.check_stmts(&event.body);
    }
}

fn is_uy_primitive(type_: &Type) -> bool {
    matches!(
        type_,
        Type::Struct { name, .. }
            if matches!(name.as_str(), "Number" | "Int" | "String" | "Bool" | "Boolean" | "Value" | "Any")
    )
}

fn erase_type(type_: &mut Type) {
    if is_uy_primitive(type_) {
        *type_ = Type::Value;
    }
}

fn erase_stmt_types(stmts: &mut [Stmt]) {
    for stmt in stmts {
        match stmt {
            Stmt::Repeat { body, .. }
            | Stmt::Forever { body, .. }
            | Stmt::Until { body, .. } => erase_stmt_types(body),
            Stmt::Branch {
                if_body, else_body, ..
            } => {
                erase_stmt_types(if_body);
                erase_stmt_types(else_body);
            }
            Stmt::SetVar { type_, .. } => erase_type(type_),
            _ => {}
        }
    }
}

fn erase_sprite_types(sprite: &mut Sprite) {
    for var in sprite.vars.values_mut() {
        erase_type(&mut var.type_);
    }
    for list in sprite.lists.values_mut() {
        erase_type(&mut list.type_);
    }
    for var in sprite
        .proc_locals
        .values_mut()
        .flat_map(|locals| locals.values_mut())
    {
        erase_type(&mut var.type_);
    }
    for var in sprite
        .func_locals
        .values_mut()
        .flat_map(|locals| locals.values_mut())
    {
        erase_type(&mut var.type_);
    }
    for args in sprite.proc_args.values_mut() {
        for arg in args {
            erase_type(&mut arg.type_);
        }
    }
    for args in sprite.func_args.values_mut() {
        for arg in args {
            erase_type(&mut arg.type_);
        }
    }
    for func in sprite.funcs.values_mut() {
        erase_type(&mut func.type_);
    }
    for body in sprite.proc_definitions.values_mut() {
        erase_stmt_types(body);
    }
    for body in sprite.func_definitions.values_mut() {
        erase_stmt_types(body);
    }
    for event in &mut sprite.events {
        erase_stmt_types(&mut event.body);
    }
}

/// UY-script's compatibility-first static analysis pass.
///
/// Primitive annotations reuse goboscript's existing type syntax, e.g.
/// `String name = "UY";`, `Number x = 1;`, and `func f(Int x) Int { ... }`.
/// They are checked here and then erased back to `Type::Value` before the
/// original goboscript lowering/codegen pipeline runs, keeping Scratch output
/// semantics unchanged.
pub fn visit_project(
    project: &mut Project,
    stage_diagnostics: &mut SpriteDiagnostics,
    sprites_diagnostics: &mut FxHashMap<SmolStr, SpriteDiagnostics>,
) {
    check_sprite(&project.stage, None, stage_diagnostics);
    for (name, sprite) in &project.sprites {
        if let Some(d) = sprites_diagnostics.get_mut(name) {
            check_sprite(sprite, Some(&project.stage), d);
        }
    }

    erase_sprite_types(&mut project.stage);
    for sprite in project.sprites.values_mut() {
        erase_sprite_types(sprite);
    }
}
