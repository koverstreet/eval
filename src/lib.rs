#![feature(dynamic_lib)]

use std::fs::File;
use std::io::Write;
use std::process::Command;

#[allow(deprecated)]
use std::dynamic_lib::DynamicLibrary;
use std::mem::transmute;

extern crate tempdir;
use tempdir::TempDir;

pub fn eval_typename<T>(src: &str, ret_type: &str) -> Option<T> {
    let tempdir = TempDir::new("rust-eval").expect("tmpdir failure");
    let srcpath = tempdir.path().join("eval.rs");

    let mut srcfile = File::create(&srcpath).expect("error creating src file");
    srcfile.write(b"#[no_mangle]\n").ok().unwrap();
    srcfile.write(b"pub fn eval_fn() -> ").ok().unwrap();
    srcfile.write(ret_type.as_bytes()).ok().unwrap();
    srcfile.write(b" { ").ok().unwrap();
    srcfile.write(src.as_bytes()).ok().unwrap();
    srcfile.write(b" } ").ok().unwrap();

    if !Command::new("rustc")
        .arg("-C").arg("prefer-dynamic")
        .arg("--crate-type").arg("dylib")
        .arg("--out-dir").arg(tempdir.path())
        .arg(srcpath)
        .status()
        .expect("error execcing rustc")
        .success() {
        return None;
    }

    let libpath = tempdir.path().join("libeval.so");
    let lib = DynamicLibrary::open(Some(libpath.as_path())).expect("error opening eval lib");
    unsafe {
        let eval_fn : fn() -> T =
            transmute(lib.symbol::<isize>("eval_fn").expect("error looking up symbol"));

        Some(eval_fn())
    }
}

macro_rules! eval {
    ($T:ty, $src:expr) => { eval_typename::<$T>($src, stringify!($T)) }
}

#[test]
fn test_eval() {
    assert_eq!(eval!(u32, "0u32"), Some(0));
    assert_eq!(eval!(u32, "let mut x = 0; for i in 0..10 { x += i; } x"), Some(45));
}
