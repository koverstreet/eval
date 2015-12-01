#![feature(dynamic_lib)]

use std::fs::File;
use std::io;
use std::io::Write;
use std::process::Command;

#[allow(deprecated)]
use std::dynamic_lib::DynamicLibrary;
use std::mem::transmute;

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
    CompileError,
    InternalError,
}

pub fn eval_typename<T>(src: &str, ret_type: &str) -> Result<T, EvalError> {
    let tempdir = try!(TempDir::new("rust-eval"));
    let srcpath = tempdir.path().join("eval.rs");

    let mut srcfile = try!(File::create(&srcpath));

    try!(srcfile.write(b"#[no_mangle]\n"));
    try!(srcfile.write(b"pub fn eval_fn() -> "));
    try!(srcfile.write(ret_type.as_bytes()));
    try!(srcfile.write(b" { "));
    try!(srcfile.write(src.as_bytes()));
    try!(srcfile.write(b" } "));
    drop(srcfile);

    if !try!(Command::new("rustc")
                .arg("-C").arg("prefer-dynamic")
                .arg("--crate-type").arg("dylib")
                .arg("--out-dir").arg(tempdir.path())
                .arg(srcpath)
                .status()).success() {
        return Err(EvalError::CompileError);
    }

    let libpath = tempdir.path().join("libeval.so");
    let lib = try!(DynamicLibrary::open(Some(libpath.as_path())));
    unsafe {
        let eval_sym = try!(lib.symbol::<isize>("eval_fn"));
        let eval_fn : fn() -> T = transmute(eval_sym);

        Ok(eval_fn())
    }
}

macro_rules! eval {
    ($T:ty, $src:expr) => { eval_typename::<$T>($src, stringify!($T)) }
}

#[test]
fn test_eval() {
    assert_eq!(eval!(u32, "0u32"), Ok(0));
    assert_eq!(eval!(u32, "let mut x = 0; for i in 0..10 { x += i; } x"), Ok(45));
}
