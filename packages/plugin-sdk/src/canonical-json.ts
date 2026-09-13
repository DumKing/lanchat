export type PluginJsonValue =
  | null
  | boolean
  | string
  | number
  | PluginJsonValue[]
  | { [key: string]: PluginJsonValue };

function invalidValue(message: string): never {
  throw new TypeError(`canonical-json-v1: ${message}`);
}

function canonicalize(value: unknown, ancestors: Set<object>): string {
  if (value === null) return "null";

  switch (typeof value) {
    case "boolean":
      return value ? "true" : "false";
    case "string":
      return JSON.stringify(value);
    case "number":
      if (!Number.isFinite(value)) invalidValue("numbers must be finite");
      return Object.is(value, -0) ? "0" : JSON.stringify(value);
    case "undefined":
    case "function":
    case "symbol":
    case "bigint":
      return invalidValue(`unsupported value type: ${typeof value}`);
    case "object":
      break;
    default:
      return invalidValue("unsupported value");
  }

  if (ancestors.has(value)) invalidValue("cyclic values are not supported");
  ancestors.add(value);
  try {
    if (Array.isArray(value)) {
      const entries: string[] = [];
      for (let index = 0; index < value.length; index += 1) {
        if (!Object.prototype.hasOwnProperty.call(value, index)) {
          invalidValue("sparse arrays are not supported");
        }
        entries.push(canonicalize(value[index], ancestors));
      }
      return `[${entries.join(",")}]`;
    }

    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) {
      invalidValue("only plain objects are supported");
    }
    if (Object.getOwnPropertySymbols(value).length > 0) {
      invalidValue("symbol keys are not supported");
    }

    const record = value as Record<string, unknown>;
    const entries = Object.keys(record)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalize(record[key], ancestors)}`);
    return `{${entries.join(",")}}`;
  } finally {
    ancestors.delete(value);
  }
}

/**
 * Serializes a standard JSON value using canonical-json-v1.
 * Plain object keys use JavaScript UTF-16 code-unit ordering and arrays retain their input order.
 */
export function canonicalizePluginValue(value: unknown): string {
  return canonicalize(value, new Set<object>());
}

/** Returns the lowercase SHA-256 hex digest of the canonical-json-v1 UTF-8 bytes. */
export async function sha256PluginValue(value: unknown): Promise<string> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) throw new Error("Web Crypto subtle.digest is unavailable");
  const bytes = new TextEncoder().encode(canonicalizePluginValue(value));
  const digest = await subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}
