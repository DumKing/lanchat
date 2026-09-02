import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const root = process.cwd();
const tempRoot = path.join(root, ".tmp");
await mkdir(tempRoot, { recursive: true });
const tempDir = await mkdtemp(path.join(tempRoot, "monopoly-"));
const outfile = path.join(tempDir, "monopoly.mjs");

try {
  await build({
    entryPoints: [path.join(root, "src/games/monopoly.ts")],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    logLevel: "silent",
  });

  const monopoly = await import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
  const {
    MONOPOLY_BOARD_SIZE,
    MONOPOLY_TURN_TIMEOUT_MS,
    applyMonopolyPropertySeal,
    applyMonopolyPriceDouble,
    applyMonopolyRandomEvent,
    acquireMonopolyGod,
    discardMonopolyCard,
    drawMonopolyCard,
    createMonopolyBoard,
    createMonopolyState,
    declareMonopolyBankruptcy,
    endMonopolyTurn,
    moveMonopolyPlayer,
    monopolyLandingRent,
    monopolyCardWeights,
    monopolyTurnRemainingSeconds,
    propertyDistrictOf,
    purchaseMonopolyProperty,
    sendMonopolyPlayerToJail,
    teleportMonopolyPlayer,
    upgradeMonopolyProperty,
    useMonopolyCard,
    resolveMonopolyLanding,
  } = monopoly;

  const board = createMonopolyBoard();
  assert.equal(MONOPOLY_BOARD_SIZE, 40);
  assert.equal(board.length, 40);
  assert.deepEqual(
    board.filter((tile) => tile.kind === "corner").map((tile) => tile.corner),
    ["start", "airport", "price_double", "jail"],
  );
  assert.equal(board.filter((tile) => tile.kind === "property").length, 32);
  assert.equal(board.filter((tile) => tile.kind === "event").length, 4);

  for (const eventIndex of [5, 15, 25, 35]) {
    assert.equal(board[eventIndex].kind, "event", `tile ${eventIndex} should be a random event`);
  }
  for (const cornerIndex of [0, 10, 20, 30]) {
    assert.equal(board[cornerIndex].kind, "corner", `tile ${cornerIndex} should be a corner`);
  }

  assert.equal(propertyDistrictOf(board, 1), 0);
  assert.equal(propertyDistrictOf(board, 4), 0);
  assert.equal(propertyDistrictOf(board, 6), 1);
  assert.equal(propertyDistrictOf(board, 9), 1);
  assert.equal(propertyDistrictOf(board, 5), null, "event tiles do not belong to a property district");
  assert.equal(propertyDistrictOf(board, 10), null, "corner tiles do not belong to a property district");

  let state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 5000, maxRounds: 20 });
  assert.equal(MONOPOLY_TURN_TIMEOUT_MS, 30_000);
  assert.equal(monopolyTurnRemainingSeconds(undefined, 1000), 30);
  assert.equal(state.properties[1].level, "empty");

  let result = purchaseMonopolyProperty(state, "a", 1, () => 0.99);
  assert.equal(result.ok, true);
  state = result.state;
  assert.equal(state.players[0].coins, 4650, "empty land should cost 350");
  assert.equal(state.properties[1].level, "house");
  assert.equal(state.properties[1].buildingVariant, 2, "首次购买小屋时应由房主随机固定建筑款式");
  assert.equal(state.properties[1].ownerDeviceId, "a");
  assert.equal(monopolyLandingRent(state, "b", 1), 500);

  result = upgradeMonopolyProperty(state, "a", 1, () => 0.49);
  assert.equal(result.ok, true);
  state = result.state;
  assert.equal(state.players[0].coins, 4650, "升级地产不应扣除金币");
  assert.equal(state.properties[1].level, "level2");
  assert.equal(state.properties[1].buildingVariant, 1, "升级洋房时应重新随机并保存建筑款式");
  assert.equal(monopolyLandingRent(state, "b", 1), 1000);

  result = upgradeMonopolyProperty(state, "a", 1, () => 0.01);
  assert.equal(result.ok, true);
  state = result.state;
  assert.equal(state.players[0].coins, 4650, "连续升级也不应扣除金币");
  assert.equal(state.properties[1].level, "level3");
  assert.equal(state.properties[1].buildingVariant, 0, "升级地标时应重新随机并保存建筑款式");
  assert.equal(monopolyLandingRent(state, "b", 1), 2000);
  state = applyMonopolyPropertySeal(state, 1, 3);
  assert.match(state.logs.at(-1), /地标/, "三级建筑的系统日志应使用地标名称");

  state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 50_000, maxRounds: 20 });
  for (const index of [1, 2, 3, 4]) {
    result = purchaseMonopolyProperty(state, "a", index);
    assert.equal(result.ok, true);
    state = result.state;
  }
  assert.equal(monopolyLandingRent(state, "b", 2), 2000, "four houses should form a four-tile row");
  state = applyMonopolyPropertySeal(state, 3, 3);
  assert.equal(monopolyLandingRent(state, "b", 2), 1000, "a sealed property should cut the row into 1+2 and 4");
  state = endMonopolyTurn(state);
  assert.equal(state.properties[3].sealedTurns, 2, "seals decrease after the owner's turn");
  state = endMonopolyTurn(state);
  assert.equal(state.properties[3].sealedTurns, 2, "another player's turn does not decrease the seal");

  const houseVariantBeforeBankruptcy = state.properties[1].buildingVariant;
  state = declareMonopolyBankruptcy(state, "a");
  assert.equal(state.players[0].eliminated, true);
  assert.equal(state.properties[1].ownerDeviceId, null);
  assert.equal(state.properties[1].level, "house", "bankruptcy preserves building level");
  assert.equal(state.properties[1].buildingVariant, houseVariantBeforeBankruptcy, "bankruptcy should preserve the building style and only turn it gray in the UI");
  result = purchaseMonopolyProperty(state, "b", 1);
  assert.equal(result.ok, true);
  assert.equal(result.state.players[1].coins, 49_500, "a bank-held house should cost 500 to buy directly");

  state = createMonopolyState([{ deviceId: "a", nickname: "甲" }], { startingCoins: 5000, maxRounds: 20 });
  state.players[0].position = 38;
  result = moveMonopolyPlayer(state, "a", 4);
  assert.equal(result.ok, true);
  state = result.state;
  assert.equal(state.players[0].position, 2);
  assert.equal(state.players[0].coins, 5200, "passing start should grant 200 coins");

  state.players[0].position = 10;
  result = teleportMonopolyPlayer(state, "a", 0);
  assert.equal(result.ok, true);
  state = result.state;
  assert.equal(state.players[0].position, 0);
  assert.equal(state.players[0].coins, 5400, "teleporting to start should settle the start reward");

  result = purchaseMonopolyProperty(state, "a", 1);
  assert.equal(result.ok, true);
  state = result.state;
  state = applyMonopolyPriceDouble(state, "a", () => 0);
  assert.equal(state.properties[1].tollMultiplier, 2, "price-double corner should mark an owned property once");
  assert.equal(monopolyLandingRent(state, "b", 1), 1000);
  state = applyMonopolyPriceDouble(state, "a", () => 0);
  assert.equal(state.properties[1].tollMultiplier, 3, "price-double corner should stack instead of resetting at x2");
  assert.equal(monopolyLandingRent(state, "b", 1), 1500);

  state.players[0].cards = [];
  state = sendMonopolyPlayerToJail(state, "a");
  assert.equal(state.players[0].position, 30);
  assert.equal(state.players[0].jailTurns, 3);
  state.players[0].cards = ["acquittal"];
  state = sendMonopolyPlayerToJail(state, "a");
  assert.equal(state.players[0].position, 0, "acquittal should immediately return its holder to start");
  assert.equal(state.players[0].jailTurns, 0);
  assert.equal(state.players[0].cards.includes("acquittal"), false);

  state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 5000, maxRounds: 20, seed: 7 });
  assert.equal(state.players[0].cards.length, 3, "every player should start with three cards");
  assert.equal(new Set(state.players[0].cards).size, 3, "opening cards should not repeat");
  assert.ok(state.players[0].cards.filter((card) => ["seize", "frame", "loot", "seal"].includes(card)).length <= 1);
  assert.equal(monopolyCardWeights(state, "a").seize, 3, "normal seize probability should stay low");

  state.players[0].coins = 100;
  state.players[1].coins = 50_000;
  for (const index of [1, 2, 3, 4]) {
    state.properties[index] = { ...state.properties[index], ownerDeviceId: "b", level: "house" };
  }
  assert.equal(monopolyCardWeights(state, "a").seize, 8, "a player low on money and buildings gets a limited comeback boost");
  const fullDraw = drawMonopolyCard(state, "a", () => 0);
  assert.equal(fullDraw.card, null, "players with three cards should not receive another card");
  state = discardMonopolyCard(state, "a", state.players[0].cards[0]);
  assert.equal(state.players[0].cards.length, 2);
  const drawn = drawMonopolyCard(state, "a", () => 0);
  assert.ok(drawn.card, "players with free backpack space should draw a card");
  assert.equal(drawn.state.players[0].cards.length, 3);

  state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 50_000, maxRounds: 20, seed: 3 });
  state.players[0].cards = ["reverse", "roadblock", "seize"];
  state.players[1].cards = ["turtle", "loot", "seal"];
  result = useMonopolyCard(state, "a", "reverse", { playerId: "b" });
  assert.equal(result.ok, true);
  state = result.state;
  assert.equal(state.players[1].direction, "counterclockwise", "reverse is permanent until another reverse card");
  result = useMonopolyCard(state, "a", "roadblock", { index: 39 });
  assert.equal(result.ok, true);
  state = result.state;
  state.players[0].position = 38;
  result = moveMonopolyPlayer(state, "a", 4);
  assert.equal(result.ok, true);
  assert.equal(result.state.players[0].position, 39, "roadblocks should stop their owner too");
  assert.equal(result.state.roadblocks.length, 0, "a triggered roadblock disappears");

  state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 50_000, maxRounds: 20, seed: 3 });
  state.players[0].cards = ["seize"];
  result = purchaseMonopolyProperty(state, "b", 1);
  assert.equal(result.ok, true);
  state = result.state;
  result = useMonopolyCard(state, "a", "seize", { propertyIndex: 1 });
  assert.equal(result.ok, true);
  assert.equal(result.state.properties[1].ownerDeviceId, "a");

  state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 50_000, maxRounds: 20, seed: 3 });
  state.players[0].cards = ["turtle"];
  result = useMonopolyCard(state, "a", "turtle", { playerId: "a" });
  assert.equal(result.ok, true, "乌龟卡应允许指定自己");
  state = result.state;
  assert.equal(state.players[0].turtleTurns, 3);
  state.players[0].cards = ["reverse"];
  result = useMonopolyCard(state, "a", "reverse", { playerId: "a" });
  assert.equal(result.ok, true, "转向卡应允许指定自己");
  state = result.state;
  assert.equal(state.players[0].direction, "counterclockwise");
  state.players[0].cards = ["frame"];
  result = useMonopolyCard(state, "a", "frame", { playerId: "a" });
  assert.equal(result.ok, true, "陷害卡应允许指定自己");
  assert.equal(result.state.players[0].jailTurns, 3);

  state = createMonopolyState([{ deviceId: "a", nickname: "甲" }], { startingCoins: 50_000, maxRounds: 20, seed: 3 });
  result = purchaseMonopolyProperty(state, "a", 1);
  assert.equal(result.ok, true);
  state = result.state;
  state.players[0].cards = ["double"];
  result = useMonopolyCard(state, "a", "double", { propertyIndex: 1 });
  assert.equal(result.ok, true);
  state = result.state;
  assert.equal(state.properties[1].tollMultiplier, 2, "a double card should apply the first x2 tier");
  state.players[0].cards = ["double"];
  result = useMonopolyCard(state, "a", "double", { propertyIndex: 1 });
  assert.equal(result.ok, true);
  assert.equal(result.state.properties[1].tollMultiplier, 3, "a second double card should stack to x3");

  state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 5000, maxRounds: 20, seed: 9 });
  result = purchaseMonopolyProperty(state, "a", 1);
  assert.equal(result.ok, true);
  state = result.state;
  state = acquireMonopolyGod(state, "b", "wealth");
  assert.equal(monopolyLandingRent(state, "b", 1), 0, "wealth god should waive every property rent");
  state = acquireMonopolyGod(state, "b", "poverty");
  assert.equal(monopolyLandingRent(state, "b", 1), 1000, "poverty god should double property rent");
  state = acquireMonopolyGod(state, "b", "angel");
  state = resolveMonopolyLanding(state, "b", 1, () => 0);
  assert.equal(state.properties[1].level, "level2", "angel acts before the property settlement");
  state = acquireMonopolyGod(state, "b", "devil");
  state = resolveMonopolyLanding(state, "b", 1, () => 0);
  assert.equal(state.properties[1].level, "house");
  state = resolveMonopolyLanding(state, "b", 1, () => 0);
  assert.equal(state.properties[1].level, "empty", "devil can turn a house into empty land");
  assert.equal(state.properties[1].ownerDeviceId, null);
  const coinsBeforeSubsidy = state.players.map((player) => player.coins);
  state = applyMonopolyRandomEvent(state, "subsidy", () => 0);
  assert.deepEqual(state.players.map((player, index) => player.coins - coinsBeforeSubsidy[index]), [100, 100], "subsidy event grants every active player 100 coins");

  state = createMonopolyState([
    { deviceId: "a", nickname: "甲" },
    { deviceId: "b", nickname: "乙" },
  ], { startingCoins: 5000, maxRounds: 20, seed: 13 });
  state.players.forEach((player) => { player.cards = []; });
  for (let index = 0; index < 6; index += 1) state = endMonopolyTurn(state, index, () => 0);
  assert.equal(state.completedRounds, 3, "six player turns should complete three rounds in a two-player match");
  assert.ok(state.players.every((player) => player.cards.length === 1), "every third complete round should issue one card when a bag has space");
  assert.equal(state.godTokens.length, 2, "every third complete round should refresh up to two god tokens");

  console.log("monopoly board rules ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
