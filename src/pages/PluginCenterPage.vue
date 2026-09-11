<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NEmpty,
  NModal,
  NSpin,
  NSwitch,
  NTag,
} from "naive-ui";
import { pluginApi, type InstalledPluginRecord } from "../services/plugin-api";

const emit = defineEmits<{ changed: [] }>();

const plugins = ref<InstalledPluginRecord[]>([]);
const loading = ref(false);
const operationId = ref<string | null>(null);
const errorMessage = ref("");
const successMessage = ref("");
const permissionTarget = ref<InstalledPluginRecord | null>(null);
const uninstallTarget = ref<InstalledPluginRecord | null>(null);
const deletePrivateData = ref(false);

const sortedPlugins = computed(() => [...plugins.value].sort((left, right) => left.pluginId.localeCompare(right.pluginId)));

const capabilityLabels: Record<string, string> = {
  "rooms.read": "读取游戏房间",
  "rooms.write": "创建和更新游戏房间",
  "leaderboard.read": "读取排行榜",
  "leaderboard.write": "提交排行榜成绩",
  "storage.private": "读写插件私有数据",
  "theme.read": "读取当前主题",
  "ui.notify": "显示软件通知",
  "ui.filePicker": "选择本地文件",
  "network.lan": "访问局域网",
  "network.internet": "访问互联网",
  "logger.write": "写入诊断日志",
  "native.sidecar": "运行插件附带的本地程序",
};

function pluginName(plugin: InstalledPluginRecord) {
  return plugin.displayName?.trim() || plugin.pluginId;
}

function capabilityLabel(capability: string) {
  return capabilityLabels[capability] ?? capability;
}

async function refresh() {
  loading.value = true;
  errorMessage.value = "";
  try {
    plugins.value = await pluginApi.listInstalled();
  } catch (error) {
    errorMessage.value = `读取插件列表失败：${String(error)}`;
  } finally {
    loading.value = false;
  }
}

async function installLocalPackage() {
  const selected = await openFileDialog({
    multiple: false,
    directory: false,
    title: "选择 LanChat 插件包",
    filters: [{ name: "LanChat 插件", extensions: ["lcp"] }],
  });
  if (!selected || Array.isArray(selected)) return;
  operationId.value = "install";
  errorMessage.value = "";
  successMessage.value = "";
  try {
    const installed = await pluginApi.installPackage(selected, false);
    await refresh();
    permissionTarget.value = installed;
    successMessage.value = `${pluginName(installed)} 已完成签名校验，请确认权限后启用。`;
  } catch (error) {
    errorMessage.value = `安装失败：${String(error)}`;
  } finally {
    operationId.value = null;
  }
}

async function enableAfterPermissionReview() {
  const target = permissionTarget.value;
  if (!target) return;
  operationId.value = target.pluginId;
  errorMessage.value = "";
  try {
    await pluginApi.setPermissions(target.pluginId, target.grantedCapabilities);
    await pluginApi.setEnabled(target.pluginId, true);
    permissionTarget.value = null;
    successMessage.value = `${pluginName(target)} 已启用。`;
    await refresh();
    emit("changed");
  } catch (error) {
    errorMessage.value = `启用失败：${String(error)}`;
  } finally {
    operationId.value = null;
  }
}

async function togglePlugin(plugin: InstalledPluginRecord, enabled: boolean) {
  if (enabled) {
    permissionTarget.value = plugin;
    return;
  }
  operationId.value = plugin.pluginId;
  errorMessage.value = "";
  try {
    await pluginApi.setEnabled(plugin.pluginId, false);
    successMessage.value = `${pluginName(plugin)} 已停用。`;
    await refresh();
    emit("changed");
  } catch (error) {
    errorMessage.value = `停用失败：${String(error)}`;
  } finally {
    operationId.value = null;
  }
}

async function rollback(plugin: InstalledPluginRecord) {
  operationId.value = plugin.pluginId;
  errorMessage.value = "";
  try {
    await pluginApi.rollback(plugin.pluginId);
    successMessage.value = `${pluginName(plugin)} 已回滚，确认权限后可以重新启用。`;
    await refresh();
    emit("changed");
  } catch (error) {
    errorMessage.value = `回滚失败：${String(error)}`;
  } finally {
    operationId.value = null;
  }
}

function requestUninstall(plugin: InstalledPluginRecord) {
  deletePrivateData.value = false;
  uninstallTarget.value = plugin;
}

async function confirmUninstall() {
  const target = uninstallTarget.value;
  if (!target) return;
  operationId.value = target.pluginId;
  errorMessage.value = "";
  try {
    await pluginApi.uninstall(target.pluginId, deletePrivateData.value);
    uninstallTarget.value = null;
    successMessage.value = `${pluginName(target)} 已卸载。`;
    await refresh();
    emit("changed");
  } catch (error) {
    errorMessage.value = `卸载失败：${String(error)}`;
  } finally {
    operationId.value = null;
  }
}

onMounted(refresh);
</script>

