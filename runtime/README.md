# Runtime components

The Rust manager and SwiftUI interface are original Vela code under MIT.
The optional Wine engine and FEX emulator are separate open-source components.
Wine patches here are authored for this runtime and licensed under LGPL-2.1-or-later,
consistent with the files they modify. Upstream notices and attribution remain intact.

The local capability patch prevents a high-address-only build from advertising
32-bit architectures that it cannot actually initialize.
