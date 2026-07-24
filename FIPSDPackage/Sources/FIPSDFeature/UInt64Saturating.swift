extension UInt64 {
    func saturatingAdd(_ value: UInt64) -> UInt64 {
        let (result, overflow) = addingReportingOverflow(value)
        return overflow ? .max : result
    }

    func saturatingSubtract(_ value: UInt64) -> UInt64 {
        self > value ? self - value : 0
    }
}
