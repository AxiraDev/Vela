import SwiftUI

struct PreferencesView: View {
    @AppStorage(L10n.preferenceKey) private var language = "system"
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Label(tr("App settings"), systemImage: "gearshape").font(.title2.weight(.semibold))
            Picker(tr("Interface language"), selection: $language) {
                Text(tr("System default")).tag("system")
                Text("English").tag("en")
                Text("Русский").tag("ru")
            }.pickerStyle(.menu)
            Text(tr("System default follows your preferred device languages. English is used when none of them is supported."))
                .font(.callout).foregroundStyle(.secondary)
            Text(tr("The interface changes immediately. Wine and Windows applications have their own language settings."))
                .font(.caption).foregroundStyle(.secondary)
        }.padding(32).frame(maxWidth: 560, alignment: .leading).frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct TrashView: View {
    @EnvironmentObject private var model: AppModel
    @State private var purging: Bottle?
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                Label(tr("Vela Trash"), systemImage: "trash").font(.largeTitle.weight(.semibold))
                Text(tr("Deleted environments can be restored here. Their files still occupy disk space until you delete them permanently."))
                    .foregroundStyle(.secondary)
                if model.deletedBottles.isEmpty {
                    ContentUnavailableView(tr("Trash is empty"), systemImage: "trash")
                }
                ForEach(model.deletedBottles) { bottle in
                    VStack(alignment: .leading, spacing: 12) {
                        Text(bottle.name).font(.headline)
                        Text(model.prefix(bottle)).font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                        HStack {
                            Button(tr("Restore")) {
                                model.perform(tr("Restoring environment…"), request: ["op":"restore_environment", "id":bottle.id], notice: tr("Environment restored.")) {
                                    model.selection = bottle.id
                                }
                            }.buttonStyle(.borderedProminent).disabled(model.busy)
                            Spacer()
                            Button(tr("Delete permanently…"), role: .destructive) { purging = bottle }.disabled(model.busy)
                        }
                    }.padding(20).background(.quaternary.opacity(0.3), in: RoundedRectangle(cornerRadius: 16))
                }
            }.padding(32)
        }.confirmationDialog(tr("Delete permanently?"), isPresented: Binding(get: { purging != nil }, set: { if !$0 { purging = nil } }), titleVisibility: .visible, presenting: purging) { bottle in
            Button(tr("Delete permanently"), role: .destructive) {
                model.perform(tr("Deleting environment files…"), request: ["op":"purge_environment", "id":bottle.id], notice: tr("Environment permanently deleted."))
            }
            Button(tr("Отмена"), role: .cancel) {}
        } message: { bottle in
            Text(trf("“%@” and all its Windows applications, files, saved data, engine backups, cached installers, and logs will be deleted. This cannot be undone. Files outside this environment and Wine engines are kept.", bottle.name))
        }
    }
}
