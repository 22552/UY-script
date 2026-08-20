use libgoboscript::lexer::{
    adaptor::Lexer,
    token::Token,
};

fn tokens(source: &str) -> Vec<Token> {
    Lexer::new(source)
        .map(|result| match result {
            Ok((_, token, _)) => token,
            Err(_) => panic!("lexer error"),
        })
        .collect()
}

#[test]
fn encodes_proc_reference_parameter() {
    let tokens = tokens("proc read &String s {}");
    assert_eq!(
        tokens,
        vec![
            Token::Proc,
            Token::Name("read".into()),
            Token::Name("Any".into()),
            Token::Name("@uyref:0:String:s".into()),
            Token::LBrace,
            Token::RBrace,
        ]
    );
}

#[test]
fn encodes_mutable_function_reference_parameter() {
    let tokens = tokens("func edit(&mut String s) Number { return 0; }");
    assert_eq!(
        tokens,
        vec![
            Token::Func,
            Token::Name("edit".into()),
            Token::LParen,
            Token::Name("Any".into()),
            Token::Name("@uyref:1:String:s".into()),
            Token::RParen,
            Token::Name("Number".into()),
            Token::LBrace,
            Token::Return,
            Token::Int(0),
            Token::Semicolon,
            Token::RBrace,
        ]
    );
}

#[test]
fn encodes_borrow_as_first_proc_argument() {
    let tokens = tokens("onflag { read &s; }");
    assert_eq!(
        tokens,
        vec![
            Token::OnFlag,
            Token::LBrace,
            Token::Name("read".into()),
            Token::Name("@uyborrow:0:s".into()),
            Token::Semicolon,
            Token::RBrace,
        ]
    );
}

#[test]
fn keeps_binary_amp_as_join_operator() {
    let tokens = tokens("onflag { out x & y; }");
    assert_eq!(
        tokens,
        vec![
            Token::OnFlag,
            Token::LBrace,
            Token::Name("out".into()),
            Token::Name("x".into()),
            Token::Amp,
            Token::Name("y".into()),
            Token::Semicolon,
            Token::RBrace,
        ]
    );
}

#[test]
fn encodes_mutable_borrow_expression() {
    let tokens = tokens("onflag { edit &mut s; }");
    assert_eq!(
        tokens,
        vec![
            Token::OnFlag,
            Token::LBrace,
            Token::Name("edit".into()),
            Token::Name("@uyborrow:1:s".into()),
            Token::Semicolon,
            Token::RBrace,
        ]
    );
}
