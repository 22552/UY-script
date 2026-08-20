use fxhash::{FxHashMap, FxHashSet};
use logos::Span;

use crate::{
    ast::*,
    diagnostic::{DiagnosticKind, SpriteDiagnostics},
    misc::SmolStr,
};

const REF_ARG_PREFIX: &str = "@uyref:";
const BORROW_PREFIX: &str = "@uyborrow:";

#[derive(Clone, Debug, PartialEq, Eq)]
enum BaseTy {
    Dynamic,
    Number,
    Int,
    String,
    Bool,
    Struct(SmolStr),
}

impl BaseTy {
    fn from_name(name: &str) -> Self {
        match name {
            "Number" => Self::Number,
            "Int" => Self::Int,
            "String" => Self::String,
            "Bool" | "Boolean" => Self::Bool,
            "Value" | "Any" => Self::Dynamic,
            _ => Self::Struct(name.into()),
        }
    }

    fn from_ast(type_: &Type) -> Self {
        match type_ {
            Type::Value => Self::Dynamic,
            Type::Struct { name, .. } => Self::from_name(name),
        }
    }

    fn accepts(&self, given: &Self) -> bool {
        matches!(self, Self::Dynamic)
            || matches!(given, Self::Dynamic)
            || self == given
            || matches!((self, given), (Self::Number, Self::Int))
    }

