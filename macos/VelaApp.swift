import SwiftUI
import AppKit
import UniformTypeIdentifiers

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
    var compatibility: String {
        if backend == "arm64-fex" { return supports_x86_32 == false ? tr("Без Rosetta · Windows 64 бит") : tr("ARM64 + FEX · без Rosetta") }
        if isNative { return tr("ARM64 · без Rosetta") }
        if needsRosetta { return tr("Intel · требуется Rosetta") }
        return tr("Архитектура не определена")
    }
}
struct Shortcut: Codable, Identifiable, Sendable {
    let id: String
    let name: String
    let executable: String
    let arguments: [String]
    let directory: String?
}
struct Bottle: Codable, Identifiable, Sendable {
    let id: String
    let name: String
    let engine_id: String
    let status: String
    let environment: [String: String]
    let shortcuts: [Shortcut]
    let last_log: String?
    let dpi: Int?
    let installation: LauncherInstallation?
}
struct LauncherInstallation: Codable, Sendable {
    let launcher_id: String
    let status: String
    let installer_path: String
    let client_path: String?
}
struct LauncherEntry: Codable, Identifiable, Sendable {
    let id: String
    let name: String
    let description: String
    let website: String
    let download: String?
    let filename: String
    let download_hosts: [String]
    let requires_wow64: Bool
    let note: String
    func accepts(_ engine: Engine) -> Bool { !requires_wow64 || engine.supports_x86_32 != false }
}
struct CatalogReply: Decodable, Sendable { let launchers: [LauncherEntry] }
struct InstalledReply: Decodable, Sendable { let id: String }
struct LibraryState: Codable, Sendable { let schema: Int; let engines: [Engine]; let bottles: [Bottle]; let deleted_bottles: [Bottle]? }
struct Snapshot: Codable, Sendable { let state: LibraryState; let root: String }
struct Reply<T: Decodable>: Decodable { let ok: Bool; let data: T?; let error: String? }
struct EmptyReply: Decodable, Sendable {}
struct LogReply: Decodable, Sendable { let text: String; let path: String? }

enum VelaError: LocalizedError {
    case message(String)
    var errorDescription: String? { if case let .message(s) = self { return s }; return nil }
}

struct Core: Sendable {
    let executable: URL
    let root: URL
    init() {
        executable = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/vela-core")
        if let override = ProcessInfo.processInfo.environment["VELA_HOME"] {
            root = URL(fileURLWithPath: override)
        } else {
            let besideApp = Bundle.main.bundleURL.deletingLastPathComponent()
            if FileManager.default.fileExists(atPath: besideApp.appendingPathComponent("Portable").path) {
                root = besideApp.appendingPathComponent("Data")
            } else {
                root = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Vela")
            }
        }
    }
    @MainActor
    func call<T: Decodable & Sendable>(_ request: [String: Any], as: T.Type) async throws -> T {
        var data = try JSONSerialization.data(withJSONObject: request, options: [.sortedKeys])
        data.append(0x0a)
        let operation = request["op"] as? String ?? ""
        let timeout: Double = operation == "purge_environment" ? 600 :
            (["create", "initialize", "install_launcher", "retry_launcher"].contains(operation) ? 180 :
             (operation == "delete_environment" ? 60 : 30))
        let executable = executable, root = root, language = L10n.code
        return try await Task.detached(priority: .userInitiated) {
            let process = Process()
            process.executableURL = executable
            var env = ProcessInfo.processInfo.environment
            env["VELA_HOME"] = root.path
            env["VELA_LANGUAGE"] = language
            process.environment = env
            let input = Pipe(), output = Pipe()
            process.standardInput = input
            process.standardOutput = output
            process.standardError = FileHandle.nullDevice
            try process.run()
            let timer = DispatchSource.makeTimerSource(queue: .global())
            timer.schedule(deadline: .now() + timeout)
            timer.setEventHandler { if process.isRunning { process.terminate() } }
            timer.resume()
            defer { timer.cancel() }
            try input.fileHandleForWriting.write(contentsOf: data)
            try input.fileHandleForWriting.close()
            let replyData = output.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            let reply: Reply<T>
            do { reply = try JSONDecoder().decode(Reply<T>.self, from: replyData) }
            catch { throw VelaError.message(trf("Vela core did not reply. Exit code: %ld.", Int(process.terminationStatus))) }
            guard reply.ok, let result = reply.data else { throw VelaError.message(reply.error ?? tr("Неизвестная ошибка")) }
            return result
        }.value
    }
}

@MainActor
final class AppModel: ObservableObject {
    @Published var engines: [Engine] = []
    @Published var bottles: [Bottle] = []
    @Published var deletedBottles: [Bottle] = []
    @Published var pendingDeletion: Bottle?
    @Published var selection: String? = "engines"
    @Published var loading = true
    @Published var busy = false
    @Published var activity = ""
    @Published var notice: String?
    @Published var error: String?
    @Published var showingSupport = false
    @Published var launchers: [LauncherEntry] = []
    @Published var downloadProgress: Double?
    @Published var canCancelDownload = false
    private var installationTask: Task<Void, Never>?
    let core = Core()

