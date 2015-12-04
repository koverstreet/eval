#![crate_type="dylib"]
#![feature(dynamic_lib, unboxed_closures, plugin_registrar, rustc_private, slice_patterns, convert)]

use std::fs::File;
use std::io;
use std::io::Write;
use std::process::Command;

#[allow(deprecated)]
use std::dynamic_lib::DynamicLibrary;

extern crate tempdir;
use tempdir::TempDir;

impl From<io::Error> for EvalError {
    fn from(_: io::Error) -> EvalError {
        EvalError::InternalError
    }
}

impl From<String> for EvalError {
    fn from(_: String) -> EvalError {
        EvalError::InternalError
    }
}

impl<'a> AsRef<str> for EvalError {
    fn as_ref(&self) -> &str {
        match self {
            &EvalError::CompileError(ref s) => s.as_str(),
            &EvalError::InternalError   => "internal error",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EvalError {
    CompileError(String),
    InternalError,
}

#[allow(deprecated)]
pub unsafe fn __compile_fn(environment: &str, fn_type: &str, src: &str)
            -> Result<(DynamicLibrary, *mut usize), EvalError> {
    let tempdir = try!(TempDir::new("rust-eval"));
    let srcpath = tempdir.path().join("eval.rs");

    let mut srcfile = try!(File::create(&srcpath));

    try!(srcfile.write(environment.as_bytes()));
    try!(srcfile.write(b"\n#[no_mangle]\n"));
    try!(srcfile.write(b"pub fn func "));
    try!(srcfile.write(fn_type.as_bytes()));
    try!(srcfile.write(b" { "));
    try!(srcfile.write(src.as_bytes()));
    try!(srcfile.write(b" } "));
    drop(srcfile);

    let output = try!(Command::new("rustc")
        .arg("-C").arg("prefer-dynamic")
        .arg("--crate-type").arg("dylib")
        .arg("--out-dir").arg(tempdir.path())
        .arg(srcpath)
        .output());

    if !output.status.success() {
        return Err(EvalError::CompileError(String::from_utf8_lossy(&output.stderr[..]).into_owned()));
    }

    let libpath = tempdir.path().join("libeval.so");
    let lib = try!(DynamicLibrary::open(Some(libpath.as_path())));
    let sym = try!(lib.symbol::<usize>("func"));

    Ok((lib, sym))
}

macro_rules! compile_fn_str {
    ($environment:expr, ($($A:ident : $T:ty),*) -> $R:ty, $src:expr) => { unsafe {
        use std::mem::transmute;
        struct CompiledFn {
            /* need the DynamicLibrary handle to live as long as the fn pointer.. */
            #[allow(dead_code, deprecated)]
            lib: DynamicLibrary,
            func: fn($($A: $T),*) -> $R,
        }

        /*
        impl Fn<($($T,)*)> for CompiledFn {
            #[inline]
            extern "rust-call" fn call(&self, ($($A,)*): ($($T,)*)) -> $R {
                (self.func)($($A),*)
            }
        }

        impl FnMut<($($T,)*)> for CompiledFn {
            #[inline]
            extern "rust-call" fn call_mut(&mut self, ($($A,)*): ($($T,)*)) -> $R {
                (self.func)($($A),*)
            }
        }

        impl FnOnce<($($T,)*)> for CompiledFn {
            type Output = $R;

            #[inline]
            extern "rust-call" fn call_once(self, ($($A,)*): ($($T,)*)) -> $R {
                (self.func)($($A),*)
            }
        }
        */

        __compile_fn($environment, stringify!(($($A: $T),*) -> $R), $src)
            .map(|x| { let (l, f) = x; CompiledFn { lib: l, func: transmute(f)} } )
    } };

    ($environment:expr, ($($A:ident : $T:ty),*), $src:expr)  => {
        compile_fn_str!(($($A: $T),*) -> (), $src)
    };
}

macro_rules! compile_fn {
    (($($A:ident : $T:ty),*) -> $R:ty, $src:expr)  => {
        compile_fn_str!("", ($($A: $T),*) -> $R, stringify!($src))
    };

    (($($A:ident : $T:ty),*), $src:expr)  => {
        compile_fn!(($($A: $T),*) -> (), $src)
    };
}

macro_rules! eval_str {
    ($R:ty, $src:expr) => {
        compile_fn_str!("", () -> $R, $src).map(|f| f())
    };
}

macro_rules! eval {
    ($T:ty, $src:expr) => { eval_str!($T, stringify!($src)) }
}

#[test]
fn test_eval() {
    /*
    assert_eq!(eval_str!(u32, "0u32"), Ok(0));
    assert_eq!(eval_str!(u32, "let mut x = 0; for i in 0..10 { x += i; } x"), Ok(45));

    assert_eq!(eval!(u32, 0u32), Ok(0));
    assert_eq!(eval!(u32, { let mut x = 0; for i in 0..10 { x += i; } x } ), Ok(45));

    let no_ret_no_args = compile_fn!((), {
                               let mut x = 0; for i in 0..10 { x += i; }
                           }).expect("compile error");
    no_ret_no_args();

    let no_ret_one_arg = compile_fn!((a: u32), {
                               let mut x = a; for i in 0..10 { x += i; }
                           }).expect("compile error");
    no_ret_one_arg(5);

    let no_args = compile_fn!(() -> u32, {
                               let mut x = 0; for i in 0..10 { x += i; } x
                           }).expect("compile error");
    assert_eq!(no_args(), 45);

    let one_arg = compile_fn!((a: u32) -> u32, {
                               let mut x = a; for i in 0..10 { x += i; } x
                           }).expect("compile error");
    assert_eq!(one_arg(5), 50);

    let two_args = compile_fn!((a: u32, b: u32) -> u32, {
                               let mut x = a; for i in 0..10 { x += i * b; } x
                           }).expect("compile error");
    assert_eq!(two_args(5, 1), 50);
    */
}

extern crate syntax;
extern crate rustc;

use syntax::codemap::Span;
use syntax::parse::token;
use syntax::ast::TokenTree;
use syntax::ast::TokenTree::Token;
use syntax::ext::base::{ExtCtxt, MacResult, DummyResult, NormalTT, TTMacroExpander};
use syntax::print::pprust;
use rustc::plugin::Registry;

struct DefmacroFunc {
    #[allow(dead_code, deprecated)]
    lib: DynamicLibrary,
    func: fn(cx: &mut syntax::ext::base::ExtCtxt,
             sp: syntax::codemap::Span,
             tt: &[syntax::ast::TokenTree])
        -> Box<syntax::ext::base::MacResult + 'static>,
}

impl TTMacroExpander for DefmacroFunc {
    fn expand<'cx>(&self,
                   cx: &'cx mut ExtCtxt,
                   sp: Span,
                   tt: &[TokenTree])
        -> Box<MacResult+'cx> {
        (self.func)(cx, sp, tt)
    }
}