<template>
  <section class="plugin-center">
    <header class="plugin-center-hero">
      <div>
        <span class="plugin-center-kicker">扩展 LanChat</span>
        <h3>插件中心</h3>
        <p>安装经过签名验证的独立插件。插件只有在授权并启用后才会出现在软件中。</p>
      </div>
      <NButton type="primary" :loading="operationId === 'install'" @click="installLocalPackage">
        从本地安装 .lcp
      </NButton>
    </header>

    <NAlert v-if="errorMessage" type="error" closable @close="errorMessage = ''">{{ errorMessage }}</NAlert>
    <NAlert v-if="successMessage" type="success" closable @close="successMessage = ''">{{ successMessage }}</NAlert>

    <NSpin :show="loading">
      <div v-if="sortedPlugins.length" class="plugin-card-grid">
        <NCard v-for="plugin in sortedPlugins" :key="plugin.pluginId" size="small" class="plugin-card">
          <div class="plugin-card-main">
            <div class="plugin-card-icon">{{ pluginName(plugin).slice(0, 1) }}</div>
            <div class="plugin-card-copy">
              <div class="plugin-card-title">
                <strong>{{ pluginName(plugin) }}</strong>
                <NTag size="small" :type="plugin.enabled ? 'success' : 'default'">{{ plugin.enabled ? '已启用' : '已停用' }}</NTag>
                <NTag v-if="plugin.signatureKeyId" size="small" type="info">签名已验证</NTag>
                <NTag v-else size="small" type="warning">开发插件</NTag>
              </div>
              <code>{{ plugin.pluginId }}</code>
              <span>版本 {{ plugin.activeVersion }} · {{ plugin.source === 'official' ? '官方来源' : '开发来源' }}</span>
            </div>
            <NSwitch
              :value="plugin.enabled"
              :loading="operationId === plugin.pluginId"
              @update:value="(value) => togglePlugin(plugin, value)"
            />
          </div>
          <div class="plugin-capabilities">
            <span v-for="capability in plugin.grantedCapabilities" :key="capability">{{ capabilityLabel(capability) }}</span>
          </div>
          <div class="plugin-card-actions">
            <NButton size="small" secondary :disabled="!plugin.previousVersion || operationId === plugin.pluginId" @click="rollback(plugin)">回滚</NButton>
            <NButton size="small" secondary type="error" :disabled="operationId === plugin.pluginId" @click="requestUninstall(plugin)">卸载</NButton>
          </div>
        </NCard>
      </div>
      <NEmpty v-else-if="!loading" description="还没有安装插件">
        <template #extra><NButton type="primary" secondary @click="installLocalPackage">选择 .lcp 插件包</NButton></template>
      </NEmpty>
    </NSpin>

    <NModal :show="permissionTarget !== null" preset="card" title="确认插件权限" class="plugin-permission-modal" @update:show="(show) => { if (!show) permissionTarget = null; }">
      <template v-if="permissionTarget">
        <p><strong>{{ pluginName(permissionTarget) }}</strong> 申请以下能力：</p>
        <ul class="permission-list">
          <li v-for="capability in permissionTarget.grantedCapabilities" :key="capability">
            <span>{{ capabilityLabel(capability) }}</span><code>{{ capability }}</code>
          </li>
        </ul>
        <NAlert type="info" :show-icon="false">启用后插件入口才会出现在软件中，之后可以随时停用。</NAlert>
        <div class="modal-actions">
          <NButton @click="permissionTarget = null">暂不启用</NButton>
          <NButton type="primary" :loading="operationId === permissionTarget.pluginId" @click="enableAfterPermissionReview">授权并启用</NButton>
        </div>
      </template>
    </NModal>

    <NModal :show="uninstallTarget !== null" preset="card" title="卸载插件" class="plugin-permission-modal" @update:show="(show) => { if (!show) uninstallTarget = null; }">
      <template v-if="uninstallTarget">
        <p>确定卸载 <strong>{{ pluginName(uninstallTarget) }}</strong>？插件会立即停止运行。</p>
        <NCheckbox v-model:checked="deletePrivateData">同时删除该插件的本地数据</NCheckbox>
        <div class="modal-actions">
          <NButton @click="uninstallTarget = null">取消</NButton>
          <NButton type="error" :loading="operationId === uninstallTarget.pluginId" @click="confirmUninstall">卸载</NButton>
        </div>
      </template>
    </NModal>
  </section>
</template>

<style scoped>
.plugin-center { display: grid; grid-column: 1 / -1; gap: 14px; min-width: 0; }
.plugin-center-hero { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 22px; border: 1px solid var(--panel-border); border-radius: 12px; background: linear-gradient(135deg, color-mix(in srgb, var(--primary-color) 12%, var(--panel-bg)), var(--panel-bg)); }
.plugin-center-hero h3 { margin: 3px 0 7px; font-size: 22px; }
.plugin-center-hero p { max-width: 620px; margin: 0; color: var(--text-secondary); line-height: 1.65; }
.plugin-center-kicker { color: var(--primary-color); font-size: 12px; font-weight: 700; letter-spacing: .08em; }
.plugin-card-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: 12px; }
.plugin-card-main { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; align-items: center; gap: 12px; }
.plugin-card-icon { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 11px; background: color-mix(in srgb, var(--primary-color) 14%, var(--panel-bg)); color: var(--primary-color); font-size: 20px; font-weight: 700; }
.plugin-card-copy { display: grid; min-width: 0; gap: 3px; }
.plugin-card-copy code, .plugin-card-copy > span { overflow: hidden; color: var(--text-secondary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.plugin-card-title { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
.plugin-card-title strong { margin-right: 2px; font-size: 16px; }
.plugin-capabilities { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 14px; }
.plugin-capabilities span { padding: 3px 8px; border-radius: 999px; background: var(--input-bg); color: var(--text-secondary); font-size: 11px; }
.plugin-card-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--panel-border); }
.permission-list { display: grid; gap: 7px; max-height: 300px; margin: 14px 0; padding: 0; overflow: auto; list-style: none; }
.permission-list li { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 9px 11px; border-radius: 7px; background: var(--input-bg); }
.permission-list code { color: var(--text-secondary); font-size: 11px; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
:global(.plugin-permission-modal) { width: min(560px, calc(100vw - 32px)); }
@media (max-width: 720px) {
  .plugin-center-hero { align-items: stretch; flex-direction: column; }
  .plugin-card-grid { grid-template-columns: 1fr; }
}
</style>
