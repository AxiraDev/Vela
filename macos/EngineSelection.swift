import SwiftUI

struct Engine: Codable, Identifiable, Sendable {
    let id: String
    let name: String
    let wine: String
    let version: String
    let architecture: String?
    let backend: String?
    let supports_x86_32: Bool?
    var isNative: Bool { architecture == "arm64" }
    var needsRosetta: Bool { backend == "rosetta" }
    var windowsSupportTitle: String {
        switch supports_x86_32 {
        case true: return tr("Windows apps: 32-bit and 64-bit")
        case false: return tr("Windows apps: 64-bit only")
        default: return tr("Windows apps: 32/64-bit support unverified")
        }
    }
    var macArchitectureTitle: String {
        switch architecture {
        case "arm64": return "Apple Silicon (ARM64)"
        case "x86_64": return "Intel (x86_64)"
        default: return tr("Архитектура не определена")
        }
    }
}

struct EngineSupportView: View {
    let engine: Engine
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(engine.windowsSupportTitle).font(.headline).fixedSize(horizontal: false, vertical: true)
            Text(trf("Mac engine: %@", engine.macArchitectureTitle)).font(.caption).foregroundStyle(.secondary)
            if engine.needsRosetta {
                Label(tr("Requires Rosetta on Apple Silicon"), systemImage: "exclamationmark.triangle")
                    .font(.caption).foregroundStyle(.orange)
            } else if engine.isNative {
                Label(tr("Runs without Rosetta"), systemImage: "checkmark.shield")
                    .font(.caption).foregroundStyle(.green)
            }
        }
    }
}

struct EnginePickerView: View {
    let engines: [Engine]
    @Binding var selection: String
    var requires32Bit = false
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(tr("Движок Wine")).font(.headline)
            ScrollView {
                VStack(spacing: 10) {
                    ForEach(engines) { engine in
                        let selected = selection == engine.id
                        let compatible = !requires32Bit || engine.supports_x86_32 != false
                        Button { selection = engine.id } label: {
                            HStack(alignment: .top, spacing: 12) {
                                Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                                    .font(.title3).foregroundStyle(selected ? accent : .secondary)
                                VStack(alignment: .leading, spacing: 6) {
                                    EngineSupportView(engine: engine)
                                    Text(engine.name == engine.version ? engine.name : "\(engine.name) · \(engine.version)")
                                        .font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                                    if !compatible {
                                        Text(tr("Cannot run this 32-bit installer")).font(.caption).foregroundStyle(.orange)
                                    }
                                }
                                Spacer(minLength: 0)
                            }.padding(12).frame(maxWidth: .infinity, alignment: .leading)
                                .background(selected ? accent.opacity(0.12) : Color.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 12))
                                .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(selected ? accent : .clear, lineWidth: 1.5))
                                .contentShape(RoundedRectangle(cornerRadius: 12))
                        }.buttonStyle(.plain).disabled(!compatible).opacity(compatible ? 1 : 0.65)
                            .accessibilityLabel("\(engine.windowsSupportTitle), \(engine.macArchitectureTitle), \(engine.name)")
                            .accessibilityValue(selected ? tr("Selected") : tr("Not selected"))
                    }
                }
            }.frame(height: min(CGFloat(engines.count) * 132, 280))
            Text(tr("32/64-bit describes Windows applications. ARM64/Intel describes the Mac engine."))
                .font(.caption).foregroundStyle(.secondary)
        }
    }
}
