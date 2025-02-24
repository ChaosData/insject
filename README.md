# Insject

`insject` is a tool for poking at containers. It enables you to run an
arbitrary command in a container or any mix of Linux namespaces. It supports
three main use-cases:

* LD_PRELOAD-mode using `libsetns.so` (`LD_PRELOAD=./libsetns.so SETNS_ARGS="..."`)
* Running a host command in a container (`insject ... -- <cmd>...`)
* Forcing a running process into a container (`insject ... -! <pid>`)

When using the first two modes, the `-s <symbol>` option is used to place a
function hook that triggers the containerization of the process. This can help
with simple commands that need to load resources from the host filesystem by
having them containerize on calling a specific function after initializing.

For processes with more complicated initialization routines, such as scripting
languages, the third use-case may be preferable, enabling one to ensure full
initialization before entering a container.

***Note:*** insject and libsetns.so share the same limitations as `setns(2)` in
that they may fail when a process contains multiple threads.

***WARNING:*** Be careful when accessing or executing files in containers as
they may be able to abuse the access of the joined process to escape.

## Installation

```
$ git clone https://github.com/ChaosData/insject
$ cd insject/setns-so
$ ./fetch-frida.sh 16.5.9 linux x86_64 x86_64-unknown-linux-gnu
$ cargo build --lib --release && cargo build --bin insject --release
$ pip3 install --user lief
$ ./patch.py target/release/insject
```

### Android

```
## cargo-ndk
$ cd insject/setns-so
$ export ANDROID_NDK_HOME=...
$ ./fetch-frida.sh 16.5.9 android arm64 aarch64-linux-android
$ cargo ndk -t aarch64-linux-android build --lib --release && cargo ndk -t aarch64-linux-android build --bin insject --release
$ rm .cargo/config.toml
$ ./fetch-frida.sh 16.5.9 android x86_64 x86_64-linux-android
$ cargo ndk -t x86_64-linux-android build --lib --release && cargo ndk -t x86_64-linux-android build --bin insject --release

## cross
$ cd setns-so
$ cross build --lib --release --target aarch64-linux-android && cross build --bin insject --release --target aarch64-linux-android
$ rm .cargo/config.toml
$ cross build --lib --release --target x86_64-linux-android && cross build --bin insject --release --target x86_64-linux-android
```

## Examples

```
## Terminal 1
$ docker run --rm -it -v $(PWD):/FOO:ro alpine /bin/sh
/ # ls /
bin    dev    etc    FOO   home   lib    media  mnt    opt    proc   root   run    sbin   srv    sys    tmp    usr    var
```

```
## Terminal 2
$ sudo bash
# echo $$
164001
#
```

```
## Terminal 3
$ docker ps -q
acd1d4d97027
$ docker inspect acd1d4d97027 | jq .[0].State.Pid
68575
$ sudo LD_PRELOAD=./target/release/libsetns.so SETNS_ARGS="-I 68575 --user 0:85:0,1,2,3,4" ls /
setns -> mnt: 0, net: 0, time: 0, ipc: N/A, uts: 0, pid: 0, cgroup: 0, userns: 0, apparmor: docker-default, user: 0/0/0
bin    dev    etc    FOO   home   lib    media  mnt    opt    proc   root   run    sbin   srv    sys    tmp    usr    var
```

```
## Terminal 2
# setns -> mnt: 0, net: 0, time: 0, ipc: N/A, uts: 0, pid: 0, cgroup: 0, userns: 0, apparmor: docker-default, user: 0/0/0
# ls /
bin    dev    etc    FOO   home   lib    media  mnt    opt    proc   root   run    sbin   srv    sys    tmp    usr    var
# ifconfig
eth0      Link encap:Ethernet  HWaddr 02:42:AC:11:00:02
          inet addr:172.17.0.2  Bcast:172.17.255.255  Mask:255.255.0.0
          UP BROADCAST RUNNING MULTICAST  MTU:1500  Metric:1
          RX packets:525 errors:0 dropped:0 overruns:0 frame:0
          TX packets:0 errors:0 dropped:0 overruns:0 carrier:0
          collisions:0 txqueuelen:0
          RX bytes:49454 (48.2 KiB)  TX bytes:0 (0.0 B)

lo        Link encap:Local Loopback
          inet addr:127.0.0.1  Mask:255.0.0.0
          UP LOOPBACK RUNNING  MTU:65536  Metric:1
          RX packets:0 errors:0 dropped:0 overruns:0 frame:0
          TX packets:0 errors:0 dropped:0 overruns:0 carrier:0
          collisions:0 txqueuelen:1000
          RX bytes:0 (0.0 B)  TX bytes:0 (0.0 B)
```

