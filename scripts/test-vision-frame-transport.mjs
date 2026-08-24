import assert from "node:assert/strict";
import { transform } from "esbuild";
import { readFile } from "node:fs/promises";

const source = await readFile(
  new URL("../src/services/visionFrameTransport.ts", import.meta.url),
  "utf8",
);
const compiled = await transform(source, { loader: "ts", format: "esm", target: "es2022" });
const transport = await import(`data:text/javascript,${encodeURIComponent(compiled.code)}`);

const first = new transport.VisionFrameTransport("11111111-1111-4111-8111-111111111111");
const firstFrame = first.encode({
  capturedAt: 100,
  width: 2,
  height: 1,
  rgba: new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]),
});
const decodedFirst = transport.readVisionFrameEnvelope(firstFrame);
assert.equal(decodedFirst.streamGeneration, 1);
assert.equal(decodedFirst.frameId, 1);
assert.deepEqual([...decodedFirst.rgba], [1, 2, 3, 4, 5, 6, 7, 8]);

const rebuilt = first.reset("22222222-2222-4222-8222-222222222222");
const rebuiltFrame = first.encode({
  capturedAt: 200,
  width: 1,
  height: 1,
  rgba: new Uint8Array([9, 10, 11, 12]),
});
assert.equal(rebuilt.streamGeneration, 2);
assert.notEqual(decodedFirst.streamId, transport.readVisionFrameEnvelope(rebuiltFrame).streamId);
assert.equal(transport.readVisionFrameEnvelope(rebuiltFrame).streamGeneration, 2);

assert.throws(
  () => transport.readVisionFrameEnvelope(firstFrame.subarray(0, firstFrame.byteLength - 1)),
  /payload length/i,
);

console.log("vision raw frame transport passed");
