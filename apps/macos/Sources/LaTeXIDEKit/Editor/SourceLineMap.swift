import Foundation
struct SourceLineMap {
    private(set) var starts: [Int]
    private var length: Int
    init(_ text: String = "") {
        var offsets = [0]
        for (index, unit) in text.utf16.enumerated() where unit == 10 { offsets.append(index + 1) }
        starts = offsets
        length = text.utf16.count
    }
    /// Update UTF-16 line offsets after an already validated native edit.
    /// Same-length edits without line breaks leave the index untouched.
    mutating func replace(range: NSRange, with replacement: String) -> Bool {
        guard range.location >= 0, range.location <= length,
              range.length >= 0, range.length <= length - range.location else { return false }
        let added = SourceLineMap(replacement)
        let delta = added.length - range.length
        func upperBound(_ offset: Int) -> Int {
            var low = 0, high = starts.count
            while low < high {
                let mid = (low + high) / 2
                if starts[mid] <= offset { low = mid + 1 } else { high = mid }
            }
            return low
        }
        let first = upperBound(range.location), end = upperBound(NSMaxRange(range))
        if first != end || added.starts.count > 1 {
            starts.replaceSubrange(first..<end, with: added.starts.dropFirst().map { range.location + $0 })
        }
        if delta != 0 {
            let suffix = first + added.starts.count - 1
            for index in suffix..<starts.count { starts[index] += delta }
        }
        length += delta
        return true
    }
    func position(at offset: Int) -> (line: Int, column: Int) {
        var low = 0, high = starts.count
        while low < high { let mid = (low + high) / 2; if starts[mid] <= offset { low = mid + 1 } else { high = mid } }
        let index = max(0, low - 1)
        return (index + 1, max(0, offset - starts[index]) + 1)
    }
    func offset(line: Int) -> Int { starts[min(starts.count - 1, max(0, line - 1))] }
}