    fn owned(&self) -> bool {
        matches!(self, Self::String | Self::Struct(_))
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

    fn display(&self) -> String {
        match self {
            Self::Dynamic => "value".into(),
            Self::Number => "Number".into(),
            Self::Int => "Int".into(),
            Self::String => "String".into(),
            Self::Bool => "Bool".into(),
            Self::Struct(name) => name.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct RefSpec {
    mutable: bool,
    inner: BaseTy,
    logical_name: SmolStr,
}

impl RefSpec {
    fn to_ast(&self, span: Span) -> Type {
        let prefix = if self.mutable { "&mut " } else { "&" };
        Type::Struct {
            name: format!("{prefix}{}", self.inner.display()).into(),
            span,
        }
    }
}

#[derive(Clone, Debug)]
enum ParamTy {
    Value(BaseTy),
    Ref(RefSpec),
}

#[derive(Clone, Debug)]
struct Param {
    name: SmolStr,
    type_: ParamTy,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    is_arg: bool,
    name: SmolStr,
}

impl Key {
    fn var(name: SmolStr) -> Self {
        Self {
            is_arg: false,
            name,
        }
    }

    fn arg(name: SmolStr) -> Self {
        Self { is_arg: true, name }
    }
}

#[derive(Clone, Debug)]
struct BorrowExpr {
    mutable: bool,
    key: Key,
    span: Span,
}

#[derive(Clone, Debug, Default)]
struct BorrowScope {
    shared: FxHashSet<Key>,
    mutable: FxHashSet<Key>,
}

fn decode_ref_arg(name: &str) -> Option<RefSpec> {
    let rest = name.strip_prefix(REF_ARG_PREFIX)?;
    let mut parts = rest.splitn(3, ':');
    let mutable = match parts.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    let type_name = parts.next()?;
    let logical_name = parts.next()?;
    Some(RefSpec {
        mutable,
        inner: BaseTy::from_name(type_name),
        logical_name: logical_name.into(),
    })
}

fn decode_borrow_name(name: &str) -> Option<(bool, SmolStr)> {
    let rest = name.strip_prefix(BORROW_PREFIX)?;
    let (mutable, name) = rest.split_once(':')?;
    let mutable = match mutable {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    Some((mutable, name.into()))
}

fn param(arg: &Arg) -> Param {
    if let Some(spec) = decode_ref_arg(&arg.name) {
        Param {
            name: spec.logical_name.clone(),
            type_: ParamTy::Ref(spec),
        }
    } else {
        Param {
            name: arg.name.clone(),
            type_: ParamTy::Value(BaseTy::from_ast(&arg.type_)),
        }
    }
}

fn signature(args: &[Arg]) -> Vec<Param> {
    args.iter().map(param).collect()
}

struct Checker<'a> {
    sprite: &'a Sprite,
    stage: Option<&'a Sprite>,
    args: Option<&'a [Arg]>,
    return_type: Option<BaseTy>,
    diagnostics: &'a mut SpriteDiagnostics,
    moved: FxHashSet<Key>,
    borrow_scopes: Vec<BorrowScope>,
}

impl Checker<'_> {
    fn arg_param(&self, logical_name: &str) -> Option<Param> {
        self.args?
            .iter()
            .map(param)
            .find(|arg| arg.name == logical_name)
    }

    fn arg_type(&self, logical_name: &str) -> BaseTy {
        match self.arg_param(logical_name).map(|param| param.type_) {
            Some(ParamTy::Value(type_)) => type_,
            Some(ParamTy::Ref(spec)) => spec.inner,
            None => BaseTy::Dynamic,
        }
    }

    fn arg_is_ref(&self, logical_name: &str) -> bool {
        matches!(
            self.arg_param(logical_name).map(|param| param.type_),
            Some(ParamTy::Ref(_))
        )
    }

    fn arg_ref_is_mutable(&self, logical_name: &str) -> Option<bool> {
        match self.arg_param(logical_name).map(|param| param.type_) {
            Some(ParamTy::Ref(spec)) => Some(spec.mutable),
            _ => None,
        }
    }

    fn var_type(&self, name: &str) -> BaseTy {
        self.sprite
            .proc_locals
            .values()
            .find_map(|locals| locals.get(name))
            .or_else(|| {
                self.sprite
                    .func_locals
                    .values()
                    .find_map(|locals| locals.get(name))
            })
            .or_else(|| self.sprite.vars.get(name))
            .or_else(|| self.stage.and_then(|stage| stage.vars.get(name)))
            .map(|var| BaseTy::from_ast(&var.type_))
            .unwrap_or(BaseTy::Dynamic)
    }

    fn list_type(&self, name: &str) -> BaseTy {
        self.sprite
            .lists
            .get(name)
            .or_else(|| self.stage.and_then(|stage| stage.lists.get(name)))
            .map(|list| BaseTy::from_ast(&list.type_))
            .unwrap_or(BaseTy::Dynamic)
    }

    fn key_type(&self, key: &Key) -> BaseTy {
        if key.is_arg {
            self.arg_type(&key.name)
        } else {
            self.var_type(&key.name)
        }
    }

    fn borrowed_mutably(&self, key: &Key) -> bool {
        self.borrow_scopes
            .iter()
            .any(|scope| scope.mutable.contains(key))
    }

    fn borrowed_shared(&self, key: &Key) -> bool {
        self.borrow_scopes
            .iter()
            .any(|scope| scope.shared.contains(key))
    }

    fn borrowed_any(&self, key: &Key) -> bool {
        self.borrowed_mutably(key) || self.borrowed_shared(key)
    }

    fn state_error(&mut self, expected: Type, state: impl Into<SmolStr>, span: &Span) {
        self.diagnostics.report(
            DiagnosticKind::TypeMismatch {
                expected,
                given: Type::Struct {
                    name: state.into(),
                    span: span.clone(),
                },
            },
            span,
        );
    }

    fn borrow_expr(&self, expr: &Expr) -> Option<BorrowExpr> {
        match expr {
            Expr::Name(Name::Name { name, span }) => {
                let (mutable, logical_name) = decode_borrow_name(name)?;
                Some(BorrowExpr {
                    mutable,
                    key: Key::var(logical_name),
                    span: span.clone(),
                })
            }
            Expr::Arg(Name::Name { name, span }) => {
                let (mutable, logical_name) = decode_borrow_name(name)?;
                Some(BorrowExpr {
                    mutable,
                    key: Key::arg(logical_name),
                    span: span.clone(),
                })
            }
            _ => None,
        }
    }

    fn check_read(&mut self, key: &Key, span: &Span) {
        if self.borrowed_mutably(key) {
            let type_ = self.key_type(key);
            self.state_error(
                type_.to_ast(span.clone()),
                format!("mutably borrowed {}", type_.display()),
                span,
            );
        }
    }

    fn walk_expr(&mut self, expr: &Expr) {
        if let Some(borrow) = self.borrow_expr(expr) {
            let type_ = self.key_type(&borrow.key);
            self.state_error(
                type_.to_ast(borrow.span.clone()),
                format!(
                    "{}{} outside reference argument",
                    if borrow.mutable { "&mut " } else { "&" },
                    type_.display()
                ),
                &borrow.span,
            );
            return;
        }

        match expr {
            Expr::Value { .. } => {}
            Expr::Name(name) => {
                self.check_read(&Key::var(name.basename().clone()), &name.span());
            }
            Expr::Arg(name) => {
                self.check_read(&Key::arg(name.basename().clone()), &name.span());
            }
            Expr::Dot { lhs, .. } => self.walk_expr(lhs),
            Expr::Property { object, .. } => self.walk_expr(object),
            Expr::Repr { args, .. } => {
                for arg in args {
                    self.walk_expr(arg);
                }
            }
            Expr::FuncCall {
                name, args, kwargs, ..
            } => {
                let signature = self
                    .sprite
                    .func_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.check_call(&signature, args, kwargs);
            }
            Expr::UnOp { opr, .. } => self.walk_expr(opr),
            Expr::BinOp { lhs, rhs, .. } => {
                self.walk_expr(lhs);
                self.walk_expr(rhs);
            }
            Expr::StructLiteral { fields, .. } => {
                for field in fields {
                    self.walk_expr(&field.value);
                }
            }
        }
    }

    fn move_source(&self, expr: &Expr) -> Option<Key> {
        match expr {
            Expr::Name(Name::Name { name, .. }) if decode_borrow_name(name).is_none() => {
                Some(Key::var(name.clone()))
            }
            Expr::Arg(Name::Name { name, .. }) if decode_borrow_name(name).is_none() => {
                Some(Key::arg(name.clone()))
            }
            _ => None,
        }
    }

    fn mark_move(&mut self, expr: &Expr, expected: &BaseTy, destination: Option<&Name>) {
        if !expected.owned() {
            return;
        }
        let Some(key) = self.move_source(expr) else {
            return;
        };
        if destination.is_some_and(|name| !key.is_arg && name.basename() == &key.name) {
            return;
        }
        let span = expr.span();
        if key.is_arg && self.arg_is_ref(&key.name) {
            self.state_error(
                expected.to_ast(span.clone()),
                format!("borrowed parameter {}", expected.display()),
                &span,
            );
            return;
        }
        if self.borrowed_any(&key) {
            self.state_error(
                expected.to_ast(span.clone()),
                format!("borrowed {}", expected.display()),
                &span,
            );
            return;
        }
        self.moved.insert(key);
    }

    fn check_ref_arg(&mut self, expected: &RefSpec, expr: &Expr) {
        let Some(borrow) = self.borrow_expr(expr) else {
            let given = self
                .move_source(expr)
                .map(|key| self.key_type(&key))
                .unwrap_or(BaseTy::Dynamic);
            self.diagnostics.report(
                DiagnosticKind::TypeMismatch {
                    expected: expected.to_ast(expr.span()),
                    given: given.to_ast(expr.span()),
                },
                &expr.span(),
            );
            self.walk_expr(expr);
            return;
        };

        let given = self.key_type(&borrow.key);
        if !expected.inner.accepts(&given) || (expected.mutable && !borrow.mutable) {
            let given_ref = RefSpec {
                mutable: borrow.mutable,
                inner: given.clone(),
                logical_name: borrow.key.name.clone(),
            };
            self.diagnostics.report(
                DiagnosticKind::TypeMismatch {
                    expected: expected.to_ast(borrow.span.clone()),
                    given: given_ref.to_ast(borrow.span.clone()),
                },
                &borrow.span,
            );
            return;
        }

        if self.moved.contains(&borrow.key) {
            self.state_error(
                expected.to_ast(borrow.span.clone()),
                format!("moved {}", given.display()),
                &borrow.span,
            );
            return;
        }

        if borrow.key.is_arg && borrow.mutable {
            if let Some(false) = self.arg_ref_is_mutable(&borrow.key.name) {
                self.state_error(
                    expected.to_ast(borrow.span.clone()),
                    format!("immutably borrowed parameter {}", given.display()),
                    &borrow.span,
                );
                return;
            }
        }

        if borrow.mutable {
            if self.borrowed_any(&borrow.key) {
                self.state_error(
                    expected.to_ast(borrow.span.clone()),
                    format!("already borrowed {}", given.display()),
                    &borrow.span,
                );
                return;
            }
            if let Some(scope) = self.borrow_scopes.last_mut() {
                scope.mutable.insert(borrow.key);
            }
        } else {
            if self.borrowed_mutably(&borrow.key) {
                self.state_error(
                    expected.to_ast(borrow.span.clone()),
                    format!("mutably borrowed {}", given.display()),
                    &borrow.span,
                );
                return;
            }
            if let Some(scope) = self.borrow_scopes.last_mut() {
                scope.shared.insert(borrow.key);
            }
        }
    }

    fn check_value_arg(&mut self, expected: &BaseTy, expr: &Expr) {
        if let Some(borrow) = self.borrow_expr(expr) {
            let given = self.key_type(&borrow.key);
            let given_ref = RefSpec {
                mutable: borrow.mutable,
                inner: given,
                logical_name: borrow.key.name,
            };
            self.diagnostics.report(
                DiagnosticKind::TypeMismatch {
                    expected: expected.to_ast(borrow.span.clone()),
                    given: given_ref.to_ast(borrow.span.clone()),
                },
                &borrow.span,
            );
            return;
        }
        self.walk_expr(expr);
        self.mark_move(expr, expected, None);
    }

    fn check_param(&mut self, param: &Param, expr: &Expr) {
        match &param.type_ {
            ParamTy::Value(type_) => self.check_value_arg(type_, expr),
            ParamTy::Ref(spec) => self.check_ref_arg(spec, expr),
        }
    }

    fn check_call(
        &mut self,
        signature: &[Param],
        args: &[Expr],
        kwargs: &FxHashMap<SmolStr, (Span, Expr)>,
    ) {
        self.borrow_scopes.push(BorrowScope::default());

        let positional = args.len().min(signature.len());
        for (param, expr) in signature.iter().take(positional).zip(args) {
            self.check_param(param, expr);
        }
        for param in signature.iter().skip(positional) {
            if let Some((_, expr)) = kwargs.get(&param.name) {
                self.check_param(param, expr);
            }
        }

        self.borrow_scopes.pop();
    }

    fn check_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Repeat { times, body } => {
                self.walk_expr(times);
                self.check_stmts(body);
            }
            Stmt::Forever { body, .. } => self.check_stmts(body),
            Stmt::Until { cond, body } => {
                self.walk_expr(cond);
                self.check_stmts(body);
            }
            Stmt::Branch {
                cond,
                if_body,
                else_body,
            } => {
                self.walk_expr(cond);
                let before = self.moved.clone();
                self.check_stmts(if_body);
                let moved_if = self.moved.clone();
                self.moved = before;
                self.check_stmts(else_body);
                self.moved.extend(moved_if);
            }
            Stmt::SetVar { name, value, .. } => {
                self.walk_expr(value);
                let expected = self.var_type(name.basename());
                self.mark_move(value, &expected, Some(name));
                if name.fieldname().is_none() {
                    self.moved.remove(&Key::var(name.basename().clone()));
                }
            }
            Stmt::ChangeVar { name, value } => {
                self.walk_expr(value);
                self.check_read(&Key::var(name.basename().clone()), &name.span());
            }
            Stmt::Show(name) | Stmt::Hide(name) => {
                self.check_read(&Key::var(name.basename().clone()), &name.span());
            }
            Stmt::AddToList { name, value } => {
                self.walk_expr(value);
                let expected = self.list_type(name.basename());
                self.mark_move(value, &expected, None);
            }
            Stmt::DeleteList(_) => {}
            Stmt::DeleteListIndex { index, .. } => self.walk_expr(index),
            Stmt::InsertAtList {
                name, index, value,
            }
            | Stmt::SetListIndex {
                name, index, value,
            } => {
                self.walk_expr(index);
                self.walk_expr(value);
                let expected = self.list_type(name.basename());
                self.mark_move(value, &expected, None);
            }
            Stmt::Block { args, kwargs, .. } => {
                for arg in args {
                    self.walk_expr(arg);
                }
                for (_, arg) in kwargs.values() {
                    self.walk_expr(arg);
                }
            }
            Stmt::ProcCall {
                name, args, kwargs, ..
            } => {
                let signature = self
                    .sprite
                    .proc_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.check_call(&signature, args, kwargs);
            }
            Stmt::FuncCall {
                name, args, kwargs, ..
            } => {
                let signature = self
                    .sprite
                    .func_args
                    .get(name)
                    .map(|args| signature(args))
                    .unwrap_or_default();
                self.check_call(&signature, args, kwargs);
            }
            Stmt::Return { value, .. } => {
                self.walk_expr(value);
                if let Some(expected) = self.return_type.clone() {
                    self.mark_move(value, &expected, None);
                }
            }
        }
    }
}

fn check_sprite(sprite: &Sprite, stage: Option<&Sprite>, d: &mut SpriteDiagnostics) {
    for proc in sprite.procs.values() {
        let mut checker = Checker {
            sprite,
            stage,
            args: sprite.proc_args.get(&proc.name).map(Vec::as_slice),
            return_type: None,
            diagnostics: d,
            moved: FxHashSet::default(),
            borrow_scopes: vec![],
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
            return_type: Some(BaseTy::from_ast(&func.type_)),
            diagnostics: d,
            moved: FxHashSet::default(),
            borrow_scopes: vec![],
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
            return_type: None,
            diagnostics: d,
            moved: FxHashSet::default(),
            borrow_scopes: vec![],
        };
        checker.check_stmts(&event.body);
    }
}

pub fn check_project(
    project: &Project,
    stage_diagnostics: &mut SpriteDiagnostics,
    sprites_diagnostics: &mut FxHashMap<SmolStr, SpriteDiagnostics>,
) {
    check_sprite(&project.stage, None, stage_diagnostics);
    for (name, sprite) in &project.sprites {
        if let Some(d) = sprites_diagnostics.get_mut(name) {
            check_sprite(sprite, Some(&project.stage), d);
        }
    }
}

fn backend_type(type_name: &BaseTy, span: Span) -> Type {
    match type_name {
        BaseTy::Dynamic
        | BaseTy::Number
        | BaseTy::Int
        | BaseTy::String
        | BaseTy::Bool => Type::Value,
        BaseTy::Struct(name) => Type::Struct {
            name: name.clone(),
            span,
        },
    }
}

fn erase_arg(arg: &mut Arg) {
    let Some(spec) = decode_ref_arg(&arg.name) else {
        return;
    };
    arg.name = spec.logical_name;
    arg.type_ = backend_type(&spec.inner, arg.span.clone());
}

fn erase_expr(expr: &mut Expr) {
    match expr {
        Expr::Value { .. } => {}
        Expr::Name(Name::Name { name, .. }) | Expr::Arg(Name::Name { name, .. }) => {
            if let Some((_, logical_name)) = decode_borrow_name(name) {
                *name = logical_name;
            }
        }
        Expr::Name(_) | Expr::Arg(_) => {}
        Expr::Dot { lhs, .. } => erase_expr(lhs),
        Expr::Property { object, .. } => erase_expr(object),
        Expr::Repr { args, .. } => {
            for arg in args {
                erase_expr(arg);
            }
        }
        Expr::FuncCall { args, kwargs, .. } => {
            for arg in args {
                erase_expr(arg);
            }
            for (_, arg) in kwargs.values_mut() {
                erase_expr(arg);
            }
        }
        Expr::UnOp { opr, .. } => erase_expr(opr),
        Expr::BinOp { lhs, rhs, .. } => {
            erase_expr(lhs);
            erase_expr(rhs);
        }
        Expr::StructLiteral { fields, .. } => {
            for field in fields {
                erase_expr(&mut field.value);
            }
        }
    }
}

fn erase_stmts(stmts: &mut [Stmt]) {
    for stmt in stmts {
        match stmt {
            Stmt::Repeat { times, body } => {
                erase_expr(times);
                erase_stmts(body);
            }
            Stmt::Forever { body, .. } => erase_stmts(body),
            Stmt::Until { cond, body } => {
                erase_expr(cond);
                erase_stmts(body);
            }
            Stmt::Branch {
                cond,
                if_body,
                else_body,
            } => {
                erase_expr(cond);
                erase_stmts(if_body);
                erase_stmts(else_body);
            }
            Stmt::SetVar { value, .. } | Stmt::ChangeVar { value, .. } => erase_expr(value),
            Stmt::Show(_) | Stmt::Hide(_) | Stmt::DeleteList(_) => {}
            Stmt::AddToList { value, .. } => erase_expr(value),
            Stmt::DeleteListIndex { index, .. } => erase_expr(index),
            Stmt::InsertAtList { index, value, .. }
            | Stmt::SetListIndex { index, value, .. } => {
                erase_expr(index);
                erase_expr(value);
            }
            Stmt::Block { args, kwargs, .. }
            | Stmt::ProcCall { args, kwargs, .. }
            | Stmt::FuncCall { args, kwargs, .. } => {
                for arg in args {
                    erase_expr(arg);
                }
                for (_, arg) in kwargs.values_mut() {
                    erase_expr(arg);
                }
            }
            Stmt::Return { value, .. } => erase_expr(value),
        }
    }
}

fn erase_sprite(sprite: &mut Sprite) {
    for args in sprite.proc_args.values_mut() {
        for arg in args {
            erase_arg(arg);
        }
    }
    for args in sprite.func_args.values_mut() {
        for arg in args {
            erase_arg(arg);
        }
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

/// Remove UY's temporary reference encoding after static checking. The Scratch
/// backend receives the original argument names and the underlying value/struct
/// types, so runtime code generation remains unchanged.
pub fn erase_project(project: &mut Project) {
    erase_sprite(&mut project.stage);
    for sprite in project.sprites.values_mut() {
        erase_sprite(sprite);
    }
}
