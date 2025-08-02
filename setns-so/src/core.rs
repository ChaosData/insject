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

//#[macro_use]
extern crate ctor;

extern crate gumshoe;
use gumshoe::*;

extern crate libc;
use libc::*;

extern crate setns_common;
use setns_common::*;

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

//static mut ARGS: Option<String> = None;
static mut OPTS: Option<Opts> = None;

static mut HOOK_SINGLETON: SetnsHook = SetnsHook{ raw_listener: std::ptr::null_mut() };

pub fn setup_frida_gum() {
  gumshoe::gum_init_embedded();
}

#[derive(Clone,Debug)]
enum LoadType {
  Executable, //ie ./insject
  LdPreload,
  DlOpen
}

#[derive(Clone,Debug)]
struct SetnsState {
  on_load_hit: bool,
  libc_start_main_hit: bool,
  target: Option<usize>,
  target_name: Option<String>,
  target_is_main: Option<bool>,
  target_hit: bool,
  load_type: Option<LoadType>
}
static mut STATE: SetnsState = SetnsState {
  on_load_hit: false,
  libc_start_main_hit: false,
  target: None,
  target_name: None,
  target_is_main: None,
  target_hit: false,
  load_type: None
};

pub fn has_sym(name: &String) -> Option<GumAddress> {
  let path = std::fs::read_link("/proc/self/exe").unwrap();
  let path = path.to_str().unwrap();

  let mut addr = gumshoe::gum_module_find_symbol_by_name(path, name.as_str());
  if addr == 0 {
    addr = gumshoe::gum_module_find_export_by_name("", name.as_str());
  }
  if addr == 0 {
    None
  } else {
    Some(addr)
  }
}

pub fn try_hook_by_sym_name(name: &String) -> bool {
  let path = std::fs::read_link("/proc/self/exe").unwrap();
  let path = path.to_str().unwrap();

  let mut addr = gumshoe::gum_module_find_symbol_by_name(path, name.as_str());
  if addr == 0 {
    addr = gumshoe::gum_module_find_export_by_name("", name.as_str());
  }
  if addr == 0 {
    return false;
  }

  unsafe {
    match &mut *std::ptr::addr_of_mut!(OPTS) {
      Some(opts) => {
        opts.raw_address = Some(addr);
        STATE.target = Some(addr as usize);
        STATE.target_name = Some(name.clone());
      },
      None => { unreachable!(); }
    }
  }
  do_hook(addr);
  return true;
}

pub fn get_and_sanitize_env(vars: &[&str]) -> std::collections::BTreeMap<String,String> {
  let mut ret = std::collections::BTreeMap::new();
  let input: std::collections::BTreeSet<String> = vars.into_iter().map(|s| s.to_string()).collect();

  unsafe extern "C" {
    static mut environ: *mut *const c_char;
  }
  let mut orig_env = unsafe { *(std::ptr::addr_of_mut!(environ)) };
  let mut env = orig_env;

  let mut c = 0;
  let mut var_vals: std::vec::Vec<*const c_char> = vec!();

  if env.is_null() {
    return ret;
  }
  unsafe {
    while !(*env).is_null() {
      var_vals.push(*env);
      c += 1;
      match std::ffi::CStr::from_ptr(*env).to_str() {
        Ok(str) => {
          let v: std::vec::Vec<&str> = str.splitn(2, '=').collect();
          if v.len() == 2 {
            if input.contains(v[0]) {
              ret.insert(v[0].to_string(), v[1].to_string());
              let _ = var_vals.pop();
            }
          }
        },
        Err(_) => {}
      }
      env = env.add(1);
    }

    for i in 0..c {
      *orig_env = match var_vals.get(i) {
        Some(val) => val.clone(),
        None => 0 as *const c_char
      };
      orig_env = orig_env.add(1);
    }
  }

  ret
}

