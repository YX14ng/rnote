# shellcheck shell=bash
# Shared environment for cross-compiling Rnote for Android.
#
# Source it with the target arch:  . android/scripts/env.sh aarch64|x86_64
#
# Overridable locations:
#   RNOTE_ANDROID_HOME  work dir for toolchains, builds and the deps prefix
#                       (default: ~/.cache/rnote-android)
#   ANDROID_NDK_HOME    Android NDK (default: $RNOTE_ANDROID_HOME/toolchain/android-ndk-r30)
#   ANDROID_API         minimum API level (default: 28, first one with iconv in bionic)

RNOTE_ANDROID_ARCH="${1:-aarch64}"
case "$RNOTE_ANDROID_ARCH" in
  aarch64) RNOTE_ANDROID_ABI=arm64-v8a ;;
  x86_64) RNOTE_ANDROID_ABI=x86_64 ;;
  *)
    echo "env.sh: unsupported arch '$RNOTE_ANDROID_ARCH' (expected aarch64 or x86_64)" >&2
    return 1
    ;;
esac

export RNOTE_ANDROID_ARCH RNOTE_ANDROID_ABI
export RNOTE_ANDROID_HOME="${RNOTE_ANDROID_HOME:-$HOME/.cache/rnote-android}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$RNOTE_ANDROID_HOME/toolchain/android-ndk-r30}"
export ANDROID_API="${ANDROID_API:-28}"

if [[ ! -d "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64" ]]; then
  echo "env.sh: Android NDK not found at '$ANDROID_NDK_HOME'" >&2
  return 1
fi

# Isolated rustup install, so the system Rust stays untouched.
if [[ -x "$RNOTE_ANDROID_HOME/cargo/bin/cargo" ]]; then
  export RUSTUP_HOME="$RNOTE_ANDROID_HOME/rustup"
  export CARGO_HOME="$RNOTE_ANDROID_HOME/cargo"
  export PATH="$CARGO_HOME/bin:$PATH"
fi

export RNOTE_ANDROID_TRIPLE="$RNOTE_ANDROID_ARCH-linux-android"
export RNOTE_ANDROID_TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64"
export RNOTE_ANDROID_PREFIX="$RNOTE_ANDROID_HOME/prefix/$RNOTE_ANDROID_ABI"

triple_env="${RNOTE_ANDROID_TRIPLE//-/_}"
triple_env_upper="${triple_env^^}"
android_cc="$RNOTE_ANDROID_TOOLCHAIN/bin/$RNOTE_ANDROID_TRIPLE$ANDROID_API-clang"
export "CC_$triple_env=$android_cc"
export "CXX_$triple_env=$android_cc++"
export "AR_$triple_env=$RNOTE_ANDROID_TOOLCHAIN/bin/llvm-ar"
export "CARGO_TARGET_${triple_env_upper}_LINKER=$android_cc"
# rustc links Android targets with -nodefaultlibs, but the JITs in libffi and pcre2
# need __clear_cache from compiler-rt's builtins.
android_builtins="$("$android_cc" --rtlib=compiler-rt --print-libgcc-file-name)"
export "CARGO_TARGET_${triple_env_upper}_RUSTFLAGS=-C link-arg=$android_builtins"
unset triple_env triple_env_upper android_cc android_builtins

# The *-sys crates resolve the cross-built libraries via pkg-config, and link them
# statically so everything ends up in a single shared object.
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR=/
export PKG_CONFIG_LIBDIR="$RNOTE_ANDROID_PREFIX/lib/pkgconfig:$RNOTE_ANDROID_PREFIX/share/pkgconfig"
unset PKG_CONFIG_PATH
export SYSTEM_DEPS_LINK=static
