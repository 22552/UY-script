use std::collections::VecDeque;

use logos::{
    Logos,
    Span,
    SpannedIter,
};

use super::token::Token;
use crate::diagnostic::{
    Diagnostic,
    DiagnosticKind,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DeclMode {
    Normal,
    AwaitProcName,
    ProcArgs,
    AwaitFuncName,
    AwaitFuncLParen,
    FuncArgs,
}

type RawToken = (Result<Token, ()>, Span);

pub struct Lexer<'source> {
    token_stream: SpannedIter<'source, Token>,
    buffered: VecDeque<RawToken>,
    previous_can_end_expr: bool,
    previous_is_call_head: bool,
    at_stmt_start: bool,
    decl_mode: DeclMode,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            token_stream: Token::lexer(source).spanned(),
            buffered: VecDeque::new(),
            previous_can_end_expr: false,
            previous_is_call_head: false,
            at_stmt_start: true,
            decl_mode: DeclMode::Normal,
        }
    }

    fn raw_next(&mut self) -> Option<RawToken> {
        self.buffered
            .pop_front()
            .or_else(|| self.token_stream.next())
    }

    fn push_front(&mut self, token: RawToken) {
        self.buffered.push_front(token);
    }

    fn in_arg_declaration(&self) -> bool {
        matches!(self.decl_mode, DeclMode::ProcArgs | DeclMode::FuncArgs)
    }

    fn try_reference_type(&mut self, amp_span: &Span) -> Option<(Token, Span)> {
        if !self.in_arg_declaration() {
            return None;
        }

        let first = self.raw_next()?;
        match first.0.clone() {
            Ok(Token::Name(name)) if name == "mut" => {
                let Some(second) = self.raw_next() else {
                    self.push_front(first);
                    return None;
                };
                let Some(third) = self.raw_next() else {
                    self.push_front(second);
                    self.push_front(first);
                    return None;
                };
                match (second.0.clone(), third.0.clone()) {
                    (Ok(Token::Name(type_name)), Ok(Token::Name(arg_name))) => {
                        let encoded = format!("@uyref:1:{type_name}:{arg_name}").into();
                        self.push_front((Ok(Token::Name(encoded)), third.1.clone()));
                        Some((Token::Name("Any".into()), amp_span.start..second.1.end))
                    }
                    _ => {
                        self.push_front(third);
                        self.push_front(second);
                        self.push_front(first);
                        None
                    }
                }
            }
            Ok(Token::Name(type_name)) => {
                let Some(second) = self.raw_next() else {
                    self.push_front(first);
                    return None;
                };
                match second.0.clone() {
                    Ok(Token::Name(arg_name)) => {
                        let encoded = format!("@uyref:0:{type_name}:{arg_name}").into();
                        self.push_front((Ok(Token::Name(encoded)), second.1.clone()));
                        Some((Token::Name("Any".into()), amp_span.start..first.1.end))
                    }
                    _ => {
                        self.push_front(second);
                        self.push_front(first);
                        None
                    }
                }
            }
            _ => {
                self.push_front(first);
                None
            }
        }
    }

    fn try_borrow_expr(&mut self, amp_span: &Span) -> Option<(Token, Span)> {
        if self.previous_can_end_expr && !self.previous_is_call_head {
            return None;
        }

        let first = self.raw_next()?;
        match first.0.clone() {
            Ok(Token::Name(name)) if name == "mut" => {
                let Some(second) = self.raw_next() else {
                    self.push_front(first);
                    return None;
                };
                match second.0.clone() {
                    Ok(Token::Name(name)) => Some((
                        Token::Name(format!("@uyborrow:1:{name}").into()),
                        amp_span.start..second.1.end,
                    )),
                    Ok(Token::Arg(name)) => Some((
                        Token::Arg(format!("@uyborrow:1:{name}").into()),
                        amp_span.start..second.1.end,
                    )),
                    _ => {
                        self.push_front(second);
                        self.push_front(first);
                        None
                    }
                }
            }
            Ok(Token::Name(name)) => Some((
                Token::Name(format!("@uyborrow:0:{name}").into()),
                amp_span.start..first.1.end,
            )),
            Ok(Token::Arg(name)) => Some((
                Token::Arg(format!("@uyborrow:0:{name}").into()),
                amp_span.start..first.1.end,
            )),
            _ => {
                self.push_front(first);
                None
            }
        }
    }

    fn token_can_end_expr(token: &Token) -> bool {
        matches!(
            token,
            Token::Name(_)
                | Token::Arg(_)
                | Token::Bin(_)
                | Token::Oct(_)
                | Token::Int(_)
                | Token::Hex(_)
                | Token::Float(_)
                | Token::Str(_)
                | Token::True
                | Token::False
                | Token::RParen
                | Token::RBracket
                | Token::RBrace
        )
    }

    fn update_decl_mode(&mut self, token: &Token) {
        self.decl_mode = match (self.decl_mode, token) {
            (_, Token::Proc) => DeclMode::AwaitProcName,
            (_, Token::Func) => DeclMode::AwaitFuncName,
            (DeclMode::AwaitProcName, Token::Name(_)) => DeclMode::ProcArgs,
            (DeclMode::AwaitFuncName, Token::Name(_)) => DeclMode::AwaitFuncLParen,
            (DeclMode::AwaitFuncLParen, Token::LParen) => DeclMode::FuncArgs,
            (DeclMode::FuncArgs, Token::RParen) => DeclMode::Normal,
            (DeclMode::ProcArgs, Token::LBrace) => DeclMode::Normal,
            (mode, _) => mode,
        };
    }

    fn finish(&mut self, token: Token, span: Span) -> (usize, Token, usize) {
        let was_stmt_start = self.at_stmt_start;
        self.previous_is_call_head = was_stmt_start
            && matches!(self.decl_mode, DeclMode::Normal)
            && matches!(token, Token::Name(_));

        self.update_decl_mode(&token);
        self.previous_can_end_expr = Self::token_can_end_expr(&token);
        self.at_stmt_start = match token {
            Token::Semicolon | Token::LBrace | Token::RBrace => true,
            Token::Newline => self.at_stmt_start,
            _ => false,
        };
        (span.start, token, span.end)
    }
}

impl<'source> From<&'source str> for Lexer<'source> {
    fn from(source: &'source str) -> Self {
        Lexer::new(source)
    }
}

impl Iterator for Lexer<'_> {
    type Item = Result<(usize, Token, usize), Diagnostic>;

    fn next(&mut self) -> Option<Self::Item> {
        let (token, span) = self.raw_next()?;
        let token = match token {
            Ok(token) => token,
            Err(_) => {
                self.previous_can_end_expr = false;
                self.previous_is_call_head = false;
                return Some(Err(Diagnostic {
                    kind: DiagnosticKind::InvalidToken,
                    span,
                }));
            }
        };

        if matches!(token, Token::Amp) {
            if let Some((token, span)) = self.try_reference_type(&span) {
                return Some(Ok(self.finish(token, span)));
            }
            if let Some((token, span)) = self.try_borrow_expr(&span) {
                return Some(Ok(self.finish(token, span)));
            }
        }

        Some(Ok(self.finish(token, span)))
    }
}
