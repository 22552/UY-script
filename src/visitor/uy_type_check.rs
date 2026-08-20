use fxhash::{FxHashMap, FxHashSet};
use logos::Span;

use crate::{
    ast::*,
    diagnostic::{DiagnosticKind, SpriteDiagnostics},
    misc::SmolStr,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ty {
    Dynamic,
    Number,
    Int,
    String,
    Bool,
    Struct(SmolStr),
}

impl Ty {
    fn from_ast(ty: &Type) -> Self {
        match ty {
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
        let name: SmolStr = match self {
            Self::Dynamic => return Type::Value,
            Self::Number => "Number".into(),
            Self::Int => "Int".into(),
            Self::String => "String".into(),
            Self::Bool => "Bool".into(),
            Self::Struct(name) => name.clone(),
        };
        Type::Struct { name, span }
    }

    fn accepts(&self, other: &Self) -> bool {
        matches!(self, Self::Dynamic)
            || matches!(other, Self::Dynamic)
            || self == other
            || matches!((self, other), (Self::Number, Self::Int))
    }

    fn owned(&self) -> bool {
        matches!(self, Self::String | Self::Struct(_))
    }
}

struct Checker<'a> {
    sprite: &'a Sprite,
    stage: Option<&'a Sprite>,
    args: Option<&'a [Arg]>,
    locals: Option<&'a FxHashMap<SmolStr, Var>>,
    return_ty: Option<Ty>,
    diagnostics: &'a mut SpriteDiagnostics,
    moved: FxHashSet<SmolStr>,
}

impl Checker<'_> {
    fn var_ty(&self, name: &str) -> Ty {
        self.locals
            .and_then(|vars| vars.get(name))
            .or_else(|| self.sprite.vars.get(name))
            .or_else(|| self.stage.and_then(|stage| stage.vars.get(name)))
            .map(|var| Ty::from_ast(&var.type_))
            .unwrap_or(Ty::Dynamic)
    }

    fn list_ty(&self, name: &str) -> Ty {
        self.sprite
            .lists
            .get(name)
            .or_else(|| self.stage.and_then(|stage| stage.lists.get(name)))
            .map(|list| Ty::from_ast(&list.type_))
            .unwrap_or(Ty::Dynamic)
    }

    fn arg_ty(&self, name: &str) -> Ty {
        self.args
            .and_then(|args| args.iter().find(|arg| arg.name == name))
            .map(|arg| Ty::from_ast(&arg.type_))
            .unwrap_or(Ty::Dynamic)
    }

    fn literal_ty(&self, value: &Value, span: &Span) -> Ty {
        // goboscript currently lowers true/false to numeric constants in the
        // parser, so preserve their source spelling for UY's static checker.
        if let Some(bytes) = self.diagnostics.translation_unit.text.get(span.clone()) {
            if let Ok(source) = std::str::from_utf8(bytes) {
                if matches!(source.trim(), "true" | "false") {
                    return Ty::Bool;
                }
            }
        }
        match value {
            Value::Boolean(_) => Ty::Bool,
            Value::Number(number) if number.is_finite() && number.fract() == 0.0 => Ty::Int,
            Value::Number(_) => Ty::Number,
            Value::String(_) => Ty::String,
        }
    }

    fn mismatch(&mut self, expected: &Ty, given: &Ty, span: &Span) {
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

    fn moved_use(&mut self, name: &SmolStr, ty: &Ty, span: &Span) {
        // Keep the public diagnostic surface unchanged in v0.1. A moved value
        // is represented as a synthetic type so this still becomes a normal
        // compile-time error: "expected String, but got moved String".
        let moved = Ty::Struct(format!("moved {}", display_ty(ty)).into());
        self.diagnostics.report(
            DiagnosticKind::TypeMismatch {
                expected: ty.to_ast(span.clone()),
                given: moved.to_ast(span.clone()),
            },
            span,
        );
        let _ = name;
    }

    fn check_name(&mut self, name: &Name, is_arg: bool) {
        let base = name.basename();
        if self.moved.contains(base) {
            let ty = if is_arg {
                self.arg_ty(base)
            } else {
                self.var_ty(base)
            };
            self.moved_use(base, &ty, &name.basespan());
        }
    }

    fn expr_ty(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::Value { value, span } => self.literal_ty(value, span),
            Expr::Name(name) => {
                self.check_name(name, false);
                if name.fieldname().is_some() {
                    Ty::Dynamic
                } else {
                    self.var_ty(name.basename())
                }
            }
            Expr::Arg(name) => {
                self.check_name(name, true);
                self.arg_ty(name.basename())
            }
            Expr::Dot { lhs, .. } => {
                self.expr_ty(lhs);
                Ty::Dynamic
            }
            Expr::Property { object, .. } => {
                self.expr_ty(object);
                Ty::Dynamic
            }
            Expr::Repr { args, .. } => {
                for arg in args {
                    self.expr_ty(arg);
                }
                Ty::Dynamic
            }
            Expr::FuncCall {
                name,
                args,
                kwargs,
                ..
            } => {
                let sig = self
                    .sprite
                    .func_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.call_args(&sig, args, kwargs);
                self.sprite
                    .funcs
                    .get(name)
                    .map(|func| Ty::from_ast(&func.type_))
                    .unwrap_or(Ty::Dynamic)
            }
            Expr::UnOp { op, opr, .. } => {
                let operand = self.expr_ty(opr);
                use crate::blocks::UnOp;
                match op {
                    UnOp::Not => Ty::Bool,
                    UnOp::Length | UnOp::Floor | UnOp::Ceil => Ty::Int,
                    UnOp::Minus if matches!(operand, Ty::Int) => Ty::Int,
                    _ => Ty::Number,
                }
            }
            Expr::BinOp { op, lhs, rhs, .. } => {
                let lhs_ty = self.expr_ty(lhs);
                let rhs_ty = self.expr_ty(rhs);
                use crate::blocks::BinOp;
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Mod | BinOp::FloorDiv
                        if matches!(lhs_ty, Ty::Int) && matches!(rhs_ty, Ty::Int) => Ty::Int,
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::FloorDiv => Ty::Number,
                    BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Eq
                    | BinOp::And
                    | BinOp::Or
                    | BinOp::In
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::Ne => Ty::Bool,
                    BinOp::Join => Ty::String,
                    BinOp::Of => {
                        if let Expr::Name(name) = lhs.as_ref() {
                            let ty = self.list_ty(name.basename());
                            if !matches!(ty, Ty::Dynamic) {
                                return ty;
                            }
                        }
                        Ty::String
                    }
                }
            }
            Expr::StructLiteral { name, fields, .. } => {
                for field in fields {
                    self.expr_ty(&field.value);
                }
                Ty::Struct(name.clone())
            }
        }
    }

    fn move_from(&mut self, expr: &Expr, expected: &Ty, destination: Option<&Name>) {
        if !expected.owned() {
            return;
        }
        let source = match expr {
            Expr::Name(name) | Expr::Arg(name) if name.fieldname().is_none() => name,
            _ => return,
        };
        if destination.is_some_and(|dest| dest.basename() == source.basename()) {
            return;
        }
        self.moved.insert(source.basename().clone());
    }

    fn call_args(
        &mut self,
        signature: &[(SmolStr, Ty)],
        args: &[Expr],
        kwargs: &FxHashMap<SmolStr, (Span, Expr)>,
    ) {
        for ((_, expected), expr) in signature.iter().zip(args) {
            let given = self.expr_ty(expr);
            self.mismatch(expected, &given, &expr.span());
            if expected.accepts(&given) {
                self.move_from(expr, expected, None);
            }
        }
        for (name, expected) in signature {
            if let Some((_, expr)) = kwargs.get(name) {
                let given = self.expr_ty(expr);
                self.mismatch(expected, &given, &expr.span());
                if expected.accepts(&given) {
                    self.move_from(expr, expected, None);
                }
            }
        }
    }

    fn stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Repeat { times, body } => {
                self.expr_ty(times);
                self.stmts(body);
            }
            Stmt::Forever { body, .. } => self.stmts(body),
            Stmt::Until { cond, body } => {
                self.expr_ty(cond);
                self.stmts(body);
            }
            Stmt::Branch {
                cond,
                if_body,
                else_body,
            } => {
                self.expr_ty(cond);
                let before = self.moved.clone();
                self.stmts(if_body);
                let moved_in_if = self.moved.clone();
                self.moved = before;
                self.stmts(else_body);
                self.moved.extend(moved_in_if);
            }
            Stmt::SetVar {
                name, value, type_, ..
            } => {
                let explicit = Ty::from_ast(type_);
                let expected = if matches!(explicit, Ty::Dynamic) {
                    self.var_ty(name.basename())
                } else {
                    explicit
                };
                let given = self.expr_ty(value);
                self.mismatch(&expected, &given, &value.span());
                if expected.accepts(&given) {
                    self.move_from(value, &expected, Some(name));
                }
                if name.fieldname().is_none() {
                    // Assignment creates a fresh value, so a previously moved
                    // destination becomes usable again.
                    self.moved.remove(name.basename());
                }
            }
            Stmt::ChangeVar { name, value } => {
                self.check_name(name, false);
                self.expr_ty(value);
            }
            Stmt::Show(name) | Stmt::Hide(name) => self.check_name(name, false),
            Stmt::AddToList { name, value } => {
                let expected = self.list_ty(name.basename());
                let given = self.expr_ty(value);
                self.mismatch(&expected, &given, &value.span());
                if expected.accepts(&given) {
                    self.move_from(value, &expected, None);
                }
            }
            Stmt::DeleteList(_) => {}
            Stmt::DeleteListIndex { index, .. } => {
                self.expr_ty(index);
            }
            Stmt::InsertAtList {
                name, index, value,
            }
            | Stmt::SetListIndex {
                name, index, value,
            } => {
                self.expr_ty(index);
                let expected = self.list_ty(name.basename());
                let given = self.expr_ty(value);
                self.mismatch(&expected, &given, &value.span());
                if expected.accepts(&given) {
                    self.move_from(value, &expected, None);
                }
            }
            Stmt::Block { args, kwargs, .. } => {
                for arg in args {
                    self.expr_ty(arg);
                }
                for (_, arg) in kwargs.values() {
                    self.expr_ty(arg);
                }
            }
            Stmt::ProcCall {
                name, args, kwargs, ..
            } => {
                let sig = self
                    .sprite
                    .proc_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.call_args(&sig, args, kwargs);
            }
            Stmt::FuncCall {
                name, args, kwargs, ..
            } => {
                let sig = self
                    .sprite
                    .func_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.call_args(&sig, args, kwargs);
            }
            Stmt::Return { value, .. } => {
                let given = self.expr_ty(value);
                if let Some(expected) = self.return_ty.clone() {
                    self.mismatch(&expected, &given, &value.span());
                }
            }
        }
    }
}