    var selectedBottle: Bottle? { bottles.first { $0.id == selection } }
    func loadCatalog() async {
        do { launchers = try await core.call(["op":"catalog"], as: CatalogReply.self).launchers }
        catch { self.error = error.localizedDescription }
    }
    func install(_ entry: LauncherEntry, engineID: String, name: String, dpi: Int, localFile: URL?) {
        guard !busy else { return }
        busy = true; notice = nil; downloadProgress = nil
        canCancelDownload = localFile == nil
        activity = trf("Downloading %@…", entry.name)
        installationTask = Task {
            var downloadFolder: URL?
            defer {
                if let downloadFolder { try? FileManager.default.removeItem(at: downloadFolder) }
                busy = false; canCancelDownload = false; downloadProgress = nil; activity = ""; installationTask = nil
            }
            do {
                let installer: URL
                if let localFile { installer = localFile }
                else {
                    guard let source = entry.download.flatMap(URL.init(string:)), LauncherDownload.allows(source, hosts: entry.download_hosts) else {
                        throw VelaError.message(tr("Нет проверенной ссылки загрузки."))
                    }
                    let configuration = URLSessionConfiguration.ephemeral
                    configuration.timeoutIntervalForRequest = 60
                    configuration.timeoutIntervalForResource = 900
                    configuration.urlCache = nil
                    configuration.httpCookieStorage = nil
                    configuration.urlCredentialStorage = nil
                    let session = URLSession(configuration: configuration)
                    defer { session.invalidateAndCancel() }
                    let delegate = LauncherDownload(hosts: entry.download_hosts) { received, total in
                        Task { @MainActor [weak self] in
                            guard let self, self.canCancelDownload else { return }
                            self.downloadProgress = total > 0 ? min(1, Double(received) / Double(total)) : nil
                            self.activity = trf("Downloading %@ · %@", entry.name, ByteCountFormatter.string(fromByteCount: received, countStyle: .file))
                        }
                    }
                    var request = URLRequest(url: source)
                    request.setValue("Vela/0.4", forHTTPHeaderField: "User-Agent")
                    let (temporary, response) = try await session.download(for: request, delegate: delegate)
                    defer { try? FileManager.default.removeItem(at: temporary) }
                    try Task.checkCancellation()
                    guard let http = response as? HTTPURLResponse, http.statusCode == 200,
                          let final = response.url, LauncherDownload.allows(final, hosts: entry.download_hosts) else {
                        throw VelaError.message(tr("Официальный сервер не отдал установщик. Попробуйте позже или выберите файл с сайта издателя."))
                    }
                    let size = (try FileManager.default.attributesOfItem(atPath: temporary.path)[.size] as? NSNumber)?.int64Value ?? 0
                    guard size >= 64, size <= LauncherDownload.maximumSize else { throw VelaError.message(tr("Недопустимый размер установщика (максимум 512 МБ).")) }
                    let folder = FileManager.default.temporaryDirectory.appendingPathComponent("vela-download-\(UUID().uuidString)")
                    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: false)
                    downloadFolder = folder
                    installer = folder.appendingPathComponent(entry.filename)
                    try FileManager.default.moveItem(at: temporary, to: installer)
                }
                try Task.checkCancellation()
                canCancelDownload = false; downloadProgress = nil
                activity = trf("Creating environment and opening the %@ installer…", entry.name)
                let reply = try await core.call(["op":"install_launcher", "launcher_id":entry.id, "engine_id":engineID, "name":name, "dpi":dpi, "path":installer.path], as: InstalledReply.self)
                await refresh()
                selection = reply.id
                notice = trf("Finish the %@ setup wizard. Vela will add a shortcut when it finds the client files.", entry.name)
            } catch {
                if Task.isCancelled { notice = tr("Загрузка отменена.") }
                else { self.error = error.localizedDescription }
                await refresh()
            }
        }
    }
    func cancelDownload() { installationTask?.cancel() }
    func refresh() async {
        defer { loading = false }
        do {
            let snapshot = try await core.call(["op": "list"], as: Snapshot.self)
            engines = snapshot.state.engines.sorted { ($0.isNative ? 0 : 1, $0.name) < ($1.isNative ? 0 : 1, $1.name) }
            bottles = snapshot.state.bottles
            deletedBottles = snapshot.state.deleted_bottles ?? []
        } catch { self.error = error.localizedDescription }
    }
    func perform(_ title: String, request: [String: Any], notice: String? = nil, completion: (() -> Void)? = nil) {
        guard !busy else { return }
        busy = true; activity = title; self.notice = nil
        Task {
            defer { busy = false; activity = "" }
            do {
                let _: EmptyReply = try await core.call(request, as: EmptyReply.self)
                self.notice = notice
                await refresh()
                completion?()
            } catch { self.error = error.localizedDescription; await refresh() }
        }
    }
    func chooseEngine() {
        let panel = NSOpenPanel()
        panel.title = tr("Выберите Wine.app, папку движка или файл wine")
        panel.prompt = tr("Добавить движок")
        panel.canChooseFiles = true; panel.canChooseDirectories = true
        panel.treatsFilePackagesAsDirectories = false
        let adjacent = Bundle.main.bundleURL.deletingLastPathComponent().appendingPathComponent("Engines")
        if FileManager.default.fileExists(atPath: adjacent.path) { panel.directoryURL = adjacent }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        perform(tr("Проверяем движок…"), request: ["op": "add_engine", "path": url.path], notice: tr("Движок добавлен. Теперь можно создать окружение."))
    }
    func chooseProgram(for bottle: Bottle) {
        let panel = NSOpenPanel()
        panel.title = tr("Открыть программу Windows")
        panel.prompt = tr("Запустить")
        panel.allowedContentTypes = ["exe", "msi", "bat", "cmd"].compactMap { UTType(filenameExtension: $0) }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        perform(tr("Передаём программу Wine…"), request: ["op": "launch", "id": bottle.id, "path": url.path], notice: tr("Запуск передан Wine. Результат доступен в журнале."))
    }
    func tool(_ name: String, bottle: Bottle) {
        perform(tr("Открываем инструмент Wine…"), request: ["op": "tool", "id": bottle.id, "tool": name], notice: tr("Инструмент передан Wine."))
    }
    func showFolder(_ path: String) { NSWorkspace.shared.open(URL(fileURLWithPath: path)) }
    func prefix(_ bottle: Bottle) -> String { core.root.appendingPathComponent("prefixes/\(bottle.id)").path }
}

