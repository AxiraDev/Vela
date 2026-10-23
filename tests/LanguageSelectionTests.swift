import Foundation

@main struct LocalizationCheck {
    static func main() throws {
        precondition(L10n.resolve(selection:"system",preferred:["ru-RU","en-US"]) == "ru")
        precondition(L10n.resolve(selection:"system",preferred:["en-GB","ru"]) == "en")
        precondition(L10n.resolve(selection:"system",preferred:["fr-FR"]) == "en")
        precondition(L10n.resolve(selection:"system",preferred:["de-DE","ru_RU"]) == "ru")
        precondition(L10n.resolve(selection:"en",preferred:["ru-RU"]) == "en")
        precondition(L10n.resolve(selection:"ru",preferred:["en-US"]) == "ru")
        guard CommandLine.arguments.count == 2 else { fatalError("Pass macos/Resources as the argument") }
        let root = URL(fileURLWithPath:CommandLine.arguments[1])
        func strings(_ code:String) throws -> [String:String] {
            let data = try Data(contentsOf:root.appendingPathComponent("\(code).lproj/Localizable.strings"))
            return try PropertyListSerialization.propertyList(from:data,format:nil) as! [String:String]
        }
        let en = try strings("en"),ru = try strings("ru")
        precondition(Set(en.keys)==Set(ru.keys))
        for key in en.keys {
            let pattern = #"%(?:\d+\$)?(?:@|ld)"#
            let regex = try NSRegularExpression(pattern:pattern)
            func formats(_ s:String)->[String] { regex.matches(in:s,range:NSRange(s.startIndex...,in:s)).map { String(s[Range($0.range,in:s)!]) } }
            precondition(formats(en[key]!) == formats(ru[key]!), "Format mismatch: \(key)")
        }
        precondition(en["Удаление окружения"] == nil) // User content is not translated by matching names.
        precondition(en["Delete environment…"] == "Delete environment…")
        precondition(ru["Delete environment…"] == "Удалить окружение…")
        print("PASS language negotiation, explicit overrides, resource parity and format arguments (\(en.count) keys)")
    }
}