fn display_ty(ty: &Ty) -> String {
    match ty {
        Ty::Dynamic => "value".into(),
        Ty::Number => "Number".into(),
        Ty::Int => "Int".into(),
        Ty::String => "String".into(),
        Ty::Bool => "Bool".into(),
        Ty::Struct(name) => name.to_string(),
    }
}

fn signature(args: &[Arg]) -> Vec<(SmolStr, Ty)> {
    args.iter()
        .map(|arg| (arg.name.clone(), Ty::from_ast(&arg.type_)))
        .collect()
}

fn const_ty(expr: &ConstExpr, d: &SpriteDiagnostics) -> Ty {
    match expr {
        ConstExpr::Value { value, span } => {
            if let Some(bytes) = d.translation_unit.text.get(span.clone()) {
                if let Ok(source) = std::str::from_utf8(bytes) {
                    if matches!(source.trim(), "true" | "false") {
                        return Ty::Bool;
                    }
                }
            }
            match value {
                Value::Boolean(_) => Ty::Bool,
                Value::Number(number) if number.is_finite() && number.fract() == 0.0 => Ty::Int,
                Value::Number(_) => Ty::Number,
                Value::String(_) => Ty::String,
            }
        }
        ConstExpr::EnumVariant { .. } => Ty::Dynamic,
        ConstExpr::StructLiteral { name, .. } => Ty::Struct(name.clone()),
    }
}