let accent = Color(red: 0.51, green: 0.61, blue: 1)

enum SupportConfiguration {
    static var donationURL: URL? {
        guard let value = Bundle.main.object(forInfoDictionaryKey: "VelaDonationURL") as? String,
              let url = URL(string: value.trimmingCharacters(in: .whitespacesAndNewlines)),
              url.scheme?.lowercased() == "https", let host = url.host, !host.isEmpty,
              url.user == nil, url.password == nil else { return nil }
        return url
    }
}

@main
struct VelaApp: App {
    @StateObject private var model = AppModel()
    @AppStorage(L10n.preferenceKey) private var language = "system"
    var body: some Scene {
        WindowGroup {
            ContentView().environmentObject(model)
                .environment(\.locale, Locale(identifier: L10n.code))
                .id(language + L10n.code)
                .frame(minWidth: 1000, minHeight: 680)
                .tint(accent)
                .task {
                    await model.refresh(); await model.loadCatalog()
                    while !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(3))
                        if !model.busy && model.bottles.contains(where: { $0.installation?.status == "pending" }) { await model.refresh() }
                    }
                }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1160, height: 780)
        .commands {
            CommandGroup(replacing: .newItem) {}
            CommandGroup(after: .newItem) {
                Button(tr("Обновить библиотеку")) { Task { await model.refresh() } }
                    .keyboardShortcut("r", modifiers: .command)
            }
            CommandGroup(replacing: .appInfo) {
                Button(tr("О Vela")) {
                    NSApplication.shared.orderFrontStandardAboutPanel(options: [
                        .applicationName: "Vela",
                        .applicationVersion: "0.4.0 · Preview",
                        .credits: NSAttributedString(string: tr("Независимая открытая оболочка Wine\nSwiftUI + Rust · MIT\nWine и графические компоненты имеют собственные лицензии."))
                    ])
                }
            }
            CommandGroup(after: .help) {
                Button(tr("Поддержать Vela…")) { model.showingSupport = true }
            }
        }
        Settings {
            PreferencesView().environment(\.locale, Locale(identifier: L10n.code)).id(language + L10n.code)
                .frame(width: 560, height: 280)
        }
    }
}

