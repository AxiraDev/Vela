import SwiftUI
import AppKit
import UniformTypeIdentifiers

struct LaunchersView: View {
    @EnvironmentObject private var model: AppModel
    @State private var query = ""
    @State private var installing: LauncherEntry?
    private var entries: [LauncherEntry] {
        model.launchers.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) || tr($0.description).localizedCaseInsensitiveContains(query) }
    }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 8) {
                    Text(tr("ВАША ИГРОВАЯ БИБЛИОТЕКА")).font(.caption).tracking(1.8).foregroundStyle(accent)
                    Text(tr("Игровые лаунчеры")).font(.system(size: 30, weight: .semibold, design: .rounded))
                    Text(tr("Официальный установщик, отдельное окружение и ярлык — в Vela."))
                        .foregroundStyle(.secondary)
                }.padding(.top, 24)
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                    TextField(tr("Найти лаунчер"), text: $query).textFieldStyle(.plain)
                    if !query.isEmpty { Button { query = "" } label: { Image(systemName: "xmark.circle.fill") }.buttonStyle(.plain) }
                }.padding(12).background(.quaternary.opacity(0.4), in: RoundedRectangle(cornerRadius: 12))
                if model.engines.isEmpty {
                    HStack {
                        Label(tr("Сначала подключите движок Wine."), systemImage: "cpu")
                        Spacer()
                        Button(tr("Выбрать Wine")) { model.chooseEngine() }.disabled(model.busy)
                    }.padding(16).background(accent.opacity(0.08), in: RoundedRectangle(cornerRadius: 12))
                }
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 310), spacing: 16)], spacing: 16) {
                    ForEach(entries) { entry in
                        VStack(alignment: .leading, spacing: 14) {
                            HStack(alignment: .top) {
                                Image(systemName: entry.download == nil ? "info.circle" : "gamecontroller.fill")
                                    .font(.title2).foregroundStyle(accent).frame(width: 44, height: 44)
                                    .background(accent.opacity(0.12), in: RoundedRectangle(cornerRadius: 12))
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(entry.name).font(.headline)
                                    Text(entry.download == nil ? tr("Информация") : (entry.requires_wow64 ? tr("Установщик · 32/64 бит") : tr("Установщик · 64 бита")))
                                        .font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer(minLength: 0)
                            }
                            Text(tr(entry.description)).font(.callout).foregroundStyle(.secondary)
                            Text(tr(entry.note)).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                            let count = model.bottles.filter { $0.installation?.launcher_id == entry.id }.count
                            if count > 0 { Label(trf("Environments: %ld", count), systemImage: "shippingbox").font(.caption).foregroundStyle(accent) }
                            Spacer(minLength: 0)
                            HStack {
                                if entry.download != nil {
                                    Button(tr("Установить")) { installing = entry }.buttonStyle(.borderedProminent)
                                        .disabled(model.busy || model.engines.isEmpty)
                                }
                                Spacer()
                                if let url = URL(string: entry.website) { Link(tr("Официальный сайт ↗"), destination: url).font(.caption) }
                            }
                        }.padding(20).frame(maxWidth: .infinity, minHeight: 230, alignment: .topLeading)
                            .background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 18))
                    }
                }
                if entries.isEmpty { ContentUnavailableView.search(text: query) }
                Text(tr("Совместимость лаунчера и каждой игры зависит от Wine. Игры с драйверами античита могут требовать Windows. Обновления клиентов загружаются самими лаунчерами внутри их окружений."))
                    .font(.caption).foregroundStyle(.secondary)
            }.padding(32)
        }.sheet(item: $installing) { InstallLauncherView(entry: $0) }
    }
}

struct InstallLauncherView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    let entry: LauncherEntry
    @State private var name = ""
    @State private var engineID = ""
    @State private var dpi = 192
    private var engine: Engine? { model.engines.first { $0.id == engineID } }
    private var allowed: Bool {
        engine.map(entry.accepts) == true && !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !model.busy
    }
    private func start(local: URL? = nil) {
        model.install(entry, engineID: engineID, name: name, dpi: dpi, localFile: local)
        dismiss()
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Label(trf("Install %@", entry.name), systemImage: "arrow.down.app").font(.title2.weight(.semibold))
            Text(tr("Vela создаст новое окружение и откроет мастер установки Windows. Игры этого лаунчера будут храниться в том же окружении."))
                .foregroundStyle(.secondary)
            TextField(tr("Название окружения"), text: $name).textFieldStyle(.roundedBorder)
            EnginePickerView(engines: model.engines, selection: $engineID, requires32Bit: entry.requires_wow64)
            if let engine {
                if !entry.accepts(engine) {
                    Label(tr("Нужен движок с поддержкой Windows 32/64 бит (WoW64). Выбранный движок поддерживает только 64 бита."), systemImage: "exclamationmark.triangle")
                        .font(.callout).foregroundStyle(.orange)
                } else if engine.supports_x86_32 == nil && entry.requires_wow64 {
                    Text(tr("Поддержка 32-битных установщиков в этой сборке неизвестна. Проверьте наличие WoW64 у поставщика движка.")).font(.caption).foregroundStyle(.orange)
                }
                if engine.needsRosetta {
                    Text(tr("Выбран Intel-движок: на Apple Silicon ему нужна Rosetta.")).font(.caption).foregroundStyle(.orange)
                }
            }
            Picker(tr("Масштаб Windows"), selection: $dpi) {
                ForEach([96, 120, 144, 168, 192, 240, 288], id: \.self) { Text("\($0 * 100 / 96)%").tag($0) }
            }
            Text(tr(entry.note)).font(.caption).foregroundStyle(.secondary)
            if let url = entry.download.flatMap(URL.init(string:)) {
                Text(trf("Source: %@", url.host ?? "")).font(.caption).foregroundStyle(.secondary)
            }
            HStack {
                Button(tr("Выбрать скачанный файл…")) {
                    let panel = NSOpenPanel()
                    panel.title = trf("Official %@ installer", entry.name)
                    panel.allowedContentTypes = [UTType(filenameExtension: URL(fileURLWithPath: entry.filename).pathExtension)].compactMap { $0 }
                    if panel.runModal() == .OK, let url = panel.url { start(local: url) }
                }.disabled(!allowed)
                Spacer()
                Button(tr("Отмена")) { dismiss() }.keyboardShortcut(.cancelAction)
                Button(tr("Скачать и установить")) { start() }.buttonStyle(.borderedProminent)
                    .disabled(!allowed).keyboardShortcut(.defaultAction)
            }
        }.padding(28).frame(width: 640).onAppear {
            name = entry.name
            engineID = model.engines.first(where: entry.accepts)?.id ?? model.engines.first?.id ?? ""
            dpi = (NSScreen.main?.backingScaleFactor ?? 1) >= 2 ? 192 : 96
        }
    }
}
