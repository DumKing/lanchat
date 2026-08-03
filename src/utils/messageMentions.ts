export type MentionKind = "me" | "all";

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function detectMentionKind(content: string, nickname: string): MentionKind | null {
  const mentionBoundary = "(^|\\s|[，。！？、,.:：；;])";
  if (new RegExp(`${mentionBoundary}@所有人(?=\\s|$|[，。！？、,.:：；;])`).test(content)) return "all";
  const normalizedNickname = nickname.trim();
  if (!normalizedNickname) return null;
  const pattern = new RegExp(`${mentionBoundary}@${escapeRegExp(normalizedNickname)}(?=\\s|$|[，。！？、,.:：；;])`);
  return pattern.test(content) ? "me" : null;
}

export function trayConversationTitle(title: string, kind: "direct" | "group") {
  if (kind !== "group" || title.endsWith("频道")) return title;
  return `${title}频道`;
}