struct ContentView: View {
    @EnvironmentObject private var model: AppModel
    @State private var creating = false
    var body: some View {
        NavigationSplitView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(spacing: 10) {
                    Image(systemName: "sailboat.fill").font(.system(size: 27)).foregroundStyle(accent)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Vela").font(.system(size: 26, weight: .semibold, design: .rounded))
                        Text(tr("WINDOWS НА ВАШЕМ MAC")).font(.system(size: 8, weight: .semibold)).tracking(1.1).foregroundStyle(.secondary)
                    }
                }.padding(.horizontal, 20).padding(.top, 36)
                List(selection: $model.selection) {
                    Label(tr("Движки Wine"), systemImage: "cpu").tag("engines")
                    Label(tr("Игровые лаунчеры"), systemImage: "gamecontroller").tag("launchers")
                    Section(tr("Окружения")) {
                        ForEach(model.bottles) { bottle in
                            HStack {
                                Image(systemName: "shippingbox").foregroundStyle(accent)
                                Text(bottle.name).lineLimit(1)
                                Spacer()
                                if bottle.status != "ready" { Image(systemName: "exclamationmark.circle").foregroundStyle(.orange) }
                            }.tag(bottle.id).contextMenu {
                                Button(tr("Delete environment…"), role: .destructive) { model.pendingDeletion = bottle }.disabled(model.busy)
                            }
                        }
                    }
                    Label(tr("Vela Trash"), systemImage: "trash").tag("trash")
                    Label(tr("App settings"), systemImage: "gearshape").tag("preferences")
                }.listStyle(.sidebar)
                VStack(alignment: .leading, spacing: 14) {
                    Button { creating = true } label: { Label(tr("Новое окружение"), systemImage: "plus").frame(maxWidth: .infinity) }
                        .buttonStyle(.borderedProminent).disabled(model.engines.isEmpty || model.busy)
                    Button { model.showingSupport = true } label: {
                        Label(tr("Поддержать Vela"), systemImage: "heart").frame(maxWidth: .infinity)
                    }.buttonStyle(.bordered)
                    Text(tr("Открытый код. Свободный выбор.")).font(.caption2).foregroundStyle(.tertiary)
                }.padding(20)
            }.navigationSplitViewColumnWidth(min: 220, ideal: 245, max: 300)
        } detail: {
            VStack(spacing: 0) {
                if model.loading {
                    ProgressView(tr("Загружаем библиотеку…")).frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let bottle = model.selectedBottle {
                    BottleView(bottle: bottle).id(bottle.id)
                } else if model.selection == "launchers" { LaunchersView() }
                else if model.selection == "trash" { TrashView() }
                else if model.selection == "preferences" { PreferencesView() }
                else { EnginesView() }
                if model.busy || model.notice != nil {
                    HStack(spacing: 10) {
                        if model.busy { ProgressView().controlSize(.small) }
                        else { Image(systemName: "info.circle").foregroundStyle(accent) }
                        Text(model.busy ? model.activity : (model.notice ?? "")).font(.callout).textSelection(.enabled)
                        if let progress = model.downloadProgress { ProgressView(value: progress).frame(width: 100) }
                        Spacer()
                        if model.canCancelDownload { Button(tr("Отменить загрузку")) { model.cancelDownload() } }
                        if !model.busy { Button { model.notice = nil } label: { Image(systemName: "xmark") }.buttonStyle(.plain) }
                    }.padding(14).background(.quaternary.opacity(0.45))
                }
            }
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .sheet(isPresented: $creating) { CreateBottleView() }
        .sheet(isPresented: $model.showingSupport) { SupportView() }
        .confirmationDialog(tr("Delete environment?"), isPresented: Binding(get: { model.pendingDeletion != nil }, set: { if !$0 { model.pendingDeletion = nil } }), titleVisibility: .visible, presenting: model.pendingDeletion) { bottle in
            Button(tr("Move to Vela Trash"), role: .destructive) {
                model.perform(tr("Deleting environment…"), request: ["op":"delete_environment", "id":bottle.id], notice: tr("Environment moved to Vela Trash. You can restore it there.")) {
                    model.selection = "trash"
                }
            }
            Button(tr("Отмена"), role: .cancel) {}
        } message: { bottle in
            Text(trf("Programs in “%@” will be stopped. Unsaved changes may be lost. The environment and its files will stay in Vela Trash until you restore or permanently delete it.", bottle.name))
        }
        .alert(tr("Не удалось выполнить действие"), isPresented: Binding(get: { model.error != nil }, set: { if !$0 { model.error = nil } })) {
            Button(tr("Понятно")) { model.error = nil }
        } message: { Text(model.error ?? "") }
    }
}

struct SupportView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var openError = false
    private let donationURL = SupportConfiguration.donationURL

    var body: some View {
        VStack(spacing: 20) {
            Image(nsImage: NSApplication.shared.applicationIconImage)
                .resizable().frame(width: 80, height: 80)
            Text(tr("Поддержите развитие Vela")).font(.title2.weight(.semibold))
            Text(tr("Ваша поддержка поможет исправлять ошибки, улучшать совместимость и выпускать обновления."))
                .multilineTextAlignment(.center).foregroundStyle(.secondary)
            if let donationURL {
                Text(trf("Support page: %@", donationURL.host ?? ""))
                    .font(.callout).foregroundStyle(.secondary)
            } else {
                Text(tr("Страница поддержки пока не подключена."))
                    .font(.callout).foregroundStyle(.secondary)
            }
            Button {
                guard let donationURL else { return }
                openError = !NSWorkspace.shared.open(donationURL)
            } label: {
                Label(tr("Открыть страницу поддержки"), systemImage: "arrow.up.right")
            }.buttonStyle(.borderedProminent).disabled(donationURL == nil)
            if openError {
                Text(tr("Не удалось открыть браузер. Попробуйте ещё раз.")).font(.caption).foregroundStyle(.red)
            }
            Text(tr("Поддержка добровольная. Пользоваться Vela можно бесплатно."))
                .font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
            Button(tr("Закрыть")) { dismiss() }.keyboardShortcut(.cancelAction)
        }.padding(32).frame(width: 440).tint(accent)
    }
}

