/**
 * Minimal protobuf decoder for StateChange messages.
 *
 * StateChange wire format (from operation.proto):
 *   1: string plugin_id
 *   2: string member_name
 *   3: ChangeType change_type (enum/varint)
 *   5: google.protobuf.Value new_value
 *   8: string schema_hash
 *  13: StateFrameKind frame_kind (enum/varint)
 *
 * We only need plugin_id, member_name, and new_value for the state store.
 * google.protobuf.Value is decoded into native JS values.
 */

export interface DecodedStateChange {
  pluginId: string;
  memberName: string | undefined;
  newValue: unknown;
  frameKind: number;
}

export function decodeStateChange(buf: Uint8Array): DecodedStateChange | null {
  try {
    const reader = new ProtoReader(buf);
    let pluginId = "";
    let memberName: string | undefined;
    let newValue: unknown;
    let frameKind = 0;

    while (reader.hasMore()) {
      const tag = reader.readTag();
      const fieldNumber = tag >>> 3;
      const wireType = tag & 0x07;

      switch (fieldNumber) {
        case 1: pluginId = reader.readString(); break;
        case 2: memberName = reader.readString() || undefined; break;
        case 3: frameKind = reader.readVarint(); break; // ChangeType
        case 5: newValue = decodeValue(reader.readBytes()); break;
        case 13: frameKind = reader.readVarint(); break; // StateFrameKind
        default: reader.skip(wireType); break;
      }
    }

    // frameKind 3 = HEARTBEAT — skip
    if (frameKind === 3) return null;
    if (!pluginId) return null;

    return { pluginId, memberName, newValue, frameKind };
  } catch {
    return null;
  }
}

/** Decode google.protobuf.Value → JS value */
function decodeValue(buf: Uint8Array): unknown {
  if (buf.length === 0) return null;
  const reader = new ProtoReader(buf);
  while (reader.hasMore()) {
    const tag = reader.readTag();
    const fieldNumber = tag >>> 3;
    switch (fieldNumber) {
      case 1: return null; // null_value
      case 2: return reader.readDouble(); // number_value
      case 3: return reader.readString(); // string_value
      case 4: return reader.readVarint() !== 0; // bool_value
      case 5: return decodeStruct(reader.readBytes()); // struct_value
      case 6: return decodeListValue(reader.readBytes()); // list_value
      default: reader.skip(tag & 0x07); break;
    }
  }
  return null;
}

/** Decode google.protobuf.Struct → Record<string, unknown> */
function decodeStruct(buf: Uint8Array): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  const reader = new ProtoReader(buf);
  while (reader.hasMore()) {
    const tag = reader.readTag();
    if ((tag >>> 3) === 1) {
      // fields: map<string, Value> — each entry is a length-delimited sub-message
      const entryBuf = reader.readBytes();
      const entryReader = new ProtoReader(entryBuf);
      let key = "";
      let value: unknown = null;
      while (entryReader.hasMore()) {
        const entryTag = entryReader.readTag();
        if ((entryTag >>> 3) === 1) key = entryReader.readString();
        else if ((entryTag >>> 3) === 2) value = decodeValue(entryReader.readBytes());
        else entryReader.skip(entryTag & 0x07);
      }
      if (key) result[key] = value;
    } else {
      reader.skip(tag & 0x07);
    }
  }
  return result;
}

/** Decode google.protobuf.ListValue → unknown[] */
function decodeListValue(buf: Uint8Array): unknown[] {
  const result: unknown[] = [];
  const reader = new ProtoReader(buf);
  while (reader.hasMore()) {
    const tag = reader.readTag();
    if ((tag >>> 3) === 1) {
      result.push(decodeValue(reader.readBytes()));
    } else {
      reader.skip(tag & 0x07);
    }
  }
  return result;
}

/** Minimal protobuf wire-format reader */
class ProtoReader {
  private pos = 0;
  constructor(private buf: Uint8Array) {}

  hasMore(): boolean { return this.pos < this.buf.length; }

  readVarint(): number {
    let result = 0;
    let shift = 0;
    while (this.pos < this.buf.length) {
      const byte = this.buf[this.pos++]!;
      result |= (byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) return result >>> 0;
      shift += 7;
    }
    return result >>> 0;
  }

  readTag(): number { return this.readVarint(); }

  readBytes(): Uint8Array {
    const len = this.readVarint();
    const bytes = this.buf.slice(this.pos, this.pos + len);
    this.pos += len;
    return bytes;
  }

  readString(): string {
    return new TextDecoder().decode(this.readBytes());
  }

  readDouble(): number {
    const bytes = this.buf.slice(this.pos, this.pos + 8);
    this.pos += 8;
    return new DataView(bytes.buffer, bytes.byteOffset, 8).getFloat64(0, true);
  }

  skip(wireType: number): void {
    switch (wireType) {
      case 0: this.readVarint(); break;
      case 1: this.pos += 8; break;
      case 2: this.pos += this.readVarint(); break;
      case 5: this.pos += 4; break;
      default: break;
    }
  }
}
