<script setup lang="ts">
import { NAlert, NButton, NCard, NInput, NProgress, NSpace, NSwitch, NTag, NText } from "naive-ui";
import type { PreviewMediaCacheInfo, UpdateCheckResult, UpdateGithubTokenInfo } from "../../types/lanchat";

defineProps<{
  autostartEnabled: boolean;
  autostartLoading: boolean;
  networkRepairDescription: string;
  canRepairWindowsNetwork: boolean;
  networkRepairing: boolean;
  networkRepairStatus: string;
  previewMediaCacheInfo: PreviewMediaCacheInfo | null;
  previewMediaCacheClearing: boolean;
  localVersionLabel: string;
  updateStatusType: "default" | "error" | "primary" | "info" | "success" | "warning";
  updateStatusLabel: string;
  updateInfo: UpdateCheckResult | null;
  nativeUpdateInstalling: boolean;
  nativeUpdateProgressPercent: number;
  nativeUpdateProgressLabel: string;
  updateChecking: boolean;
  preferredUpdateUrl: string;
  updateGithubTokenInfo: UpdateGithubTokenInfo | null;
  updateGithubTokenSaving: boolean;
  formatFileSize: (bytes?: number | null) => string;
  formatDateTime: (timestamp?: number | null) => string;
  updateNotesPreview: (notes?: string | null) => string;
}>();

const updateGithubTokenDraft = defineModel<string>("updateGithubTokenDraft", { required: true });

const emit = defineEmits<{
  updateAutostart: [enabled: boolean];
  repairNetwork: [];
  requestCallPermission: [media: "audio" | "video"];
  clearImageCache: [];
  checkUpdates: [];
  downloadUpdate: [];
  openReleasePage: [];
  saveUpdateToken: [];
  clearUpdateToken: [];
}>();
</script>

<template>
  <NCard title="启动设置" size="small">
    <div class="setting-switch-row compact-setting-row">
      <div>
        <strong>开机自动启动 LanChat</strong>
        <p>登录 Windows 后自动启动软件。首次安装默认开启，关闭后会保留你的选择。</p>
      </div>
      <NSwitch :value="autostartEnabled" :loading="autostartLoading" @update:value="emit('updateAutostart', $event)" />
    </div>
  </NCard>
  <NCard title="网络修复" size="small" class="basic-network-repair-card">
    <NSpace vertical>
      <NText depth="3">{{ networkRepairDescription }}</NText>
      <NText v-if="canRepairWindowsNetwork" depth="3">会请求管理员权限，并放行 LanChat.exe、TCP 18145、UDP 18146、UDP 5353。</NText>
      <NButton v-if="canRepairWindowsNetwork" block type="primary" :loading="networkRepairing" @click="emit('repairNetwork')">网络修复</NButton>
      <NAlert v-if="networkRepairStatus" type="success" title="已打开修复窗口">{{ networkRepairStatus }}</NAlert>
    </NSpace>
  </NCard>
  <NCard title="通话权限" size="small">
    <NSpace vertical>
      <NText depth="3">首次发起或接听通话会自动申请权限；若此前拒绝，可在这里重新授权。</NText>
      <NSpace>
        <NButton secondary type="primary" @click="emit('requestCallPermission', 'audio')">重新授权麦克风</NButton>
        <NButton secondary type="primary" @click="emit('requestCallPermission', 'video')">重新授权摄像头</NButton>
      </NSpace>
    </NSpace>
  </NCard>
  <NCard title="图片缓存" size="small" class="basic-image-cache-card">
    <NSpace vertical>
      <NText depth="3">带预览能力的图片会自动下载到本机缓存，聊天历史仍可在发送方离线后查看。</NText>
      <div class="update-info-grid">
        <span>缓存文件</span><strong>{{ previewMediaCacheInfo?.fileCount ?? 0 }} 个</strong>
        <span>占用空间</span><strong>{{ formatFileSize(previewMediaCacheInfo?.totalBytes) }}</strong>
      </div>
      <NButton secondary type="warning" :loading="previewMediaCacheClearing" @click="emit('clearImageCache')">清理图片缓存</NButton>
    </NSpace>
  </NCard>
  <NCard title="版本更新" size="small">
    <NSpace vertical>
      <div class="setting-switch-row">
        <div>
          <strong>自动检查更新</strong>
          <p>每次启动都会检查，运行期间每 12 小时复检一次。强制更新版本必须安装后才能继续使用。</p>
        </div>
        <NTag type="success" :bordered="false">已启用</NTag>
      </div>
      <div class="update-info-grid">
        <span>当前版本</span><strong>{{ localVersionLabel }}</strong>
        <span>检查状态</span><NTag size="small" :type="updateStatusType" :bordered="false">{{ updateStatusLabel }}</NTag>
        <span>最新版本</span><strong>{{ updateInfo?.latestVersion ?? "未知" }}</strong>
        <span>上次检查</span><strong>{{ formatDateTime(updateInfo?.checkedAt) }}</strong>
      </div>
      <NAlert v-if="updateInfo?.updateAvailable" type="warning" title="发现新版本">{{ updateInfo.title }}</NAlert>
      <pre v-if="updateInfo" class="update-notes compact">{{ updateNotesPreview(updateInfo.notes) }}</pre>
      <div v-if="nativeUpdateInstalling" class="update-progress-panel compact">
        <NProgress type="line" :percentage="nativeUpdateProgressPercent" :height="8" processing />
        <span>{{ nativeUpdateProgressLabel }}</span>
      </div>
      <NSpace>
        <NButton type="primary" :loading="updateChecking" @click="emit('checkUpdates')">检查更新</NButton>
        <NButton secondary :disabled="!preferredUpdateUrl" @click="emit('downloadUpdate')">下载更新</NButton>
        <NButton quaternary :disabled="!updateInfo" @click="emit('openReleasePage')">Release 页面</NButton>
      </NSpace>
    </NSpace>
    <div class="update-token-section">
      <strong>GitHub API Token</strong>
      <NText depth="3">配置仅用于读取 LanChat 的 GitHub Release，减少匿名 API 限流。Token 会保存在系统凭据库，不会同步到局域网设备。</NText>
      <NAlert v-if="updateGithubTokenInfo?.configured" type="success" :show-icon="false">{{ updateGithubTokenInfo.maskedValue || "GitHub Token 已配置" }}</NAlert>
      <NInput v-model:value="updateGithubTokenDraft" type="password" show-password-on="click" placeholder="粘贴 GitHub Fine-grained Token（仅需 Contents: Read）" @keyup.enter="emit('saveUpdateToken')" />
      <NSpace>
        <NButton type="primary" :disabled="!updateGithubTokenDraft.trim()" :loading="updateGithubTokenSaving" @click="emit('saveUpdateToken')">保存 Token</NButton>
        <NButton v-if="updateGithubTokenInfo?.configured" secondary type="error" :loading="updateGithubTokenSaving" @click="emit('clearUpdateToken')">清除 Token</NButton>
      </NSpace>
    </div>
  </NCard>
</template>