struct EnginesView: View {
    @EnvironmentObject private var model: AppModel
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                HStack {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(tr("ВАША СРЕДА WINDOWS")).font(.caption).tracking(1.8).foregroundStyle(accent)
                        Text(tr("Один Mac. Больше возможностей.")).font(.system(size: 30, weight: .semibold, design: .rounded))
                        Text(tr("Выберите Wine, создайте окружение и запустите нужную программу."))
                            .font(.body).foregroundStyle(.secondary)
                    }
                    Spacer()
                }.padding(.top, 24)
                HStack(alignment: .top, spacing: 16) {
                    FeatureCard(number: "01", title: tr("Подключите Wine"), text: tr("Любая совместимая самостоятельная сборка для macOS."), icon: "cpu")
                    FeatureCard(number: "02", title: tr("Создайте окружение"), text: tr("Свой диск C:, реестр и настройки для каждой задачи."), icon: "shippingbox")
                    FeatureCard(number: "03", title: tr("Откройте программу"), text: tr("Запускайте EXE и MSI. Сохраняйте ярлыки в библиотеке."), icon: "play.rectangle")
                }
                HStack {
                    Text(tr("Движки Wine")).font(.title2.weight(.semibold))
                    Spacer()
                    Button { model.chooseEngine() } label: { Label(tr("Добавить движок"), systemImage: "plus") }.buttonStyle(.borderedProminent).disabled(model.busy)
                }
                if model.engines.isEmpty {
                    VStack(spacing: 12) {
                        Image(systemName: "cpu").font(.system(size: 34)).foregroundStyle(.secondary)
                        Text(tr("Начните с движка Wine")).font(.headline)
                        Text(tr("Для Apple Silicon выбирайте совместимый ARM64-движок.\nСборки Intel зависят от Rosetta; архитектура появится после подключения."))
                            .multilineTextAlignment(.center).foregroundStyle(.secondary)
                        Link(tr("Сборки Wine Intel (требуют Rosetta) ↗"), destination: URL(string: "https://github.com/Gcenx/macOS_Wine_builds/releases")!)
                    }.frame(maxWidth: .infinity).padding(32).background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 18))
                } else {
                    ForEach(model.engines) { engine in
                        HStack(spacing: 16) {
                            Image(systemName: "cpu.fill").font(.title).foregroundStyle(accent).frame(width: 48, height: 48).background(accent.opacity(0.1), in: RoundedRectangle(cornerRadius: 12))
                            VStack(alignment: .leading, spacing: 5) {
                                Text(engine.name).font(.headline)
                        Text(engine.version).font(.caption).foregroundStyle(.secondary)
                        Label(engine.compatibility, systemImage: engine.isNative ? "checkmark.shield" : "info.circle")
                            .font(.caption).foregroundStyle(engine.isNative ? .green : .orange)
                                Text(engine.wine).font(.caption2).foregroundStyle(.tertiary).lineLimit(2).textSelection(.enabled)
                            }
                            Spacer()
                            Label(tr("Подключён"), systemImage: "checkmark.circle.fill").font(.caption).foregroundStyle(.green)
                        }.padding(18).background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 16))
                    }
                }
                VStack(alignment: .leading, spacing: 10) {
                    Label(tr("Обновления под вашим контролем"), systemImage: "arrow.triangle.2.circlepath").font(.headline)
                    Text(tr("Добавляйте новые версии как отдельные движки. Существующие окружения продолжают использовать выбранную сборку. Автоматического скачивания и замены файлов в этой версии нет."))
                        .foregroundStyle(.secondary)
                    Text(tr("Для смены движка создайте новое окружение. Это сохраняет рабочую конфигурацию.")).font(.caption).foregroundStyle(.secondary)
                }.padding(20).frame(maxWidth: .infinity, alignment: .leading).background(accent.opacity(0.07), in: RoundedRectangle(cornerRadius: 16))
                HStack {
                    Text("Vela 0.4 · Preview · MIT").font(.caption).foregroundStyle(.tertiary)
                    Spacer()
                    Button(tr("Папка данных")) { model.showFolder(model.core.root.path) }.buttonStyle(.link)
                }
            }.padding(32)
        }
    }
}
struct FeatureCard: View {
    let number: String, title: String, text: String, icon: String
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack { Image(systemName: icon).font(.title2).foregroundStyle(accent); Spacer(); Text(number).font(.caption.monospaced()).foregroundStyle(.tertiary) }
            Text(title).font(.headline)
            Text(text).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }.padding(20).frame(maxWidth: .infinity, minHeight: 158, alignment: .topLeading)
            .background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 18))
    }
}

struct CreateBottleView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var engineID = ""
    @State private var dpi = 192
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            Label(tr("Новое окружение"), systemImage: "shippingbox").font(.title2.weight(.semibold))
            Text(tr("Каждое окружение хранит собственные программы, реестр и настройки Windows.")).foregroundStyle(.secondary)
            TextField(tr("Название, например «Работа»"), text: $name).textFieldStyle(.roundedBorder)
            Picker(tr("Движок Wine"), selection: $engineID) { ForEach(model.engines) { Text("\($0.name) · \($0.compatibility)").tag($0.id) } }
            Picker(tr("Масштаб Windows"), selection: $dpi) {
                ForEach([96, 120, 144, 168, 192, 240, 288], id: \.self) { value in Text("\(value * 100 / 96)%").tag(value) }
            }
            if model.engines.first(where: { $0.id == engineID })?.supports_x86_32 == false {
                Text(tr("Этот движок запускает 64-битные Windows-программы. Старые 32-битные приложения и установщики пока не поддерживаются.")).font(.caption).foregroundStyle(.secondary)
            }
            if model.engines.first(where: { $0.id == engineID })?.needsRosetta == true {
                Text(tr("Этот Intel-движок зависит от Rosetta. Для работы после прекращения её поддержки выберите ARM64 + FEX.")).font(.caption).foregroundStyle(.orange)
            }
            Text(tr("64-битное окружение. Запуск 32-битных программ зависит от поддержки WoW64 выбранной сборкой."))
                .font(.caption).foregroundStyle(.secondary)
            if model.busy { HStack { ProgressView().controlSize(.small); Text(tr("Wine создаёт диск C: и реестр. Это может занять до двух минут.")).font(.caption) } }
            HStack {
                Spacer()
                Button(tr("Отмена")) { dismiss() }.disabled(model.busy)
                Button(tr("Создать")) {
                    model.perform(tr("Создаём окружение…"), request: ["op":"create", "name":name, "engine_id":engineID, "dpi":dpi], notice: tr("Окружение готово.")) {
                        model.selection = model.bottles.last?.id
                        dismiss()
                    }
                }.buttonStyle(.borderedProminent).disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || engineID.isEmpty || model.busy)
                    .keyboardShortcut(.defaultAction)
            }
        }.padding(28).frame(width: 490).onAppear {
            engineID = model.engines.first?.id ?? ""
            dpi = (NSScreen.main?.backingScaleFactor ?? 1) >= 2 ? 192 : 96
        }.interactiveDismissDisabled(model.busy)
    }
}