fn expand_defmacro(cx: &mut ExtCtxt, sp: Span, args: &[TokenTree])
        -> Box<MacResult + 'static> {
    let (name, body) = match args {
        [Token(_, token::Ident(name, _)),
         Token(_, token::Comma),
        ref body] => (name, body),
        _ => {
            println!("got {} tokens", args.len());
            cx.span_err(sp, "bad arguments");
            return DummyResult::any(sp);
        }
    };

    let environment = "
    #![feature(rustc_private)]
    #![feature(slice_patterns)]
    extern crate syntax;
    extern crate rustc;
    ";

    let new_macro = match compile_fn_str!(environment,
                                  (cx: &mut syntax::ext::base::ExtCtxt,
                                   sp: syntax::codemap::Span,
                                   args: &[syntax::ast::TokenTree])
                                        -> Box<syntax::ext::base::MacResult + 'static>,
                                  &pprust::tt_to_string(&body)) {
        Ok(m)   => m,
        Err(e)  => {
            cx.span_err(sp, e.as_ref());
            return DummyResult::any(sp);
        }
    };

    let new_macro_t = unsafe { DefmacroFunc {
        lib: new_macro.lib,
        func: std::mem::transmute(new_macro.func),
    }};

    //cx.exported_macros.push(body);
    cx.syntax_env.insert(name.name, NormalTT(Box::new(new_macro_t), None, false));

    DummyResult::any(sp)
}

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_macro("defmacro", expand_defmacro);
}
