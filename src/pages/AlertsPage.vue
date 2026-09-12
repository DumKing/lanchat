<script setup lang="ts">
import { NButton, NCard, NInput, NSpace, NTag, NText } from "naive-ui";
import { alertTruthScore } from "../utils/alertCredibility";
import type { PetAlertMode, SimulationMeta } from "../types/lanchat";
import type { AlertRecord } from "../features/alerts/types";
type AlertRankingRow = {
  deviceId: string;
  nickname: string;
  probability: number | null;
  real: number;
  falseCount: number;
  total: number;
};

defineProps<{
  profileDeviceId?: string;
  alertMode: PetAlertMode;
  rankingRows: AlertRankingRow[];
  alertRecords: AlertRecord[];
  now: number;
  simulationLabel: (simulation?: SimulationMeta | null) => string;
  probabilityLabel: (alert: AlertRecord) => string;
  formatTime: (timestamp: number) => string;
}>();

const quickAlertDraft = defineModel<string>("quickAlertDraft", { required: true });
const emit = defineEmits<{ send: [mode: PetAlertMode] }>();
</script>

<template>
  <section class="workspace-view alert-dashboard-view">
    <div class="workspace-header">
      <h2>狼来了排行榜</h2>
      <p>按别人反馈后的真实概率排行，真实概率也会作为桌宠温度展示。</p>
    </div>
    <div class="alert-dashboard-grid">
      <NCard title="呱呱告警" size="small" class="quick-alert-card">
        <NSpace vertical>
          <NText depth="3">双击桌面桌宠也可以发送呱呱告警。当前告警会广播给在线设备，后续接入 LanChat Hub 后由 Hub 转发。</NText>
          <NInput v-model:value="quickAlertDraft" maxlength="60" clearable placeholder="例如：快来处理一下" />
          <NButton type="error" block @click="emit('send', alertMode)">{{ quickAlertDraft || "呱呱~呱~~" }}</NButton>
        </NSpace>
      </NCard>
      <NCard title="狼来了排行" size="small" class="alert-rank-card">
        <div class="alert-rank-list">
          <div class="alert-rank-head">
            <span>名次</span><span>人员</span><span>真实度</span><span>反馈</span>
          </div>
          <div v-if="rankingRows.length === 0" class="leaderboard-empty">暂无告警反馈，收到或发送告警后会出现在这里</div>
          <div v-for="(row, index) in rankingRows" :key="row.deviceId" class="alert-rank-row">
            <span class="leaderboard-rank">{{ index + 1 }}</span>
            <strong>{{ row.deviceId === profileDeviceId ? `我 · ${row.nickname}` : row.nickname }}</strong>
            <span class="alert-temperature">{{ row.probability === null ? "待确认" : `${row.probability}%` }}</span>
            <small>{{ row.real }} 真 / {{ row.falseCount }} 假 · {{ row.total }} 次告警</small>
          </div>
        </div>
      </NCard>
      <NCard title="最近告警" size="small" class="alert-history-card">
        <div class="alert-history-list">
          <div v-if="alertRecords.length === 0" class="leaderboard-empty">暂无告警记录</div>
          <div v-for="alert in alertRecords.slice(0, 10)" :key="alert.alertId" class="alert-history-row" :class="{ pending: alert.incoming && !alert.handled }">
            <div>
              <strong>{{ alert.senderDeviceId === profileDeviceId ? `我 · ${alert.senderNickname}` : alert.senderNickname }}</strong>
              <span>{{ alert.content }}</span>
              <NTag v-if="simulationLabel(alert.simulation)" size="tiny" :bordered="false" type="warning">{{ simulationLabel(alert.simulation) }}</NTag>
            </div>
            <div class="alert-history-meta">
              <NTag size="small" :bordered="false" :type="alertTruthScore(alert, now).feedbackCount === 0 ? 'default' : alertTruthScore(alert, now).probability >= 60 ? 'success' : 'error'">
                {{ probabilityLabel(alert) }}
              </NTag>
              <small>{{ formatTime(alert.createdAt) }}</small>
            </div>
          </div>
        </div>
      </NCard>
    </div>
  </section>
</template>
