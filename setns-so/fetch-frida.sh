#!/bin/sh

set -e
SCRIPT=$0
cd `dirname $SCRIPT`
SCRIPT=`basename $SCRIPT`
while [ -L "$SCRIPT" ]
do
  SCRIPT=`readlink $SCRIPT`
  cd `dirname $SCRIPT`
  SCRIPT=`basename $SCRIPT`
done
SCRIPTDIR=`pwd -P`
cd "${SCRIPTDIR}"


if [ "$#" -ne 4 ]; then
  # ./fetch-frida.sh 16.5.9 linux x86_64 x86_64-unknown-linux-gnu
  # ./fetch-frida.sh 16.5.9 android arm64 aarch64-linux-android
  echo "usage: ${0} <version> <platform> <arch> <target-triple>" >&2
  exit 1
fi

version="${1}"
platform="${2}"
arch="${3}"
target="${4}"

name="frida-gum-devkit-${version}-${platform}-${arch}"
wget "https://github.com/frida/frida/releases/download/${version}/${name}.tar.xz"

tar -xvJf "${name}.tar.xz" --one-top-level="${name}"

mkdir -p "frida/${target}"
cp "${name}/libfrida-gum.a" "frida/${target}/libfrida-gum.a"
cp "${name}/frida-gum.h" "frida/${target}/frida-gum.h"
cp "${name}/frida-gum.h" frida/frida-gum.h
