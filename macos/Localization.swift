import Foundation

enum L10n {
    static let preferenceKey = "VelaInterfaceLanguage"
    static func resolve(selection: String, preferred: [String]) -> String {
        if ["en", "ru"].contains(selection) { return selection }
        for identifier in preferred {
            let language = identifier.lowercased().split(whereSeparator: { $0 == "-" || $0 == "_" }).first.map(String.init)
            if let language, ["en", "ru"].contains(language) { return language }
        }
        return "en"
    }
    static var code: String {
        resolve(selection: UserDefaults.standard.string(forKey: preferenceKey) ?? "system", preferred: Locale.preferredLanguages)
    }
    static func bundle(code: String, root: Bundle = .main) -> Bundle {
        guard let path = root.path(forResource: code, ofType: "lproj"), let bundle = Bundle(path: path) else { return root }
        return bundle
    }
    static func text(_ key: String) -> String { bundle(code: code).localizedString(forKey: key, value: key, table: nil) }
}

func tr(_ key: String) -> String { L10n.text(key) }
func trf(_ key: String, _ arguments: CVarArg...) -> String {
    String(format: tr(key), locale: Locale(identifier: L10n.code), arguments: arguments)
}
