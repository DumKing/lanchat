import type {
  GameContribution,
  ManifestValidationError,
  ManifestValidationResult,
  NavigationContribution,
  PluginCapability,
  PluginManifestV1,
} from "../contracts/manifest";

const SUPPORTED_CAPABILITIES = new Set<string>([
  "devices.read",
  "chat.read",
  "chat.send",
  "rooms.read",
  "rooms.write",
  "leaderboard.read",
  "leaderboard.write",
  "storage.private",
  "theme.read",
  "ui.notify",
  "ui.filePicker",
  "network.lan",
  "network.internet",
  "logger.write",
  "native.sidecar",
]);

const PLUGIN_ID_PATTERN = /^[a-z][a-z0-9]*(?:[.-][a-z0-9]+){2,}$/;
const SEMVER_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const CONTRIBUTION_ID_PATTERN = /^[a-z][a-z0-9]*(?:[._-][a-z0-9]+)*$/;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSafeRelativePath(value: unknown): value is string {
  if (typeof value !== "string" || value.length === 0 || value.includes("\\")) return false;
  if (value.startsWith("/") || /^[A-Za-z]:\//.test(value)) return false;
  const segments = value.split("/");
  return segments.every((segment) => segment.length > 0 && segment !== "." && segment !== "..");
}

function pushError(errors: ManifestValidationError[], field: string, code: ManifestValidationError["code"], message: string) {
  errors.push({ field, code, message });
}

function validateContributionIds(
  values: unknown,
  field: "navigation" | "games" | "commands",
  errors: ManifestValidationError[],
) {
  if (values === undefined) return;
  if (!Array.isArray(values)) {
    pushError(errors, `contributes.${field}`, "INVALID_FORMAT", "贡献必须是数组");
    return;
  }
  const seen = new Set<string>();
  values.forEach((value, index) => {
    if (!isRecord(value) || typeof value.id !== "string" || !CONTRIBUTION_ID_PATTERN.test(value.id)) {
      pushError(errors, `contributes.${field}[${index}].id`, "INVALID_FORMAT", "贡献 ID 格式不正确");
      return;
    }
    if (seen.has(value.id)) {
      pushError(errors, `contributes.${field}[${index}].id`, "DUPLICATE", "同类贡献 ID 不能重复");
    }
    seen.add(value.id);
  });
}

function validateNavigation(values: unknown, errors: ManifestValidationError[]) {
  if (!Array.isArray(values)) return;
  values.forEach((value, index) => {
    if (!isRecord(value)) return;
    if (typeof value.title !== "string" || value.title.trim().length === 0) {
      pushError(errors, `contributes.navigation[${index}].title`, "REQUIRED", "导航标题不能为空");
    }
    if (value.icon !== undefined && !isSafeRelativePath(value.icon)) {
      pushError(errors, `contributes.navigation[${index}].icon`, "INVALID_FORMAT", "导航图标必须位于插件目录内");
    }
  });
}

function validateGames(values: unknown, errors: ManifestValidationError[]) {
  if (!Array.isArray(values)) return;
  values.forEach((value, index) => {
    if (!isRecord(value)) return;
    if (!Number.isInteger(value.minPlayers) || Number(value.minPlayers) < 1) {
      pushError(errors, `contributes.games[${index}].minPlayers`, "OUT_OF_RANGE", "最少玩家数必须是正整数");
    }
    if (!Number.isInteger(value.maxPlayers) || Number(value.maxPlayers) < Number(value.minPlayers)) {
      pushError(errors, `contributes.games[${index}].maxPlayers`, "OUT_OF_RANGE", "最大玩家数不能小于最少玩家数");
    }
    if (value.ranking !== undefined) {
      if (!isRecord(value.ranking) || !["win-loss", "score", "time"].includes(String(value.ranking.type))) {
        pushError(errors, `contributes.games[${index}].ranking`, "INVALID_FORMAT", "排行榜策略格式不正确");
      } else if (value.ranking.minHumanPlayers !== undefined) {
        const threshold = value.ranking.minHumanPlayers;
        if (!Number.isInteger(threshold) || Number(threshold) < 1 || Number(threshold) > Number(value.maxPlayers)) {
          pushError(errors, `contributes.games[${index}].ranking.minHumanPlayers`, "OUT_OF_RANGE", "计榜真人阈值必须在 1 和最大玩家数之间");
        }
      }
    }
  });
}

export function validatePluginManifest(input: unknown): ManifestValidationResult {
  const errors: ManifestValidationError[] = [];
  if (!isRecord(input)) {
    return { ok: false, errors: [{ field: "$", code: "INVALID_FORMAT", message: "插件清单必须是对象" }] };
  }

  if (input.manifestVersion !== 1) pushError(errors, "manifestVersion", "UNSUPPORTED", "仅支持 manifestVersion 1");
  if (typeof input.id !== "string" || !PLUGIN_ID_PATTERN.test(input.id)) pushError(errors, "id", "INVALID_FORMAT", "插件 ID 必须使用反向域名格式");
  if (typeof input.name !== "string" || input.name.trim().length === 0) pushError(errors, "name", "REQUIRED", "插件名称不能为空");
  if (typeof input.version !== "string" || !SEMVER_PATTERN.test(input.version)) pushError(errors, "version", "INVALID_FORMAT", "插件版本必须是 SemVer");
  if (input.apiVersion !== "1.0") pushError(errors, "apiVersion", "UNSUPPORTED", "当前宿主仅支持 API 1.0");
  if (typeof input.minHostVersion !== "string" || !SEMVER_PATTERN.test(input.minHostVersion)) pushError(errors, "minHostVersion", "INVALID_FORMAT", "最低宿主版本必须是 SemVer");
  if (input.type !== "web" && input.type !== "web-sidecar") pushError(errors, "type", "UNSUPPORTED", "不支持此插件类型");
  if (!isSafeRelativePath(input.entry)) pushError(errors, "entry", "INVALID_FORMAT", "入口必须是插件目录内的安全相对路径");
  if (!isSafeRelativePath(input.icon)) pushError(errors, "icon", "INVALID_FORMAT", "图标必须是插件目录内的安全相对路径");
  if (typeof input.singleton !== "boolean") pushError(errors, "singleton", "REQUIRED", "必须声明 singleton");

  if (!Array.isArray(input.capabilities)) {
    pushError(errors, "capabilities", "INVALID_FORMAT", "能力声明必须是数组");
  } else {
    const seenCapabilities = new Set<string>();
    input.capabilities.forEach((capability, index) => {
      if (typeof capability !== "string" || !SUPPORTED_CAPABILITIES.has(capability)) {
        pushError(errors, `capabilities[${index}]`, "UNSUPPORTED", "包含宿主不支持的能力");
      } else if (seenCapabilities.has(capability)) {
        pushError(errors, `capabilities[${index}]`, "DUPLICATE", "能力不能重复声明");
      }
      if (typeof capability === "string") seenCapabilities.add(capability);
    });
  }

  if (input.contributes !== undefined && !isRecord(input.contributes)) {
    pushError(errors, "contributes", "INVALID_FORMAT", "贡献声明必须是对象");
  } else if (isRecord(input.contributes)) {
    validateContributionIds(input.contributes.navigation, "navigation", errors);
    validateContributionIds(input.contributes.games, "games", errors);
    validateContributionIds(input.contributes.commands, "commands", errors);
    validateNavigation(input.contributes.navigation, errors);
    validateGames(input.contributes.games, errors);
  }

  if (errors.length > 0) return { ok: false, errors };

  let contributes: PluginManifestV1["contributes"];
  if (isRecord(input.contributes)) {
    contributes = {};
    if (input.contributes.navigation !== undefined) {
      contributes.navigation = input.contributes.navigation as NavigationContribution[];
    }
    if (input.contributes.games !== undefined) {
      contributes.games = input.contributes.games as GameContribution[];
    }
    if (input.contributes.commands !== undefined) {
      contributes.commands = input.contributes.commands as NonNullable<PluginManifestV1["contributes"]>["commands"];
    }
  }

  return {
    ok: true,
    manifest: {
      manifestVersion: 1,
      id: input.id as string,
      name: input.name as string,
      version: input.version as string,
      apiVersion: "1.0",
      minHostVersion: input.minHostVersion as string,
      type: input.type as PluginManifestV1["type"],
      entry: input.entry as string,
      icon: input.icon as string,
      singleton: input.singleton as boolean,
      capabilities: input.capabilities as PluginCapability[],
      contributes,
    },
  };
}
