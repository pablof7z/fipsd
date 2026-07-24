import Foundation

enum JSONValue: Codable, Equatable, Sendable {
    case object([String: JSONValue])
    case array([JSONValue])
    case string(String)
    case integer(Int64)
    case number(Double)
    case bool(Bool)
    case null

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let item = try? value.decode(Bool.self) { self = .bool(item) }
        else if let item = try? value.decode(Int64.self) { self = .integer(item) }
        else if let item = try? value.decode(Double.self) { self = .number(item) }
        else if let item = try? value.decode(String.self) { self = .string(item) }
        else if let item = try? value.decode([JSONValue].self) { self = .array(item) }
        else { self = .object(try value.decode([String: JSONValue].self)) }
    }

    func encode(to encoder: Encoder) throws {
        var value = encoder.singleValueContainer()
        switch self {
        case let .object(item): try value.encode(item)
        case let .array(item): try value.encode(item)
        case let .string(item): try value.encode(item)
        case let .integer(item): try value.encode(item)
        case let .number(item): try value.encode(item)
        case let .bool(item): try value.encode(item)
        case .null: try value.encodeNil()
        }
    }

    var object: [String: JSONValue]? {
        if case let .object(value) = self { value } else { nil }
    }

    var array: [JSONValue]? {
        if case let .array(value) = self { value } else { nil }
    }

    var string: String? {
        if case let .string(value) = self { value } else { nil }
    }

    var int: Int? {
        switch self {
        case let .integer(value): Int(exactly: value)
        case let .number(value): Int(exactly: value)
        default: nil
        }
    }

    var uint64: UInt64? {
        switch self {
        case let .integer(value): UInt64(exactly: value)
        case let .number(value): UInt64(exactly: value)
        default: nil
        }
    }

    var bool: Bool? {
        if case let .bool(value) = self { value } else { nil }
    }

    var prettyDescription: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        guard let data = try? encoder.encode(self) else { return String(describing: self) }
        return String(decoding: data, as: UTF8.self)
    }
}