struct BottleView: View {
    @EnvironmentObject private var model: AppModel
    let bottle: Bottle
    @State private var tab = "library"
    @State private var addingShortcut = false
    @State private var confirmingStop = false
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top, spacing: 16) {
                Image(systemName: "shippingbox.fill").font(.system(size: 32)).foregroundStyle(accent)
                    .frame(width: 66, height: 66).background(accent.opacity(0.1), in: RoundedRectangle(cornerRadius: 18))
                VStack(alignment: .leading, spacing: 7) {
                    Text(bottle.name).font(.system(size: 30, weight: .semibold, design: .rounded))
                    Text(model.engines.first { $0.id == bottle.engine_id }?.version ?? tr("Движок недоступен")).font(.callout).foregroundStyle(.secondary)
                    if let engine = model.engines.first(where: { $0.id == bottle.engine_id }) {
                        Text(engine.compatibility).font(.caption).foregroundStyle(engine.isNative ? .green : .orange)
                    }
                    Text(bottle.status == "ready" ? tr("Окружение готово · 64-bit") : tr("Инициализация не завершена"))
                        .font(.caption).foregroundStyle(bottle.status == "ready" ? .green : .orange)
                }
                Spacer()
                Button { model.chooseProgram(for: bottle) } label: { Label(tr("Открыть EXE / MSI"), systemImage: "play.fill") }
                    .buttonStyle(.borderedProminent).controlSize(.large).disabled(model.busy || bottle.status != "ready")
            }.padding(.top, 24)
            if let installation = bottle.installation {
                VStack(alignment: .leading, spacing: 10) {
                    Label(installation.status == "detected" ? tr("Лаунчер найден · ярлык добавлен") : tr("Ожидаем файлы лаунчера"), systemImage: installation.status == "detected" ? "checkmark.circle" : "arrow.down.circle")
                        .foregroundStyle(installation.status == "detected" ? .green : accent)
                    Text(installation.status == "detected" ? tr("Перед первым запуском дождитесь завершения мастера установки.") : tr("Завершите установку в окне Windows. Если выбрана другая папка, добавьте ярлык вручную."))
                        .font(.caption).foregroundStyle(.secondary)
                    HStack {
                        Button(tr("Проверить установку")) { Task { await model.refresh() } }.disabled(model.busy)
                        if installation.status != "detected" {
                            Button(tr("Открыть установщик снова")) { model.perform(tr("Открываем установщик…"), request: ["op":"retry_launcher", "id":bottle.id]) }.disabled(model.busy)
                        }
                    }
                }.padding(16).frame(maxWidth: .infinity, alignment: .leading).background(accent.opacity(0.07), in: RoundedRectangle(cornerRadius: 14))
            }
            Picker(tr("Раздел"), selection: $tab) {
                Text(tr("Программы")).tag("library")
                Text(tr("Настройки")).tag("settings")
                Text(tr("Журнал")).tag("log")
            }.pickerStyle(.segmented).frame(maxWidth: 410)
            if bottle.status != "ready" {
                HStack {
                    Label(tr("Откройте журнал, чтобы узнать причину сбоя."), systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
                    Spacer()
                    Button(tr("Повторить")) { model.perform(tr("Инициализируем Wine…"), request: ["op":"initialize", "id":bottle.id]) }.disabled(model.busy)
                }.font(.callout)
            }
            if tab == "library" { library }
            else if tab == "settings" { BottleSettingsView(bottle: bottle) }
            else { LogView(bottle: bottle) }
        }.padding(32)
            .sheet(isPresented: $addingShortcut) { AddShortcutView(bottle: bottle) }
            .alert(trf("Stop applications in “%@”?", bottle.name), isPresented: $confirmingStop) {
                Button(tr("Отмена"), role: .cancel) {}
                Button(tr("Остановить"), role: .destructive) { model.perform(tr("Останавливаем Wine…"), request: ["op":"stop", "id":bottle.id], notice: tr("Команда остановки отправлена этому окружению.")) }
            } message: { Text(tr("Несохранённые изменения в программах этого окружения могут быть потеряны.")) }
    }
    var library: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack {
                    Text(tr("Ваши программы")).font(.title3.weight(.semibold))
                    Spacer()
                    Button { addingShortcut = true } label: { Label(tr("Добавить ярлык"), systemImage: "plus") }.disabled(model.busy || bottle.status != "ready")
                }
                if bottle.shortcuts.isEmpty {
                    VStack(spacing: 12) {
                        Image(systemName: "app.dashed").font(.system(size: 40)).foregroundStyle(.secondary)
                        Text(tr("Здесь будут ваши программы")).font(.headline)
                        Text(tr("Откройте установщик, затем добавьте ярлык\nна установленный EXE с диска C:.")).multilineTextAlignment(.center).foregroundStyle(.secondary)
                        Button(tr("Открыть диск C:")) { model.showFolder(model.prefix(bottle) + "/drive_c") }.buttonStyle(.link)
                    }.frame(maxWidth: .infinity).padding(36).background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 18))
                }
                ForEach(bottle.shortcuts) { shortcut in
                    HStack(spacing: 14) {
                        Image(systemName: "app.fill").font(.title).foregroundStyle(accent)
                        VStack(alignment: .leading, spacing: 5) { Text(shortcut.name).font(.headline); Text(shortcut.executable).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle) }
                        Spacer()
                        Button { model.perform(tr("Передаём программу Wine…"), request: ["op":"run_shortcut", "id":bottle.id, "shortcut_id":shortcut.id], notice: tr("Запуск передан Wine. Результат доступен в журнале.")) } label: { Image(systemName: "play.fill") }.disabled(model.busy)
                        Menu {
                            Button(tr("Показать файл")) { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: shortcut.executable)]) }
                            Button(tr("Убрать ярлык"), role: .destructive) { model.perform(tr("Убираем ярлык…"), request: ["op":"remove_shortcut", "id":bottle.id, "shortcut_id":shortcut.id]) }
                        } label: { Image(systemName: "ellipsis").padding(.horizontal, 4) }
                            .menuStyle(.borderlessButton)
                            .frame(width: 40, height: 32)
                            .padding(.trailing, 8)
                            .disabled(model.busy)
                    }.padding(18).background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 14))
                }
                Text(tr("Инструменты окружения")).font(.headline).padding(.top, 10)
                HStack(spacing: 12) {
                    tool(tr("Настройки Wine"), icon: "slider.horizontal.3", name: "winecfg")
                    tool(tr("Реестр"), icon: "list.bullet.rectangle", name: "regedit")
                    tool(tr("Консоль"), icon: "terminal", name: "cmd")
                    tool(tr("Удаление программ"), icon: "app.badge", name: "uninstaller")
                }
                HStack {
                    Button { model.showFolder(model.prefix(bottle) + "/drive_c") } label: { Label(tr("Диск C:"), systemImage: "folder") }
                    Spacer()
                    Button { confirmingStop = true } label: { Label(tr("Остановить"), systemImage: "stop.circle") }.foregroundStyle(.orange).disabled(model.busy)
                }.padding(.top, 10)
                Text(tr("Окружения разделяют настройки Windows. Программы Wine имеют доступ к файлам Mac с правами вашего пользователя."))
                    .font(.caption).foregroundStyle(.secondary).padding(.top, 6)
            }
        }
    }
    func tool(_ label: String, icon: String, name: String) -> some View {
        Button { model.tool(name, bottle: bottle) } label: {
            VStack(alignment: .leading, spacing: 12) {
                Image(systemName: icon).font(.title2).foregroundStyle(accent)
                Text(label).font(.callout)
            }.frame(maxWidth: .infinity, minHeight: 70, alignment: .leading).padding(14)
                .background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 14))
        }.buttonStyle(.plain).disabled(model.busy || bottle.status != "ready")
    }
}