pub fn real_get_and_unset_env(vars: &[&str]) -> std::collections::BTreeMap<String,String> {
  let mut ret = std::collections::BTreeMap::new();

  let real_unsetenv = unsafe {
    //RTLD_NEXT
    dlsym(usize::MAX as *mut c_void, b"unsetenv\0".as_ptr() as *const c_char)
  };
  if real_unsetenv as usize == 0 {
    println!("could not find real unsetenv?");
    return ret;
  }

  let real_unsetenv = |n: &str| {
    let n = std::ffi::CString::new(n).unwrap();
    unsafe {
      core::intrinsics::transmute::<*mut c_void,
        extern "C" fn(var: *const c_char) -> c_int
      >(real_unsetenv)(n.as_ptr());
    }
  };

  for v in vars {
    match std::env::var(v) {
      Ok(val) => {
        ret.insert(v.to_string(), val);
      },
      Err(_) => {}
    }
  }

  for (n,_) in &ret {
    real_unsetenv(n.as_str());
  }

  ret
}

//pub fn get_and_unset_env(vars: &[&str]) -> std::collections::HashMap<String,String> {
pub fn get_and_unset_env(vars: &[&str]) -> std::collections::BTreeMap<String,String> {
  //let mut ret = std::collections::HashMap::new();
  let mut ret = std::collections::BTreeMap::new();

  let getenv = |n: &str| {
    let n = std::ffi::CString::new(n).unwrap();
    let v = unsafe { getenv(n.as_ptr()) };
    if v.is_null() {
      return None;
    }
    let v = unsafe { std::ffi::CStr::from_ptr(v) };
    Some(v.to_str().unwrap().to_string())
  };

  let unsetenv = |n: &str| {
    let n = std::ffi::CString::new(n).unwrap();
    unsafe { unsetenv(n.as_ptr()) }
  };

  for v in vars {
    match getenv(v) {
      Some(val) => {
        ret.insert(v.to_string(), val);
      },
      None => {}
    }
  }

  for (n,_) in &ret {
    unsetenv(n.as_str());
  }

  ret
}

pub fn overrides_getenv() -> bool {
  let real = unsafe {
    //RTLD_NEXT
    dlsym(usize::MAX as *mut c_void, b"getenv\0".as_ptr() as *const c_char) as usize
  };
  if real == 0 {
    false
  } else {
    real != (getenv as *const () as usize)
  }
}

pub fn overrides_setenv() -> bool {
  let real = unsafe {
    //RTLD_NEXT
    dlsym(usize::MAX as *mut c_void, b"setenv\0".as_ptr() as *const c_char) as usize
  };
  if real == 0 {
    false
  } else {
    real != (setenv as *const () as usize)
  }
}

pub fn overrides_unsetenv() -> bool {
  let real = unsafe {
    //RTLD_NEXT
    dlsym(usize::MAX as *mut c_void, b"unsetenv\0".as_ptr() as *const c_char) as usize
  };
  if real == 0 {
    false
  } else {
    real != (unsetenv as *const () as usize)
  }
}

/*
#[used]
#[link_section = ".ctors"]
#[no_mangle]
pub static CONSTRUCTOR: extern fn() = on_load;
*/

#[allow(dead_code)]
fn stdout(msg: &str) {
  let msg_cstr = CString::new(msg).expect("CString::new failed");
  unsafe {
    let _ = libc::write(1, msg_cstr.as_ptr() as *const c_void, msg.len());
  }
}

//#[no_mangle]
//pub extern "C" fn on_load() {