fn check_defaults(sprite: &Sprite, d: &mut SpriteDiagnostics) {
    for var in sprite.vars.values() {
        let expected = Ty::from_ast(&var.type_);
        if let Some(default) = &var.default {
            let given = const_ty(default, d);
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
        let expected = Ty::from_ast(&list.type_);
        if let Some(ListDefault::Values(values)) = &list.default {
            for value in values {
                let given = const_ty(value, d);
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

fn check_sprite(sprite: &Sprite, stage: Option<&Sprite>, d: &mut SpriteDiagnostics) {
    check_defaults(sprite, d);

    for proc in sprite.procs.values() {
        let mut checker = Checker {
            sprite,
            stage,
            args: sprite.proc_args.get(&proc.name).map(Vec::as_slice),
            locals: sprite.proc_locals.get(&proc.name),
            return_ty: None,
            diagnostics: d,
            moved: FxHashSet::default(),
        };
        if let Some(body) = sprite.proc_definitions.get(&proc.name) {
            checker.stmts(body);
        }
    }
    for func in sprite.funcs.values() {
        let mut checker = Checker {
            sprite,
            stage,
            args: sprite.func_args.get(&func.name).map(Vec::as_slice),
            locals: sprite.func_locals.get(&func.name),
            return_ty: Some(Ty::from_ast(&func.type_)),
            diagnostics: d,
            moved: FxHashSet::default(),
        };
        if let Some(body) = sprite.func_definitions.get(&func.name) {
            checker.stmts(body);
        }
    }
    for event in &sprite.events {
        let mut checker = Checker {
            sprite,
            stage,
            args: None,
            locals: None,
            return_ty: None,
            diagnostics: d,
            moved: FxHashSet::default(),
        };
        checker.stmts(&event.body);
    }
}

fn erase_ty(ty: &mut Type) {
    if matches!(
        ty,
        Type::Struct { name, .. }
            if matches!(
                name.as_str(),
                "Number" | "Int" | "String" | "Bool" | "Boolean" | "Value" | "Any"
            )
    ) {
        *ty = Type::Value;
    }
}

fn erase_stmts(stmts: &mut [Stmt]) {
    for stmt in stmts {
        match stmt {
            Stmt::Repeat { body, .. }
            | Stmt::Forever { body, .. }
            | Stmt::Until { body, .. } => erase_stmts(body),
            Stmt::Branch {
                if_body, else_body, ..
            } => {
                erase_stmts(if_body);
                erase_stmts(else_body);
            }
            Stmt::SetVar { type_, .. } => erase_ty(type_),
            _ => {}
        }
    }
}

fn erase_sprite(sprite: &mut Sprite) {
    for var in sprite.vars.values_mut() {
        erase_ty(&mut var.type_);
    }
    for list in sprite.lists.values_mut() {
        erase_ty(&mut list.type_);
    }
    for locals in sprite.proc_locals.values_mut() {
        for var in locals.values_mut() {
            erase_ty(&mut var.type_);
        }
    }
    for locals in sprite.func_locals.values_mut() {
        for var in locals.values_mut() {
            erase_ty(&mut var.type_);
        }
    }
    for args in sprite.proc_args.values_mut() {
        for arg in args {
            erase_ty(&mut arg.type_);
        }
    }
    for args in sprite.func_args.values_mut() {
        for arg in args {
            erase_ty(&mut arg.type_);
        }
    }
    for func in sprite.funcs.values_mut() {
        erase_ty(&mut func.type_);
    }
    for body in sprite.proc_definitions.values_mut() {
        erase_stmts(body);
    }
    for body in sprite.func_definitions.values_mut() {
        erase_stmts(body);
    }
    for event in &mut sprite.events {
        erase_stmts(&mut event.body);
    }
}

/// Check UY primitive types and ownership, then erase primitive annotations so
/// the existing goboscript -> Scratch lowering stays unchanged.
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

    erase_sprite(&mut project.stage);
    for sprite in project.sprites.values_mut() {
        erase_sprite(sprite);
    }
}
