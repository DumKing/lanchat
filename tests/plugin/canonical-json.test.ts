import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { canonicalizePluginValue, sha256PluginValue } from "../../packages/plugin-sdk/src/index";

const vectors = JSON.parse(await readFile(resolve("tests/canonical-json-vectors.json"), "utf8")) as {
  canonicalization: Array<{ name: string; input: unknown; expected: string }>;
  hashes: Array<{ name: string; input: unknown; expected: string }>;
};

describe("canonical-json-v1", () => {
  for (const vector of vectors.canonicalization) {
    it(`规范化固定向量：${vector.name}`, () => {
      expect(canonicalizePluginValue(vector.input)).toBe(vector.expected);
    });
  }

  for (const vector of vectors.hashes) {
    it(`SHA-256 固定向量：${vector.name}`, async () => {
      await expect(sha256PluginValue(vector.input)).resolves.toBe(vector.expected);
    });
  }

  it.each([
    ["undefined", undefined],
    ["function", () => undefined],
    ["symbol", Symbol("invalid")],
    ["bigint", 1n],
    ["NaN", Number.NaN],
    ["Infinity", Number.POSITIVE_INFINITY],
    ["non-plain object", new Date(0)],
  ])("拒绝 %s", (_name, value) => {
    expect(() => canonicalizePluginValue(value)).toThrow(TypeError);
  });

  it("拒绝稀疏数组、对象中的非法值和循环引用", () => {
    const sparse = new Array(1);
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;

    expect(() => canonicalizePluginValue(sparse)).toThrow(TypeError);
    expect(() => canonicalizePluginValue({ value: undefined })).toThrow(TypeError);
    expect(() => canonicalizePluginValue(cyclic)).toThrow(TypeError);
  });
});
