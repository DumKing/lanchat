import { computed, ref, watch, type Ref } from "vue";
import { dateEnUS, dateZhCN, enUS, zhCN } from "naive-ui";

export type LanguagePreference = "system" | "zh-CN" | "en-US";
export type AppLocale = "zh-CN" | "en-US";

const STORAGE_KEY = "lanchat-language";

function systemLocale(): AppLocale {
  if (typeof navigator === "undefined") return "zh-CN";
  return navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en-US";
}

function readLanguagePreference(): LanguagePreference {
  if (typeof window === "undefined") return "system";
  const saved = window.localStorage.getItem(STORAGE_KEY);
  return saved === "zh-CN" || saved === "en-US" || saved === "system" ? saved : "system";
}

export const languagePreference = ref<LanguagePreference>(readLanguagePreference());
export const effectiveLocale = computed<AppLocale>(() => (
  languagePreference.value === "system" ? systemLocale() : languagePreference.value
));
export const naiveLocale = computed(() => effectiveLocale.value === "en-US" ? enUS : zhCN);
export const dateLocale = computed(() => effectiveLocale.value === "en-US" ? dateEnUS : dateZhCN);

watch(languagePreference, (value) => {
  if (typeof window !== "undefined") window.localStorage.setItem(STORAGE_KEY, value);
});

export function setLanguagePreference(value: string | number) {
  if (value === "system" || value === "zh-CN" || value === "en-US") {
    languagePreference.value = value;
  }
}

type TranslationParams = Record<string, string | number>;