#[ctor]
fn on_load() {
  //note: if we have an LD_PRELOAD that means we were not loaded by some
  //      vanilla dlopen. if the symbol name is defined and not main, then
  //      we set up the frida infra to hook the symbol. we will _try_ to
  //      do the same for main, but it might not be defined. so we have
  //      an LD_PRELOAD hook for __libc_start_main that can catch main
  //      for us. the key thing here is that our __libc_start_main needs
  //      to be able to determine that we were LD_PRELOADed and if main
  //      was already hooked by frida by symbol name.

  //note: some shells hook unsetenv, so we try to pull it out of libc.
  //      a cleaner future approach may just be to shift the entries around
  //      to remove ours.
  //
  //      the other problem is that glibc decided to make some foolish
  //      modifications and recent versions block dlopen-ing PIE executables.
  //      we can get around the initial check by patching the ELF headers with
  //      lief, but once loaded, the symbols don't actually become available to
  //      the main process, even through dlsym. as such, we can't just call
  //      do_setns_external as originally planned unless using the .so.
  //      for convenience purposes though, we are not going to do that.
  //
  //      after a lot of testing, most issues w/ get/set/unsetvar causing
  //      crashes (libc.so.6`__mbrtowc + 132) are red herrings, or at least
  //      things are so broken that they don't matter. something about rust's
  //      internals, such as HashMap and HashSet, but apparently other stuff,
  //      will seemingly clobber memory that bash/libc wants to use, resulting
  //      in the same/similar crashes. this may be an artifact of forcing glibc
  //      dlopen to accept PIE executables as shared libraries, given that the
  //      crashes did not occur when using the actual .so as the dlopen target,
  //      but that doesn't really matter. oddly enough, i got as far as
  //      determining that using BTreeMap/BTreeSet instead stopped the crashes
  //      from happening but that further code resulted in the same issues.
  //
  //      so instead, since we can't reliably inject the PIE executable itself,
  //      insject will re-implement the setns.so logic in templated lldb "C",
  //      much like the original setns toy script. honestly, torvalds should
  //      put the glibc devs heads on pikes for constantly breaking userland
  //      and blaming it on everyone else.
  //      (ノಠ □ ಠ)ノ彡┻━┻

  //crashes
  //let mut ret = std::collections::HashMap::new();
  //ret.insert("hello".to_string(), "world".to_string());

  //does not crash
  //let mut ret = vec!();
  //ret.push(4);
  //ret.push(5);

  //does not crash
  //let mut ret = std::collections::BTreeMap::new();
  //ret.insert("hello".to_string(), "world".to_string());

  //crashes
  //let mut ret = std::collections::HashSet::new();
  //ret.insert("hello".to_string());
  //ret.insert("world".to_string());

  //does not crash
  //let mut ret = std::collections::BTreeSet::new();
  //ret.insert("hello".to_string());
  //ret.insert("world".to_string());

  //println!("testing 1,2,3 {:?}", ret);
  //return;

  //println!("testing 1,2,3");
  //stdout("on_load()\n");
  unsafe {
    STATE.on_load_hit = true;
  }

  let ld_preloaded = match std::env::var("LD_PRELOAD") {
    Ok(_) => true,
    Err(_) => false
  };

  if ld_preloaded {
    unsafe {
      STATE.load_type = Some(LoadType::LdPreload);
    }
  }

  let vars = if !ld_preloaded {
    //println!("overrides: getenv: {}, setenv: {}, unsetenv: {}",
    //         overrides_getenv(), overrides_setenv(), overrides_unsetenv());
    get_and_unset_env(&["LD_PRELOAD", "SETNS_ARGS", "SETNS_JSON"])
  } else {
    // use environ / real getenv/setenv
    //real_get_and_unset_env(&["LD_PRELOAD", "SETNS_ARGS", "SETNS_JSON"])
    get_and_sanitize_env(&["LD_PRELOAD", "SETNS_ARGS", "SETNS_JSON"])
  };
  //println!("vars: {:?}", vars);

  //println!("on_load: pid={}", unsafe{getpid()});
  match vars.get("SETNS_JSON") {
    Some(json_str) => {
      match from_json(json_str.as_str()) {
        Some(opts) => {
          unsafe { OPTS = Some(opts) };
        },
        None => {
          println!("from_json -> None");
          std::process::exit(1);
        }
      }
    },
    None => {
      match vars.get("SETNS_ARGS") {
        Some(args_str) => {
          let opts = parse_opts(&args_str);
          unsafe { OPTS = Some(opts) };
        },
        None => { }
      }
    }
  }

  if unsafe { (*&raw mut OPTS).is_none() } {
    if ld_preloaded {
      println!("SETNS_ARGS env var is required");
      std::process::exit(1);
    } else {
      //stdout("likely insject being invoked as own executable\n");
      unsafe {
        STATE.load_type = Some(LoadType::Executable);
      }
      //println!("{:?}", unsafe { (*&raw mut STATE).clone() });
      // dlopen w/o SETNS_JSON/SETNS_ARGS env set
      //println!("dlopen w/o SETNS_JSON/SETNS_ARGS env set");
      return;
    }
  } else {
    unsafe {
      let opts = (*&raw mut OPTS).as_ref().unwrap();
      STATE.target = opts.raw_address.clone();
      STATE.target_name = opts.sym_name.clone();
      if !opts.raw_address.is_none() {
        STATE.target_is_main = Some(false);
      } else {
        STATE.target_is_main = match &opts.sym_name {
          None => Some(true),
          Some(name) => Some(name == "main")
        };
      }
    }
  }

  if !ld_preloaded {
    unsafe {
      STATE.load_type = Some(LoadType::DlOpen);
    }
    // dlopen w/ SETNS_JSON/SETNS_ARGS env set
    //println!("dlopen w/ SETNS_JSON/SETNS_ARGS env set, OPTS: {:?}", unsafe { &OPTS });
    do_setns_external(0 as *const c_char);
    return;
  }

  let opts = unsafe { (*&raw mut OPTS).as_ref().unwrap() };
  setup_frida_gum();

  match &opts.raw_address {
    Some(addr) => {
      do_hook(*addr)
    },
    None => {
      match &opts.sym_name {
        Some(name) => {
          if !try_hook_by_sym_name(name) && name != "main" {
            println!("could not find symbol: {}", name);
            std::process::exit(1);
          }
        },
        None => {
          // this is fine
          if has_sym(&"main".to_string()).is_none() {
            // early enough golang binary hook
            match has_sym(&"runtime.schedule".to_string()) {
              Some(_) => { try_hook_by_sym_name(&"runtime.schedule".to_string()); },
              None => {}
            };
          } else {
          }
        }
      }
    }
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn __libc_start_main(main: *const c_void, argc: c_int, argv: *mut *mut c_char,
                                    init: *const c_void, fini: *const c_void,
                                    rtld_fini: *const c_void, stack_end: *mut c_void) -> c_int {
  //stdout("__libc_start_main()\n");
  unsafe {
    STATE.libc_start_main_hit = true;
  }

  match unsafe { &mut *std::ptr::addr_of_mut!(OPTS) } {
    Some(opts) => {
      match opts.raw_address {
        Some(_) => {},
        None => {
          opts.raw_address = Some(main as usize);
          unsafe {
            STATE.target = Some(main as usize);
            STATE.target_name = Some("main".to_owned());
            STATE.target_is_main = Some(true);
          }
          do_hook(main as usize);
        }
      }
    },
    None => {
      //note: this will be the case when we're executing as the actual binary
    }
  }

  let real_libc_start_main = unsafe {
    //RTLD_NEXT
    dlsym(usize::MAX as *mut c_void, b"__libc_start_main\0".as_ptr() as *const c_char)
  };
  if real_libc_start_main as usize == 0 {
    println!("could not find real __libc_start_main???");
    std::process::exit(1);
  }
  unsafe {
    core::intrinsics::transmute::<*mut c_void,
      extern "C" fn(main: *const c_void, argc: c_int, argv: *mut *mut c_char,
                init: *const c_void, fini: *const c_void,
                rtld_fini: *const c_void, stack_end: *mut c_void) -> c_int
    >(real_libc_start_main)(main, argc, argv, init, fini, rtld_fini, stack_end)
  }
}

fn do_hook(addr: usize) {
  let raw_listener = hook_by_addr(unsafe { &mut *std::ptr::addr_of_mut!(HOOK_SINGLETON) }, addr);
  unsafe {
    HOOK_SINGLETON.raw_listener = raw_listener;
  }
}

struct SetnsHook {
  raw_listener: *mut gumshoe::gum::ffi::ArchetypalListener
}

impl ArchetypalListener for SetnsHook {
  fn on_enter(&mut self, _ic: gum::GumInvocationContext) {
    //stdout("ArchetypalListener for SetnsHook::on_enter()\n");
    unsafe {
      STATE.target_hit = true;
    }
    gumshoe::detach_hook(self.raw_listener);

    match unsafe { &*std::ptr::addr_of_mut!(OPTS) } {
      Some(_) => {
        do_setns_external(0 as *const c_char);
      },
      None => { unreachable!(); }
    }
  }

  fn on_leave(&mut self, _ic: gum::GumInvocationContext) {
    gumshoe::detach_hook(self.raw_listener);
  }
}

fn open_for_fd(path: &String, flag: c_int) -> c_int {
  let path = std::ffi::CString::new(path.clone()).unwrap();
  unsafe { open(path.as_ptr(), flag) }
}

fn ns_fd(nstype: &str, path: &Option<String>, no: bool, pid: &Option<usize>) -> Option<c_int> {
  if no {
    return None;
  }
  match path {
    Some(path) => Some(open_for_fd(&path, 0)),
    None => if !no {
      match pid {
        Some(pid) => {
          Some(open_for_fd(&format!("/proc/{}/ns/{}", pid, nstype), 0))
        },
        None => panic!("Attempted to load {} ns from pid", nstype)
      }
    } else { None }
  }
}

fn setns_wrapper(ns_fd: Option<c_int>, nstype: c_int) -> String {
  let ret = match ns_fd {
    Some(ns_fd) if ns_fd >= 0 => {
      let r = unsafe { setns(ns_fd, nstype) };
      if r < 0 {
        //format!("err:{}", unsafe { *__errno_location() } )
        format!("err:??")
      } else {
        format!("{}", r)
      }
    },
    Some(ns_fd) if ns_fd < 0 => format!("fd:{}", ns_fd),
    _  => "N/A".to_owned()
  };
  let _ = close_wrapper(ns_fd);
  ret
}

fn close_wrapper(ns_fd: Option<c_int>) -> c_int {
  match ns_fd {
    Some(ns_fd) if ns_fd >= 0 => unsafe { close(ns_fd) },
    _  => { -1 }
  }
}


#[unsafe(no_mangle)]
pub extern "C" fn do_setns_external(args: *const c_char) {
  //stdout("do_setns_external()\n");
  //println!("{:?}", unsafe { STATE.clone() });

  let opts: Opts = if args.is_null() {
    match unsafe { &*std::ptr::addr_of_mut!(OPTS) } {
      Some(opts) => {
        (*opts).clone()
      },
      None => {
        panic!("args is null, but OPTS is None")
      },
    }
  } else {
    let args = unsafe { std::ffi::CStr::from_ptr(args) };
    let args = match args.to_str() {
      Ok(args) => args.to_owned(),
      Err(err) => {
        println!("failed to decode args: {}", err);
        std::process::exit(1);
      }
    };
    if args.starts_with("{") {
      from_json(&args).unwrap()
    } else {
      parse_opts(&args)
    }
  };

  {
    let threads = match std::fs::read_dir("/proc/self/task") {
      Ok(dir) => {
        Some(dir.count())
      },
      Err(_) => None
    };

    if threads.unwrap_or(1) > 1 {
      if opts.strict {
        println!("[insject] strict mode: multiple threads detected -> exiting");
        unsafe { exit(1) };
      } else {
        println!("[insject] wanrning: multiple threads detected");
      }
    }
  }

  let mnt_ns_fd = ns_fd("mnt", &opts.mnt, opts.no_mnt, &opts.target_pid);
  let net_ns_fd = ns_fd("net", &opts.net, opts.no_net, &opts.target_pid);
  let time_ns_fd = ns_fd("time", &opts.time, opts.no_time, &opts.target_pid);
  let ipc_ns_fd = ns_fd("ipc", &opts.ipc, opts.no_ipc, &opts.target_pid);
  let uts_ns_fd = ns_fd("uts", &opts.uts, opts.no_uts, &opts.target_pid);
  let pid_ns_fd = ns_fd("pid", &opts.pid, opts.no_pid, &opts.target_pid);
  let cgroup_ns_fd = ns_fd("cgroup", &opts.cgroup, opts.no_cgroup, &opts.target_pid);

  let mut same_userns = false;
  {
    let user_ns_path = match &opts.userns {
      Some(path) => {
        Some(path.clone())
      },
      None => {
        match &opts.target_pid {
          Some(pid) => {
            Some(format!("/proc/{}/ns/user", pid))
          },
          None => None
        }
      }
    };
    match user_ns_path {
      Some(path) => {
        match std::fs::read_link(path) {
          Ok(userns_real) => {
            match std::fs::read_link("/proc/self/ns/user") {
              Ok(self_userns_real) => {
                if userns_real == self_userns_real {
                  same_userns = true;
                }
              },
              Err(_) => {}
            };
          },
          Err(_) => {}
        };
      },
      None => {}
    };
  }
  let user_ns_fd = ns_fd("user", &opts.userns, same_userns || opts.no_userns, &opts.target_pid);

  let apparmor_profile: String = match &opts.apparmor_profile {
    Some(apparmor_profile) => (*apparmor_profile).clone(),
    None => if opts.no_apparmor {
      "unconfined".to_owned()
    } else {
      match &opts.target_pid {
        Some(pid) => {
          let profile = match std::fs::read_to_string(format!("/proc/{}/attr/current", pid)) {
            Ok(profile) => profile,
            Err(_) => "unconfined".to_string()
          };
          match profile.strip_suffix(" (enforce)\n") {
            Some(profile) => profile.to_owned(),
            None => profile
          }
        },
        None => "unconfined".to_owned()
      }
    }
  };
  //println!("apparmor_profile: {}", apparmor_profile);

  let attr_current_fd = if apparmor_profile == "unconfined" {
    None
  } else {
    Some(open_for_fd(&"/proc/self/attr/current".to_owned(), O_WRONLY))
  };

  let mut user_ns_r = "N/A".to_owned();
  if opts.userns_first {
    user_ns_r = setns_wrapper(user_ns_fd, CLONE_NEWUSER);
  }

  let net_ns_r = setns_wrapper(net_ns_fd, CLONE_NEWNET);

  //note: "CLONE_NEWTIME" is not really defined and the kernel defines it as 0,
  //      which is a problem as 0 is the magic "accept any" setns flag value
  let time_ns_r = setns_wrapper(time_ns_fd, 0);

  let ipc_ns_r = setns_wrapper(ipc_ns_fd, CLONE_NEWIPC);
  let uts_ns_r = setns_wrapper(uts_ns_fd, CLONE_NEWUTS);
  let cgroup_ns_r = setns_wrapper(cgroup_ns_fd, CLONE_NEWCGROUP);

  //note: at this moment in time in the thread doing setns for itself, we are
  //      not doing anything else. therefore the biggest target is the PID
  //      namespace, which only becomes an attack surface once we fork and have
  //      the child be exposed to PID-related operations in the container.
  let mnt_ns_r = setns_wrapper(mnt_ns_fd, CLONE_NEWNS);

  let pid_ns_r = setns_wrapper(pid_ns_fd, CLONE_NEWPID);

  if !opts.userns_first {
    user_ns_r = setns_wrapper(user_ns_fd, CLONE_NEWUSER);
  }

  let user_parts: std::vec::Vec<&str> = opts.user.split(':').collect();
  let (uid, gid, groups) = match &*user_parts {
    &[uid, gid, groups] => { (uid, gid, groups) },
    &[uid, gid] => { (uid, gid, "0") },
    &[uid] => { (uid, "0", "0") },
    _ => unreachable!()
  };

  let uid: u32 = uid.parse().unwrap();
  let gid: u32 = gid.parse().unwrap();
  let mut groups: std::vec::Vec<u32> = groups.split(',').into_iter().map(|g| g.parse::<u32>().unwrap()).collect();
  groups.shrink_to_fit();
  let groups_ptr = groups.as_ptr();
  let groups_len = groups.len();
  let user_r = unsafe {
    let groups_r = setgroups(groups_len, groups_ptr);
    let gid_r = setgid(gid);
    let uid_r = setuid(uid);
    format!("{}/{}/{}", uid_r, gid_r, groups_r)
  };

  let mut selinux = false;
  let aa_r = match attr_current_fd {
    None => "N/A".to_owned(),
    Some(fd) => {
      let s: String = apparmor_profile;
      selinux = s.as_bytes()[s.len()-1] == 0;
      let s: String = if selinux {
        match s.get(..s.len()-1) {
          Some(ss) => ss.into(),
          None => s
        }
      } else {
        s
      };

      let r = if selinux {
        //selinux (android)
        let fd = open_for_fd(&"/proc/thread-self/attr/current".to_owned(), O_WRONLY);
        let r = unsafe {
          write(fd, s.as_bytes().as_ptr() as *const c_void, s.len())
        };
        let _ = unsafe { close(fd) };
        r
      } else {
        //apparmor
        let changeprofile: String = format!("changeprofile {}", s);
        let data = std::ffi::CString::new(changeprofile.as_str()).unwrap();
        let r = unsafe {
          write(fd, data.as_ptr() as *const c_void, s.len())
        };
        let _ = unsafe { close(fd) };
        r
      };
      if r > 0 {
        format!("{}", s)
      } else {
        format!("{}->{}", s, r)
      }
    }
  };

  if !opts.no_fork && pid_ns_r == "0" {
    let p = unsafe { fork() };
    if p != 0 {
      let mut status = 0;
      let _ = unsafe { wait(&mut status as *mut i32) };
      unsafe { exit(0) };
    }
  }

  if !opts.userns_first {
    println!("[insject] -> mnt: {}, net: {}, time: {}, ipc: {}, uts: {}, pid: {}, cgroup: {}, userns: {}, {}: {}, user: {}",
             mnt_ns_r, net_ns_r, time_ns_r, ipc_ns_r, uts_ns_r, pid_ns_r, cgroup_ns_r, user_ns_r, if selinux { "selinux" } else { "apparmor" }, aa_r, user_r);
  } else {
    println!("[insject] -> userns: {}, mnt: {}, net: {}, time: {}, ipc: {}, uts: {}, pid: {}, cgroup: {}, {}: {}, user: {}",
             user_ns_r, mnt_ns_r, net_ns_r, time_ns_r, ipc_ns_r, uts_ns_r, pid_ns_r, cgroup_ns_r, if selinux { "selinux" } else { "apparmor" }, aa_r, user_r);
  }

  if opts.strict {
    if  (mnt_ns_r != "0" && mnt_ns_r != "N/A") ||
        (net_ns_r != "0" && net_ns_r != "N/A") ||
        (time_ns_r != "0" && time_ns_r != "N/A") ||
        (ipc_ns_r != "0" && ipc_ns_r != "N/A") ||
        (uts_ns_r != "0" && uts_ns_r != "N/A") ||
        (pid_ns_r != "0" && pid_ns_r != "N/A") ||
        (cgroup_ns_r != "0" && cgroup_ns_r != "N/A") ||
        (user_ns_r != "0" && user_ns_r != "N/A") ||
        user_r.find("-1").is_some() {
      println!("[insject] strict mode: some operations failed -> exiting");
      unsafe { exit(1) };
    }
  }

  //unsafe {
    //(*__errno_location()) = 0;
  //}
}
