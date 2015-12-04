#![feature(plugin)]
#![plugin(defmacro)]

defmacro!(rn, {
    use syntax::codemap::Span;
    use syntax::parse::token;
    use syntax::ast::TokenTree;
    use syntax::ast::TokenTree::Token;
    use syntax::ext::base::{ExtCtxt, MacResult, DummyResult, MacEager};
    use syntax::ext::build::AstBuilder;
    use rustc::plugin::Registry;

    static NUMERALS: &'static [(&'static str, u32)] = &[
        ("M", 1000), ("CM", 900), ("D", 500), ("CD", 400),
        ("C",  100), ("XC",  90), ("L",  50), ("XL",  40),
        ("X",   10), ("IX",   9), ("V",   5), ("IV",   4),
        ("I",    1)];

    let text = match args {
        [Token(_, token::Ident(s, _))] => s.to_string(),
        _ => {
            cx.span_err(sp, "argument should be a single identifier");
            return DummyResult::any(sp);
        }
    };

    let mut text = &*text;
    let mut total = 0;
    while !text.is_empty() {
        match NUMERALS.iter().find(|&&(rn, _)| text.starts_with(rn)) {
            Some(&(rn, val)) => {
                total += val;
                text = &text[rn.len()..];
            }
            None => {
                cx.span_err(sp, "invalid Roman numeral");
                return DummyResult::any(sp);
            }
        }
    }

    MacEager::expr(cx.expr_u32(sp, total))
});

#[test]
fn test_expand_rn() {
    assert_eq!(rn!(MMXV), 2015);
}