```
## Terminal 3
$ sudo ./target/release/insject -I 68575 --user 0:85:0,1,2,3,4 -- ls /
setns -> mnt: 0, net: 0, time: 0, ipc: N/A, uts: 0, pid: 0, cgroup: 0, userns: 0, apparmor: docker-default, user: 0/0/0
bin  dev  etc  FOO  home  lib	media  mnt  opt  proc  root  run  sbin	srv  sys  tmp  usr  var
$ sudo ./target/release/insject -I 68575 --user 0:85:0,1,2,3,4 -- id
setns -> mnt: 0, net: 0, time: 0, ipc: N/A, uts: 0, pid: 0, cgroup: 0, userns: 0, apparmor: docker-default, user: 0/0/0
uid=0 gid=85 groups=85,0,1,2,3,4
$ sudo ./target/release/insject -I 68575 --user 0:85:0,1,2,3,4 -- sh -c id
setns -> mnt: 0, net: 0, time: 0, ipc: N/A, uts: 0, pid: 0, cgroup: 0, userns: 0, apparmor: docker-default, user: 0/0/0
uid=0(root) gid=85(usb) groups=0(root),1(bin),2(daemon),3(sys),4(adm)
```

## Usage

```
$ insject --help
insject 1.0
Jeff Dileo <jeff.dileo@nccgroup.com>
A tool to simplify container testing that runs an arbitrary
command in the Linux namespaces of other processes.

WARNING: Be careful when accessing or executing files in containers as they may
         be able to abuse the access of the joined process to escape.

Note: The -! instrumentation mode has several differences from the LD_PRELOAD modes:
        * Forking is not supported
        * -S,--strict is not supported
        * errno values are not returned

USAGE:
    insject [FLAGS] [OPTIONS] [setns-opts]... [-- <cmd>...]

ARGS:
    <setns-opts>...    setns.so options. For detailed information, use --help-setns
    <cmd>...

FLAGS:
    -h, --help          Prints help information
        --help-setns    Prints help information for setns.so
    -V, --version       Prints version information

OPTIONS:
    -! <pid>        PID to instrument
$ insject --help-setns
libsetns.so 1.0
Jeff Dileo <jeff.dileo@nccgroup.com>
An inject-/LD_PRELOAD-able shim to simplify container testing by joining an external program
run with it into the Linux namespaces of other processes.

WARNING: Be careful when accessing or executing files in containers as they may
         be able to abuse the access of the joined process to escape.

USAGE:
    libsetns.so [FLAGS] [OPTIONS] [target-pid]

ARGS:
    <target-pid>    PID to source namespaces from by default

FLAGS:
        --help            Prints help information
    -A, --no-apparmor     Skip setting AppArmor profile
    -C, --no-cgroup       Skip setting cgroup namespace
    -F, --no-fork         Skip fork after entering PID namespace, if entering PID namespace
    -I, --no-ipc          Skip setting IPC namespace
    -M, --no-mnt          Skip setting mount namespace
    -N, --no-net          Skip setting network namespace
    -P, --no-pid          Skip setting PID namespace
    -T, --no-time         Skip setting time namespace
    -U, --no-userns       Skip setting user namespace
    -H, --no-uts          Skip setting UTS (hostname) namespace
    -S, --strict          Exit if any namespace attach fails
    -1, --userns-first    Set user namespace before other namespaces
    -V, --version         Prints version information

OPTIONS:
    -@, --raw-address <address>         Raw memory address to hook instead of a symbol
                                        Note: This is not an offset
    -c, --cgroup <cgroup>               Path to cgroup namespace to set
    -i, --ipc <ipc>                     Path to IPC namespace to set
    -m, --mnt <mnt>                     Path to mount namespace to set
    -n, --net <net>                     Path to network namespace to set
    -p, --pid <pid>                     Path to PID namespace to set
    -a, --apparmor-profile <profile>    Alternate AppArmor profile to set
    -s, --symbol <symbol>               Symbol to hook entry of instead of main
    -t, --time <time>                   Path to time namespace to set
        --user <user>                   <uid>[:<gid>[:<group,ids>]]) [default: 0:0:0]
    -u, --userns <userns>               Path to user namespace to set
    -h, --uts <uts>                     Path to UTS (hostname) namespace to set
```

# Weird Errors?

Due to the `build.rs` logic in use, when changing target architectures, you
will want to delete the `.config/cargo.toml` file generated within the
`setns-so/` directory before attempting a build using a different target than
the previous build. Additionally, depending on if using multiple separate build
environments out of the same clone, you may need to perform a `cargo clean` or
equivalent removal of targets to prevent linking errors.

# License

insject is licensed under the 2-clause BSD license.
