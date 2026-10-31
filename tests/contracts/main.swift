import Foundation

func require(_ condition: @autoclosure () -> Bool, _ message: String) {
    if !condition() { fatalError(message) }
}

let vectors = URL(fileURLWithPath: CommandLine.arguments[1])

func check<Request: Codable, Response: Codable>(_ id: String, _: Request.Type, _: Response.Type) throws -> NativeEnvelope<Request> {
    let data = try Data(contentsOf: vectors.appendingPathComponent(id.lowercased() + ".json"))
    let file = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    let cases = file["cases"] as! [[String: Any]]
    let sample = cases.first { $0["name"] as? String == "roundtrip" }!
    let requestData = try JSONSerialization.data(withJSONObject: sample["request"]!)
    let request = try JSONDecoder().decode(NativeEnvelope<Request>.self, from: requestData)
    require(request.package_id == id, "wrong native package")
    let requestCopy = try JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as! NSDictionary
    require(requestCopy.isEqual(to: sample["request"] as! [AnyHashable: Any]), "native request changed shape: \(id)")
    let responseData = try JSONSerialization.data(withJSONObject: sample["response_example"]!)
    let response = try JSONDecoder().decode(Response.self, from: responseData)
    let responseCopy = try JSONSerialization.jsonObject(with: JSONEncoder().encode(response)) as! NSDictionary
    require(responseCopy.isEqual(to: sample["response_example"] as! [AnyHashable: Any]), "native response changed shape: \(id)")
    let future = cases.first { $0["name"] as? String == "future_version" }!
    do {
        _ = try JSONDecoder().decode(NativeEnvelope<Request>.self, from: JSONSerialization.data(withJSONObject: future["request"]!))
        fatalError("future contract version accepted")
    } catch NativeContractError.unsupportedVersion { }
    return request
}

do {
    _ = try check("P06", SearchRequest.self, SearchPage.self)
    let editor = try check("P07", EditorRequest.self, EditorTransaction.self)
    require(editor.payload.snapshot.files[0].revision == 9_007_199_254_740_993, "u64 revision rounded")
    require(editor.payload.action.revision == 9_007_199_254_740_993, "action revision rounded")
    require(String(bytes: editor.payload.snapshot.files[0].utf8, encoding: .utf8) == "A😀\né", "UTF-8 payload changed")
    let editorVectors = try String(contentsOf: vectors.appendingPathComponent("p07.json"), encoding: .utf8)
    let maximum = editorVectors.replacingOccurrences(of: "9007199254740993", with: "18446744073709551615")
    let maximumFile = try JSONSerialization.jsonObject(with: Data(maximum.utf8)) as! [String: Any]
    let maximumCase = (maximumFile["cases"] as! [[String: Any]])[0]
    let maximumRequest = try JSONDecoder().decode(NativeEnvelope<EditorRequest>.self, from: JSONSerialization.data(withJSONObject: maximumCase["request"]!))
    require(maximumRequest.payload.action.revision == UInt64.max, "maximum u64 rounded")
    require(maximumRequest.payload.snapshot.files[0].revision == UInt64.max, "maximum snapshot revision rounded")
    _ = try check("P08", SettingsRequest.self, EffectiveSettings.self)
    _ = try check("P10", SyncRequest.self, PdfLocation.self)
    _ = try check("P14", AssistantRequest.self, AssistantState.self)
    print("Native schema PASS: P06/P07/P08/P10/P14; u64, UTF-8, shape and future-version rejection; no UI/provider executed")
} catch {
    fputs("Native schema FAIL: \(error)\n", stderr)
    exit(1)
}
