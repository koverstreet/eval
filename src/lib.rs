#![feature(dynamic_lib)]

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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EvalError {
    CompileError(String),
    InternalError,
}

#[allow(deprecated)]
pub unsafe fn __compile_fn(fn_type: &str, src: &str)
            -> Result<(DynamicLibrary, *mut usize), EvalError> {
    let tempdir = try!(TempDir::new("rust-eval"));
    let srcpath = tempdir.path().join("eval.rs");

    let mut srcfile = try!(File::create(&srcpath));

    try!(srcfile.write(b"#[no_mangle]\n"));
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
    (($($A:ident : $T:ty),*) -> $R:ty, $src:expr) => { unsafe {
        struct CompiledFn {
            /* need the DynamicLibrary handle to live as long as the fn pointer.. */
            #[allow(dead_code, deprecated)]
            lib: DynamicLibrary,
            func: fn($($A: $T),*) -> $R,
        }

        use std::mem::transmute;
        __compile_fn(stringify!(($($A: $T),*) -> $R), $src)
            .map(|x| { let (l, f) = x; CompiledFn { lib: l, func: transmute(f)} } )
    } };

    (($($A:ident : $T:ty),*), $src:expr)  => {
        compile_fn_str!(($($A: $T),*) -> (), $src)
    };
}

macro_rules! compile_fn {
    (($($A:ident : $T:ty),*) -> $R:ty, $src:expr)  => {
        compile_fn_str!(($($A: $T),*) -> $R, stringify!($src))
    };

    (($($A:ident : $T:ty),*), $src:expr)  => {
        compile_fn!(($($A: $T),*) -> (), $src)
    };
}

macro_rules! eval_str {
    ($R:ty, $src:expr) => {
        compile_fn_str!(() -> $R, $src).map(|f| (f.func)())
    };
}

macro_rules! eval {
    ($T:ty, $src:expr) => { eval_str!($T, stringify!($src)) }
}

#[test]
fn test_eval() {
    assert_eq!(eval_str!(u32, "0u32"), Ok(0));
    assert_eq!(eval_str!(u32, "let mut x = 0; for i in 0..10 { x += i; } x"), Ok(45));

    assert_eq!(eval!(u32, 0u32), Ok(0));
    assert_eq!(eval!(u32, { let mut x = 0; for i in 0..10 { x += i; } x } ), Ok(45));

    let no_ret_no_args = compile_fn!((), {
                               let mut x = 0; for i in 0..10 { x += i; }
                           }).expect("compile error");
    (no_ret_no_args.func)();

    let no_ret_one_arg = compile_fn!((a: u32), {
                               let mut x = a; for i in 0..10 { x += i; }
                           }).expect("compile error");
    (no_ret_one_arg.func)(5);

    let no_args = compile_fn!(() -> u32, {
                               let mut x = 0; for i in 0..10 { x += i; } x
                           }).expect("compile error");
    assert_eq!((no_args.func)(), 45);

    let one_arg = compile_fn!((a: u32) -> u32, {
                               let mut x = a; for i in 0..10 { x += i; } x
                           }).expect("compile error");
    assert_eq!((one_arg.func)(5), 50);

    let two_args = compile_fn!((a: u32, b: u32) -> u32, {
                               let mut x = a; for i in 0..10 { x += i * b; } x
                           }).expect("compile error");
    assert_eq!((two_args.func)(5, 1), 50);
}
