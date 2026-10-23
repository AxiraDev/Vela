import SwiftUI

struct EngineBackup: Codable, Sendable {
    let id: String
    let engine_name: String
}

struct EnvironmentEngineSection: View {
    @EnvironmentObject private var model: AppModel
    let bottle: Bottle
    @State private var changingEngine = false
    @State private var restoringBackup = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(tr("Движок Wine")).font(.headline)
            if let engine = model.engines.first(where: { $0.id == bottle.engine_id }) {
                Text(engine.name).font(.callout.weight(.semibold))
                EngineSupportView(engine: engine)
            }
            Button(tr("Change Wine engine…")) { changingEngine = true }
                .disabled(model.busy || bottle.status != "ready" || model.engines.count < 2)
            Text(tr("Change the engine without reinstalling your programs. Vela stops this environment, saves a backup, and checks the new engine before applying the change."))
                .font(.caption).foregroundStyle(.secondary)
            if bottle.pending_engine != nil {
                Label(tr("The engine change was interrupted. Restore the backup before launching programs."), systemImage: "exclamationmark.triangle")
                    .font(.callout).foregroundStyle(.orange)
            }
            if let backup = bottle.engine_backup {
                Text(trf("Backup before engine change: %@", backup.engine_name)).font(.caption).foregroundStyle(.secondary)
                HStack {
                    Button(tr("Restore before engine change…")) { restoringBackup = true }.disabled(model.busy)
                    Button(tr("Show engine backups")) {
                        model.showFolder(model.core.root.appendingPathComponent("engine-backups/\(bottle.id)").path)
                    }
                }
            }
        }
        .sheet(isPresented: $changingEngine) { ChangeEngineView(bottle: bottle) }
        .alert(tr("Restore before engine change?"), isPresented: $restoringBackup) {
            Button(tr("Отмена"), role: .cancel) {}
            Button(tr("Restore backup"), role: .destructive) {
                model.perform(tr("Restoring the environment…"), request: ["op":"restore_engine_backup", "id":bottle.id], notice: tr("The environment and its previous engine have been restored."))
            }
        } message: {
            Text(tr("Windows programs in this environment will close. Its files, registry, shortcuts, and saved settings will return to the backup state. Current prefix files will be kept in the backup folder. Files outside the environment are not backed up."))
        }
    }
}

struct ChangeEngineView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    let bottle: Bottle
    @State private var engineID = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text(tr("Change Wine engine")).font(.title2.weight(.semibold))
            Text(bottle.name).font(.headline)
            EnginePickerView(engines: model.engines, selection: $engineID)
            if model.engines.first(where: { $0.id == engineID })?.supports_x86_32 == false {
                Label(tr("32-bit programs will not run with this engine, even if they are already installed."), systemImage: "exclamationmark.triangle")
                    .font(.callout).foregroundStyle(.orange)
            }
            Text(tr("Programs in this environment will close; save your work first. Installed programs and files stay in place. A backup is created before Wine updates the environment. If the check fails, Vela attempts to restore it automatically."))
                .font(.callout).foregroundStyle(.secondary)
            Text(tr("Backups use disk space and remain until you remove them or permanently delete the environment. Compatibility with every installed program is not guaranteed."))
                .font(.caption).foregroundStyle(.secondary)
            if model.busy {
                HStack { ProgressView().controlSize(.small); Text(tr("Backing up and checking the new engine…")).font(.caption) }
            }
            HStack {
                Spacer()
                Button(tr("Отмена")) { dismiss() }.disabled(model.busy)
                Button(tr("Change engine")) {
                    model.perform(tr("Changing Wine engine…"), request: ["op":"change_engine", "id":bottle.id, "engine_id":engineID], notice: tr("Engine changed. Your programs, files, and shortcuts have been kept.")) { dismiss() }
                }.buttonStyle(.borderedProminent).disabled(model.busy || engineID.isEmpty || engineID == bottle.engine_id)
            }
        }.padding(28).frame(width: 600)
            .onAppear { engineID = bottle.engine_id }
            .interactiveDismissDisabled(model.busy)
    }
}
