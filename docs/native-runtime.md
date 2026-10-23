# Experimental native Wine/FEX runtime

These are notes for the runtime validated locally with Vela 0.2.0. They are not a complete runtime installer or a claim that a downloadable native engine is included with the application.

## Supported configuration

- Apple Silicon; runtime deployment target macOS 26.5.
- ARM64 Wine and wineserver, with ARM64EC/FEX components.
- 64-bit Windows applications only in this configuration.
- No Rosetta dependency in the validated x64 execution path.

The Vela interface itself has a macOS 14.0 deployment target. The runtime's newer system requirement is separate.

The configuration uses high-address Windows shared data and an unrestricted custom-x18 entitlement. It does not have Apple's restricted entitlement for the low-address configuration needed by the full 32-bit path. Do not change the declared 32-bit capability to true without an engine that actually supports it.

## Pinned sources

| Component | Revision |
| --- | --- |
| Wine 11.18 | `6880117619afdf62b4ccc40a8c6268c86613131f` |
| FEX | `0df84d3844bcdb87bb7d3f5b8fb0959cd009c038` |
| Patch provider | Hadron v0.3.1; binary source commit `3f5f18e2c08e22eb539f1fbbee52d4ba3d9e05c4` |

The build uses 38 Wine patches and 5 FEX patches from the provider, plus Vela's capability patch in `runtime/patches/`. The latter prevents this high-address-only runtime from advertising unsupported 32-bit architectures.

Source locations, checksums, and dependency inputs are recorded in [native-runtime-sources.json](native-runtime-sources.json). Upstream attribution is retained in [THIRD_PARTY.md](../THIRD_PARTY.md).

## Build notes

The validated build used Apple clang 21 / SDK 26.5, Bison 3.8.2, CMake, LLVM dlltool/ar/ranlib 23.1.4, and LLD 23.1.1 from Rust 1.99.1.

Use LLVM ar/ranlib for Windows COFF archives. Apple ar did not produce usable import archives in this build.

Wine's configuration included:

~~~text
MACOSX_DEPLOYMENT_TARGET=26.5
CPPFLAGS=-I<private-headers> -DWINE_HIGH_USER_SHARED_DATA
CROSSCFLAGS=-O2 -DWINE_HIGH_USER_SHARED_DATA
LDFLAGS=-L<private-libraries>

configure --prefix=<new-runtime-directory>
          --enable-archs=arm64ec,aarch64,i386
          --with-mingw=/usr/bin/clang
          --without-x --disable-tests
          CC=/usr/bin/clang CXX=/usr/bin/clang++
          BISON=<bison-3.8.2>
~~~

The angle-bracket paths are build-specific inputs. This excerpt must be adapted to a prepared source tree and private dependency directories; it is not a standalone build script.

FEX Unix libraries were rebuilt with deployment target 26.5. FEX Windows PE DLLs were obtained from the verified provider runtime; the entire engine was not built from source in a single reproducible pipeline. A subsequent Wine install can replace FEX alias DLLs with stubs, so their final contents must be checked.

Private GnuTLS and FreeType dependencies were placed under `ext/lib`, relocated to library-relative paths, and signed locally after relocation. GnuTLS loading and `gnutls_global_init() == 0` were checked; this is not an end-to-end application HTTPS test.

The local Wine loader is packaged by `scripts/package_native_loader.py` in a small application bundle with `com.apple.security.custom-x18-abi-toggle`. That script modifies a newly prepared runtime in place and rejects an already packaged loader. It is not needed to build the Vela manager.

## Runtime capabilities

A compatible runtime root uses a declaration such as:

~~~json
{
  "name": "Wine ARM64 + FEX",
  "version": "11.18-vela1",
  "backend": "arm64-fex",
  "supports_x86_32": false,
  "min_macos": "26.5"
}
~~~

Keep the matching `bin/wine`, `bin/wineserver`, and `lib/wine/` contents together. The presence of i386 DLLs does not mean the runtime can execute 32-bit applications.

The validated runtime does not include an installed DirectX graphics stack, Vulkan/MoltenVK, DXMT, or GStreamer/FFmpeg.

## Validation tools

`scripts/build_windows_probes.py` creates original x64 and x86 PE fixtures using clang and an LLVM COFF linker, without a Windows SDK. `tests/macos/translation_probe.c` reads the macOS translated-process flag.

`scripts/native_smoke_test.py` requires paths for the core, engine, both PE fixtures, the compiled translation probe, and a new disposable library. Its checks cover registration, initialization, actual Windows registry scaling, an external x64 PE launch, the translated-process flag, x86 rejection, and scoped stop.

See [Validation](validation.md) for the recorded results. Use a fresh test library and never point the script at a working user's library.

## Distribution and updates

The engine binaries are not included in the Vela app or source archive. The source archive includes the Vela-authored Wine patch and its LGPL notices, not the complete Wine/FEX source distribution.

Before distributing any engine build, provide its applicable notices, corresponding source materials, patches, and build inputs as required by its components' licenses. An upstream main-branch link is not a replacement for the matching source of a modified binary.

Install a new engine version beside the old one and validate it in a new environment. Automatic engine installation and conversion of Intel environments are not implemented.