struct BottleSettingsView: View {
    @EnvironmentObject private var model: AppModel
    let bottle: Bottle
    @State private var name = ""
    @State private var environment = ""
    @State private var dpi = 96
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text(tr("Общие")).font(.headline)
                TextField(tr("Название окружения"), text: $name).textFieldStyle(.roundedBorder)
                Text(tr("Размер интерфейса Windows")).font(.headline).padding(.top, 8)
                HStack {
                    Picker(tr("Масштаб"), selection: $dpi) {
                        ForEach([96, 120, 144, 168, 192, 240, 288], id: \.self) { value in Text("\(value * 100 / 96)%").tag(value) }
                    }.frame(maxWidth: 260)
                    Button(tr("Применить масштаб")) {
                        model.perform(tr("Меняем масштаб Windows…"), request: ["op":"set_dpi", "id":bottle.id, "dpi":dpi], notice: tr("Масштаб сохранён. Перезапустите Windows-программы, чтобы применить его."))
                    }.disabled(model.busy || bottle.status != "ready")
                }
                Text(tr("Для Retina обычно удобно 200%. Новый масштаб действует после перезапуска Windows-программ.")).font(.caption).foregroundStyle(.secondary)
                Text(tr("Версия Windows, звук и отображение")).font(.headline).padding(.top, 8)
                Button(tr("Открыть настройки Wine")) { model.tool("winecfg", bottle: bottle) }.disabled(model.busy || bottle.status != "ready")
                Text(tr("Изменения применяются в этом окружении. Для графических библиотек нужна совместимая сборка движка.")).font(.caption).foregroundStyle(.secondary)
                Text(tr("Переменные окружения")).font(.headline).padding(.top, 8)
                Text(tr("По одной переменной KEY=VALUE в строке. Они применятся при следующем запуске.")).font(.callout).foregroundStyle(.secondary)
                TextEditor(text: $environment).font(.system(.body, design: .monospaced)).frame(minHeight: 180).padding(8)
                    .background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 10))
                Text(tr("Примеры: WINEDEBUG=+seh, WINEDLLOVERRIDES=d3d11=n,b. Переменные пути к окружению и движку задаёт Vela.")).font(.caption).foregroundStyle(.secondary)
                HStack {
                    Button(tr("Сохранить")) { save() }.buttonStyle(.borderedProminent).disabled(model.busy)
                    Spacer()
                    Button(tr("Показать папку окружения")) { model.showFolder(model.prefix(bottle)) }
                }
                Divider().padding(.top, 12)
                Text(tr("Delete environment")).font(.headline)
                Text(tr("Stop its Windows programs and move the environment to Vela Trash. You can restore it or permanently delete its files there."))
                    .font(.caption).foregroundStyle(.secondary)
                Button(tr("Delete environment…"), role: .destructive) { model.pendingDeletion = bottle }.disabled(model.busy)
            }.padding(.top, 4)
        }.onAppear {
            name = bottle.name
            dpi = bottle.dpi ?? 96
            environment = bottle.environment.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" }.joined(separator: "\n")
        }
    }
    func save() {
        var values: [String: String] = [:]
        for line in environment.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { continue }
            guard let split = trimmed.firstIndex(of: "=") else { model.error = trf("Expected KEY=VALUE: %@", trimmed); return }
            let key = String(trimmed[..<split]).trimmingCharacters(in: .whitespaces)
            if values[key] != nil { model.error = trf("Variable %@ is specified twice", key); return }
            values[key] = String(trimmed[trimmed.index(after: split)...])
        }
        model.perform(tr("Сохраняем настройки…"), request: ["op":"save_settings", "id":bottle.id, "name":name, "environment":values], notice: tr("Настройки сохранены. Применятся при следующем запуске."))
    }
}

