import Foundation
struct SourceLineMap {
    let starts: [Int]
    init(_ text: String = "") {
        var offsets = [0]
        for (index, unit) in text.utf16.enumerated() where unit == 10 { offsets.append(index + 1) }
        starts = offsets
    }
    func position(at offset: Int) -> (line: Int, column: Int) {
        var low = 0, high = starts.count
        while low < high { let mid = (low + high) / 2; if starts[mid] <= offset { low = mid + 1 } else { high = mid } }
        let index = max(0, low - 1)
        return (index + 1, max(0, offset - starts[index]) + 1)
    }
    func offset(line: Int) -> Int { starts[min(starts.count - 1, max(0, line - 1))] }
}
