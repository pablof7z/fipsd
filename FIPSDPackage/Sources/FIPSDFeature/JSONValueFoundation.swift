import Foundation

extension JSONValue {
    var foundationValue: Any {
        switch self {
        case let .object(value):
            value.mapValues(\.foundationValue)
        case let .array(value):
            value.map(\.foundationValue)
        case let .string(value):
            value
        case let .integer(value):
            value
        case let .number(value):
            value
        case let .bool(value):
            value
        case .null:
            NSNull()
        }
    }

    var double: Double? {
        switch self {
        case let .integer(value): Double(value)
        case let .number(value): value
        default: nil
        }
    }
}
