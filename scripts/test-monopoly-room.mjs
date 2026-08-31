import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "monopoly-room-"));
const outfile = path.join(tempDir, "monopoly-room.mjs");

try {
  await build({ entryPoints: [path.join(root, "src/games/monopolyRoom.ts")], outfile, bundle: true, format: "esm", platform: "node", logLevel: "silent" });
  const { applyMonopolyRoomAction, createMonopolyRoomState } = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const host = { deviceId: "a", nickname: "甲", online: true, ready: false };
  let room = createMonopolyRoomState({ roomId: "monopoly-1", host, startingCoins: 5000, maxRounds: 20, now: 1 });
  room = applyMonopolyRoomAction(room, { action: "join", player: { deviceId: "b", nickname: "乙", online: true, ready: false } });
  room = applyMonopolyRoomAction(room, { action: "ready", playerId: "a", ready: true });
  room = applyMonopolyRoomAction(room, { action: "ready", playerId: "b", ready: true });
  room = applyMonopolyRoomAction(room, { action: "start", playerId: "a" });
  assert.equal(room.phase, "playing");
  assert.equal(room.game.players.length, 2);
  room = applyMonopolyRoomAction(room, { action: "roll", playerId: "a" }, () => 0);
  assert.equal(room.game.players[0].position, 1);
  room = applyMonopolyRoomAction(room, { action: "buy", playerId: "a", propertyIndex: 1 });
  assert.equal(room.game.properties[1].ownerDeviceId, "a");
  room = applyMonopolyRoomAction(room, { action: "end_turn", playerId: "a" });
  assert.equal(room.game.currentPlayerId, "b");
  console.log("monopoly room rules ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