const messages = {
  "zh-CN": {
    "language.system": "跟随系统",
    "language.chinese": "简体中文",
    "language.english": "English",
    "nav.chat": "聊天",
    "nav.devices": "设备列表",
    "nav.games": "游戏",
    "nav.alerts": "狼来了",
    "nav.vision": "视觉识别",
    "nav.notifications": "公告通知",
    "nav.settings": "设置",
    "settings.basic": "基础设置",
    "settings.camera": "摄像头检测",
    "settings.pet": "桌宠设置",
    "settings.admin": "超管设置",
    "common.online": "在线",
    "common.offline": "离线",
    "common.save": "保存",
    "common.cancel": "取消",
    "common.close": "关闭",
    "common.delete": "删除",
    "common.refresh": "刷新",
    "common.view": "查看",
    "common.send": "发送",
    "vision.workspace.title": "视觉识别",
    "vision.workspace.description": "本机离线运行，摄像头画面不保存、不上传；通话与检测继续共用同一摄像头。",
    "vision.workspace.configure": "打开检测设置",
    "vision.profile.baseline": "内置基线模型",
    "vision.profile.balanced": "均衡识别",
    "vision.profile.balanced.description": "面向普通工作笔记本，优先保证稳定性与响应速度。",
    "vision.profile.low_resource": "低资源模式",
    "vision.profile.low_resource.description": "降低采样与推理开销，适合设备繁忙时使用。",
    "vision.profile.available_soon": "即将支持",
    "vision.model.ready": "模型就绪",
    "vision.model.unavailable": "模型不可用",
    "vision.model.policy": "策略版本：",
    "vision.model.compatibility": "兼容性：",
    "vision.model.compatible": "已通过检查",
    "vision.model.needs_check": "等待检查",
    "vision.runtime.title": "运行状态",
    "vision.runtime.running": "运行中",
    "vision.runtime.paused": "已暂停",
    "vision.runtime.starting": "正在准备",
    "vision.runtime.busy": "正在处理",
    "vision.runtime.recovering": "正在恢复",
    "vision.runtime.degraded": "性能降级",
    "vision.runtime.hint": "监控持续在后台运行；停止通话视频不会关闭本机检测。",
    "vision.runtime.frames": "已接收帧",
    "vision.runtime.dropped": "跳过旧帧",
    "vision.runtime.model": "活动模型",
    "vision.runtime.latency": "处理耗时 P50 {p50}ms / P95 {p95}ms",
    "vision.runtime.queue": "队列 {depth}",
    "vision.runtime.pause": "暂停识别",
    "vision.runtime.resume": "恢复识别",
    "vision.people.title": "人员库",
    "vision.people.add": "添加人员",
    "vision.people.hint": "每人可保存多张参考图；质量不佳、过期或失效的样本会在详情中显示。",
    "vision.people.empty": "尚未录入识别人员",
    "vision.people.samples": "{count} 张参考图",
    "vision.people.active": "已启用",
    "vision.people.disabled": "已停用",
  },
  "en-US": {
    "language.system": "Follow system",
    "language.chinese": "Simplified Chinese",
    "language.english": "English",
    "nav.chat": "Chats",
    "nav.devices": "Contacts",
    "nav.games": "Games",
    "nav.alerts": "Alerts",
    "nav.vision": "Vision",
    "nav.notifications": "Notices",
    "nav.settings": "Settings",
    "settings.basic": "General",
    "settings.camera": "Camera Detection",
    "settings.pet": "Desktop Pet",
    "settings.admin": "Admin",
    "common.online": "Online",
    "common.offline": "Offline",
    "common.save": "Save",
    "common.cancel": "Cancel",
    "common.close": "Close",
    "common.delete": "Delete",
    "common.refresh": "Refresh",
    "common.view": "View",
    "common.send": "Send",
    "vision.workspace.title": "Vision Recognition",
    "vision.workspace.description": "Runs locally. Camera frames are not stored or uploaded, and calls continue to share the same camera.",
    "vision.workspace.configure": "Open detection settings",
    "vision.profile.baseline": "Built-in baseline",
    "vision.profile.balanced": "Balanced recognition",
    "vision.profile.balanced.description": "Stable and responsive for typical work laptops.",
    "vision.profile.low_resource": "Low resource mode",
    "vision.profile.low_resource.description": "Reduces sampling and inference overhead when the device is busy.",
    "vision.profile.available_soon": "Coming soon",
    "vision.model.ready": "Model ready",
    "vision.model.unavailable": "Model unavailable",
    "vision.model.policy": "Policy revision: ",
    "vision.model.compatibility": "Compatibility: ",
    "vision.model.compatible": "Verified",
    "vision.model.needs_check": "Pending check",
    "vision.runtime.title": "Runtime status",
    "vision.runtime.running": "Running",
    "vision.runtime.paused": "Paused",
    "vision.runtime.starting": "Starting",
    "vision.runtime.busy": "Processing",
    "vision.runtime.recovering": "Recovering",
    "vision.runtime.degraded": "Performance degraded",
    "vision.runtime.hint": "Monitoring keeps running in the background; stopping call video does not stop local detection.",
    "vision.runtime.frames": "Accepted frames",
    "vision.runtime.dropped": "Skipped frames",
    "vision.runtime.model": "Active model",
    "vision.runtime.latency": "Latency P50 {p50}ms / P95 {p95}ms",
    "vision.runtime.queue": "Queue {depth}",
    "vision.runtime.pause": "Pause recognition",
    "vision.runtime.resume": "Resume recognition",
    "vision.people.title": "People library",
    "vision.people.add": "Add person",
    "vision.people.hint": "Each person supports multiple references. Quality, expiry, and rebuild state are shown in details.",
    "vision.people.empty": "No people enrolled",
    "vision.people.samples": "{count} references",
    "vision.people.active": "Enabled",
    "vision.people.disabled": "Disabled",
  },
} as const;