struct LogView: View {
    @EnvironmentObject private var model: AppModel
    let bottle: Bottle
    @State private var text = ""
    @State private var path: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text(tr("Последняя операция · до 128 КБ")).font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button(tr("Все журналы")) { model.showFolder(model.core.root.appendingPathComponent("logs").path) }
                Button { Task { await reload() } } label: { Label(tr("Обновить"), systemImage: "arrow.clockwise") }
            }
            ScrollView([.vertical, .horizontal]) {
                Text(text.isEmpty ? tr("Загрузка…") : text).font(.system(size: 12, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .topLeading).padding(14)
            }.background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
            if let path { Text(path).font(.caption2).foregroundStyle(.secondary).textSelection(.enabled) }
        }.task { await reload() }.task(id: bottle.last_log) { await reload() }
    }
    func reload() async {
        do { let log = try await model.core.call(["op":"read_log", "id":bottle.id], as: LogReply.self); text = log.text; path = log.path }
        catch { model.error = error.localizedDescription }
    }
}

struct AddShortcutView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    let bottle: Bottle
    @State private var name = ""
    @State private var path = ""
    @State private var arguments = ""
    @State private var directory = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text(tr("Добавить ярлык")).font(.title2.weight(.semibold))
            TextField(tr("Название программы"), text: $name).textFieldStyle(.roundedBorder)
            HStack { TextField(tr("Путь к программе"), text: $path).textFieldStyle(.roundedBorder); Button(tr("Выбрать…")) { chooseFile() } }
            Text(tr("Аргументы запуска · каждый с новой строки")).font(.callout)
            TextEditor(text: $arguments).font(.system(.body, design: .monospaced)).frame(height: 90).border(.quaternary)
            Text(tr("Аргументы передаются буквально. macOS shell не используется.")).font(.caption).foregroundStyle(.secondary)
            HStack {
                TextField(tr("Рабочая папка · по умолчанию рядом с EXE"), text: $directory).textFieldStyle(.roundedBorder)
                Button(tr("Выбрать…")) { chooseDirectory() }
            }
            HStack {
                Spacer(); Button(tr("Отмена")) { dismiss() }
                Button(tr("Добавить")) {
                    var request: [String: Any] = ["op":"add_shortcut", "id":bottle.id, "name":name, "path":path,
                        "arguments":arguments.components(separatedBy: "\n").filter { !$0.isEmpty }]
                    if !directory.isEmpty { request["directory"] = directory }
                    model.perform(tr("Добавляем ярлык…"), request: request) { dismiss() }
                }.buttonStyle(.borderedProminent).disabled(name.isEmpty || path.isEmpty || model.busy).keyboardShortcut(.defaultAction)
            }
        }.padding(28).frame(width: 560).interactiveDismissDisabled(model.busy)
    }
    func chooseFile() {
        let p = NSOpenPanel(); p.title = tr("Выберите программу"); p.directoryURL = URL(fileURLWithPath: model.prefix(bottle) + "/drive_c")
        p.allowedContentTypes = ["exe", "msi", "bat", "cmd"].compactMap { UTType(filenameExtension: $0) }
        if p.runModal() == .OK, let url = p.url { path = url.path; if name.isEmpty { name = url.deletingPathExtension().lastPathComponent } }
    }
    func chooseDirectory() {
        let p = NSOpenPanel(); p.canChooseDirectories = true; p.canChooseFiles = false
        if p.runModal() == .OK, let url = p.url { directory = url.path }
    }
}
