/*
Copyright (c) NCC Group, 2021
Copyright (c) Google, 2025
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#[macro_use]
extern crate gumshoe;
//extern crate base64;
//extern crate cc;

/*
use base64::decode;

use std::fs::File;
use std::io::Write;

extern crate cc;

pub fn link() {
  let b64 = gumshoe::codeb64();
  let code = &decode(b64).unwrap();

  std::fs::create_dir_all("gen").unwrap();
  let mut f = File::create("gen/frida-gum-wrapper.c").expect("Unable to create file");
  f.write_all(code.as_slice()).expect("Unable to write data");

  println!("cargo:rustc-flags=-L frida/{}", std::env::var("TARGET").unwrap());
  println!("cargo:rustc-flags=-l frida-gum");
  println!("cargo:rustc-flags=-l dl");
  println!("cargo:rustc-flags=-l resolv");
  println!("cargo:rustc-flags=-l rt");
  println!("cargo:rustc-flags=-l m");
  println!("cargo:rustc-flags=-l pthread");

  cc::Build::new()
    .include("frida")
    .file("gen/frida-gum-wrapper.c")
    .compile("frida-gum-wrapper");
}
*/

link!{}

/*
use std::env;

/// Adds a temporary workaround for an issue with the Rust compiler and Android
/// in x86_64/aarch64 devices: https://github.com/rust-lang/rust/issues/109717.
/// The workaround comes from: https://github.com/mozilla/application-services/pull/5442
fn setup_android_workaround() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not set");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    if (target_arch == "x86_64" || target_arch == "aarch64") && target_os == "android" {
        let android_ndk_home = "/android-ndk"; //env::var("ANDROID_NDK_HOME").expect("ANDROID_NDK_HOME not set");
        let build_os = match env::consts::OS {
            "linux" => "linux",
            "macos" => "darwin",
            "windows" => "windows",
            _ => panic!(
                "Unsupported OS. You must use either Linux, MacOS or Windows to build the crate."
            ),
        };
        const DEFAULT_CLANG_VERSION: &str = "14.0.7";
        let clang_version =
            env::var("NDK_CLANG_VERSION").unwrap_or_else(|_| DEFAULT_CLANG_VERSION.to_owned());
        //let linux_x86_64_lib_dir = format!(
        //    "toolchains/llvm/prebuilt/{build_os}-x86_64/lib64/clang/{clang_version}/lib/linux/"
        //);

        //println!("cargo:rustc-link-search={android_ndk_home}/{linux_x86_64_lib_dir}");
        println!("cargo:rustc-link-search=/android-ndk/lib64/clang/14.0.6/lib/linux");
        println!("cargo:rustc-link-lib=static=clang_rt.builtins-{target_arch}-android");
        println!("cargo:rustc-link-lib=static=/android-ndk/lib64/clang/14.0.6/lib/linux/libclang_rt.builtins-aarch64-android.a");
    }
}
*/

fn main() {
  //println!("cargo:rustc-flags=-L /android-ndk/lib64/clang/14.0.6/lib/linux");
  //println!("cargo:rustc-flags=-l clang_rt.builtins-aarch64-android");

  //setup_android_workaround();
  link(true);
}