export function t(key: keyof typeof messages["zh-CN"] | string, params: TranslationParams = {}) {
  const table = messages[effectiveLocale.value] as Record<string, string>;
  const fallback = messages["zh-CN"] as Record<string, string>;
  return (table[key] ?? fallback[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`));
}

const englishPhrases: Record<string, string> = {
  "局域网聊天": "LAN Chat",
  "聊天": "Chats",
  "设备列表": "Contacts",
  "游戏": "Games",
  "狼来了": "Alerts",
  "公告通知": "Notices",
  "历史公告": "Notice history",
  "设置": "Settings",
  "基础设置": "General",
  "摄像头检测": "Camera Detection",
  "桌宠设置": "Desktop Pet",
  "超管设置": "Admin",
  "内置游戏": "Built-in Games",
  "添加设备": "Add Device",
  "创建房间": "Create Room",
  "创建人": "Owner",
  "房间": "Rooms",
  "可用": "Available",
  "频道": "Channels",
  "已发现设备": "Discovered Devices",
  "本机": "This Device",
  "连接": "Connect",
  "加载更早消息": "Load earlier messages",
  "回到最新消息": "Jump to latest",
  "发送": "Send",
  "取消": "Cancel",
  "关闭": "Close",
  "删除": "Delete",
  "刷新": "Refresh",
  "清除": "Clear",
  "查看": "View",
  "详情": "Details",
  "邀请": "Invite",
  "同意": "Accept",
  "拒绝": "Reject",
  "加入": "Join",
  "已加入": "Joined",
  "已拒绝": "Rejected",
  "已过期": "Expired",
  "排行榜": "Leaderboard",
  "房间聊天": "Room Chat",
  "解散房间": "Dissolve Room",
  "退出房间": "Leave Room",
  "再来一局": "Play Again",
  "本局结算": "Results",
  "语音通话": "Audio Call",
  "视频通话": "Video Call",
  "抖一抖": "Nudge",
  "挂断": "Hang Up",
  "接听": "Answer",
  "所有人": "Everyone",
  "名次": "Rank",
  "昵称": "Nickname",
  "耗时": "Time",
  "步数": "Moves",
  "记录": "Records",
  "反馈": "Feedback",
  "人员": "Person",
  "真实度": "Credibility",
  "人脸确认": "Face Match",
  "人体特征": "Body Match",
  "设备通讯录": "Device Directory",
  "IP 地址": "IP Address",
  "端口": "Port",
  "MAC 地址": "MAC Address",
  "客户端": "Client",
  "软件版本": "App Version",
  "构建时间": "Build Time",
  "支持能力": "Capabilities",
  "最近在线": "Last Seen",
  "保存备注": "Save Note",
  "删除设备": "Remove Device",
  "频道类型": "Channel Type",
  "频道 ID": "Channel ID",
  "成员数量": "Members",
  "更新时间": "Updated",
  "频道成员": "Members",
  "群主": "Owner",
  "进入频道": "Open Channel",
  "邀请成员": "Invite Members",
  "解散频道": "Dissolve Channel",
  "退出群聊": "Leave Channel",
  "局域网频道": "LAN Channel",
  "群公告": "Announcement",
  "保存公告": "Save Announcement",
  "狼来了排行榜": "Alert Leaderboard",
  "识别率排行榜": "Recognition Leaderboard",
  "暂无记录": "No records",
  "暂无告警记录": "No alert records",
  "暂无自动识别告警记录": "No recognition alerts",
  "保存资料": "Save Profile",
  "开机自动启动 LanChat": "Start LanChat at login",
  "网络修复": "Network Repair",
  "重新授权麦克风": "Authorize Microphone",
  "重新授权摄像头": "Authorize Camera",
  "本机摄像头人物识别告警": "Local Camera Recognition",
  "人脸确认识别": "Face Recognition",
  "人体特征识别": "Body Recognition",
  "视频通话期间暂停识别": "Pause detection during video calls",
  "显示实时检测画面": "Show live detection preview",
  "本机实时检测画面": "Local Live Preview",
  "告警时弹出检测画面": "Show detection preview on alert",
  "摄像头状态": "Camera Status",
  "当前采样": "Sampling",
  "模型状态": "Model Status",
  "识别模型": "Recognition Model",
  "人体增强": "Body Enhancement",
  "模型版本": "Model Version",
  "模型资源": "Model Assets",
  "最近置信度": "Latest Confidence",
  "人脸阈值": "Face Threshold",
  "人体阈值": "Body Threshold",
  "策略采样": "Policy FPS",
  "连续命中": "Consecutive Hits",
  "人脸冷却": "Face Cooldown",
  "人体冷却": "Body Cooldown",
  "策略来源": "Policy Source",
  "已保存的指定人员": "Saved People",
  "本机识别人员": "Local People",
  "上传本地照片": "Upload Photos",
  "摄像头拍照": "Take Photo",
  "保存人员": "Save Person",
  "删除本机配置": "Delete Local Profile",
  "查看本机画面": "View Local Frame",
  "图片缓存": "Image Cache",
  "缓存文件": "Cached Files",
  "占用空间": "Disk Usage",
  "清理图片缓存": "Clear Image Cache",
  "版本更新": "Updates",
  "当前版本": "Current Version",
  "最新版本": "Latest Version",
  "检查状态": "Status",
  "上次检查": "Last Checked",
  "检查时间": "Checked At",
  "自动检查更新": "Automatic Update Checks",
  "检查更新": "Check for Updates",
  "下载更新": "Download Update",
  "手动下载": "Download Manually",
  "稍后提醒": "Remind Me Later",
  "退出软件": "Quit LanChat",
  "已启用": "Enabled",
  "保存 Token": "Save Token",
  "清除 Token": "Remove Token",
  "超管通知": "Admin Notices",
  "下发通知": "Send Notice",
  "查看通知审核记录": "Review History",
  "通知审核记录": "Notice Review History",
  "一键通过待审核": "Approve All Pending",
  "一键拒绝待审核": "Reject All Pending",
  "指定设备强制更新": "Force Update Device",
  "目标设备": "Target Device",
  "目标版本": "Target Version",
  "本地安装包（可选）": "Local Package (Optional)",
  "选择 EXE / MSI / ZIP": "Choose EXE / MSI / ZIP",
  "下发强制更新": "Send Forced Update",
  "摄像头人物识别策略": "Camera Recognition Policy",
  "选择参考照片": "Choose Reference Photos",
  "保存下发草稿": "Save and Send Person",
  "下发检测策略": "Send Detection Policy",
  "导入桌宠": "Import Desktop Pet",
  "桌宠导入目录规则": "Desktop Pet Import Rules",
  "打开资源目录": "Open Resource Folder",
  "删除桌宠": "Delete Desktop Pet",
  "保存并应用": "Save and Apply",
  "启用桌面桌宠告警器": "Enable Desktop Pet Alerts",
  "随机巡逻": "Random Patrol",
  "随机生活动作": "Random Life Actions",
  "普通报警": "Normal Alert",
  "蹦迪报警": "Disco Alert",
  "线性移动": "Linear Movement",
  "跳跃移动": "Jump Movement",
  "发送一次测试告警": "Send Test Alert",
  "开启外部推送": "Enable External Push",
  "添加企业微信群": "Add WeCom Group",
  "添加钉钉群": "Add DingTalk Group",
  "@所有人": "@Everyone",
  "告警真实度": "Alert Credibility",
  "清空该用户可信度": "Clear User Credibility",
  "一键清空狼来了排行榜": "Clear Alert Leaderboard",
  "报警模式下发": "Alert Mode Delivery",
  "下发报警模式": "Send Alert Mode",
  "狼来了推送阈值下发": "Alert Push Threshold",
  "下发推送阈值": "Send Push Threshold",
  "调试模式": "Debug Mode",
  "清空日志": "Clear Logs",
  "内存诊断": "Memory Diagnostics",
  "JS 堆": "JS Heap",
  "消息缓存": "Message Cache",
  "当前消息节点": "Message Nodes",
  "头像 Base64": "Avatar Base64",
  "刷新诊断": "Refresh Diagnostics",
  "验证并开启": "Verify and Enable",
  "重新尝试": "Try Again",
  "我知道了": "Got It",
  "可关闭": "Dismissible",
  "必须确认": "Confirmation Required",
  "强制打开目标主窗口": "Force open target window",
  "通过": "Approve",
  "撤销并放行": "Revoke and Release",
  "重新提交确认": "Resubmit",
  "已发送": "Sent",
  "在线": "Online",
  "离线": "Offline",
};

const excludedSelector = [
  ".message-bubble",
  ".room-chat-bubble",
  ".room-chat-name",
  ".channel-notice-content",
  ".admin-notification-lock-content p",
  "textarea",
  "[contenteditable='true']",
].join(",");

function translateDynamicPhrase(value: string) {
  const exact = englishPhrases[value];
  if (exact) return exact;
  return value
    .replace(/^(\d+) 台设备在线$/, "$1 devices online")
    .replace(/^(\d+) 在线$/, "$1 online")
    .replace(/^已选择 (\d+) 张本地照片$/, "$1 local photos selected")
    .replace(/^参考照片：(\d+) 张$/, "$1 reference photos")
    .replace(/^桌宠资源（(\d+)）$/, "Desktop Pets ($1)")
    .replace(/^(.+)排行榜$/, "$1 Leaderboard")
    .replace(/^(.+) 秒$/, "$1 sec")
    .replace(/^(.+) 个$/, "$1 items");
}

function translateTextValue(value: string) {
  if (effectiveLocale.value !== "en-US") return value;
  const leading = value.match(/^\s*/)?.[0] ?? "";
  const trailing = value.match(/\s*$/)?.[0] ?? "";
  const core = value.slice(leading.length, value.length - trailing.length);
  if (!core) return value;
  return `${leading}${translateDynamicPhrase(core)}${trailing}`;
}

export function installUiTranslation(root: HTMLElement = document.body) {
  const originalText = new WeakMap<Text, string>();
  const originalAttributes = new WeakMap<Element, Map<string, string>>();
  const attributes = ["placeholder", "title", "aria-label"];

  const excluded = (node: Node) => {
    const parent = node.nodeType === Node.ELEMENT_NODE ? node as Element : node.parentElement;
    return Boolean(parent?.closest(excludedSelector));
  };

  const translateNode = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const textNode = node as Text;
      if (excluded(textNode)) return;
      if (effectiveLocale.value === "zh-CN") {
        const original = originalText.get(textNode);
        if (original !== undefined && textNode.data !== original) textNode.data = original;
        originalText.delete(textNode);
        return;
      }
      const previous = originalText.get(textNode);
      if (previous !== undefined && textNode.data === translateTextValue(previous)) return;
      originalText.set(textNode, textNode.data);
      const translated = translateTextValue(textNode.data);
      if (translated !== textNode.data) textNode.data = translated;
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const element = node as Element;
    if (!excluded(element)) {
      let originals = originalAttributes.get(element);
      for (const attribute of attributes) {
        const current = element.getAttribute(attribute);
        if (current === null) continue;
        if (effectiveLocale.value === "zh-CN") {
          const original = originals?.get(attribute);
          if (original !== undefined && current !== original) element.setAttribute(attribute, original);
          originals?.delete(attribute);
        } else {
          originals ??= new Map<string, string>();
          const previous = originals.get(attribute);
          if (previous !== undefined && current === translateTextValue(previous)) continue;
          originals.set(attribute, current);
          const translated = translateTextValue(current);
          if (translated !== current) element.setAttribute(attribute, translated);
        }
      }
      if (originals?.size) originalAttributes.set(element, originals);
    }
    for (const child of Array.from(element.childNodes)) translateNode(child);
  };

  let scheduled = false;
  const refresh = () => {
    if (scheduled) return;
    scheduled = true;
    window.requestAnimationFrame(() => {
      scheduled = false;
      document.documentElement.lang = effectiveLocale.value;
      translateNode(root);
    });
  };
  const observer = new MutationObserver(refresh);
  observer.observe(root, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: attributes });
  const stopLocaleWatch = watch(effectiveLocale as Ref<AppLocale>, refresh, { immediate: true });
  return () => {
    observer.disconnect();
    stopLocaleWatch();
  };
}
