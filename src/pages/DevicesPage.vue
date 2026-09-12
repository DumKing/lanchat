<script setup lang="ts">
import { NAvatar, NButton, NCheckbox, NEmpty, NFormItem, NInput, NList, NListItem, NTag, NText, NThing } from "naive-ui";
import type { ChannelMember, Conversation, Peer } from "../types/lanchat";
import { peerDisplayName, sameDeviceId } from "../utils/peerPresentation";

defineProps<{
  selectedPeer: Peer | null;
  selectedChannel: Conversation | null;
  channelOwnerName: string;
  channelMembers: Array<ChannelMember | Peer>;
  profileDeviceId?: string;
  superAdminEnabled: boolean;
  canManageChannel: boolean;
  avatarImage: (avatar?: string | null) => string | undefined;
  firstLetter: (value?: string) => string;
  peerClientKindLabel: (peer: Peer) => string;
  peerBuildVersionLabel: (peer: Peer) => string;
  peerBuildTimeLabel: (peer: Peer) => string;
  peerSupportsFullFeatures: (peer: Peer) => boolean;
  peerLastSeenLabel: (peer: Peer) => string;
  memberDisplayName: (member: ChannelMember | Peer) => string;
  memberSubtitle: (member: ChannelMember | Peer) => string;
  formatTime: (timestamp: number) => string;
}>();

const peerNoteDraft = defineModel<string>("peerNoteDraft", { required: true });
const adminNicknameDraft = defineModel<string>("adminNicknameDraft", { required: true });
const adminNicknameLockAfterIssue = defineModel<boolean>("adminNicknameLockAfterIssue", { required: true });

const emit = defineEmits<{
  savePeerNote: [];
  adminRename: [];
  adminUseSystemUsername: [];
  adminUnlockNickname: [];
  openSimulation: [];
  startDirectChat: [peer: Peer];
  deletePeer: [];
  openMember: [member: ChannelMember | Peer];
  enterChannel: [];
  inviteMembers: [];
  dissolveChannel: [];
}>();
</script>

<template>
  <section class="workspace-view device-address-book">
    <div class="workspace-header">
      <h2>设备通讯录</h2>
      <p>左侧选择设备或频道，在这里查看详细信息和操作。</p>
    </div>
    <div class="device-detail-shell">
      <div v-if="selectedPeer" class="device-profile-panel">
        <div class="device-detail-head large">
          <img v-if="avatarImage(selectedPeer.avatar)" class="avatar-image peer-avatar large-avatar" :src="avatarImage(selectedPeer.avatar)" alt="设备头像" />
          <NAvatar v-else :size="56" class="peer-avatar">{{ firstLetter(peerDisplayName(selectedPeer)) }}</NAvatar>
          <div>
            <h3>{{ peerDisplayName(selectedPeer) }}</h3>
            <p><span class="presence-dot" :class="{ online: selectedPeer.online }"></span>{{ selectedPeer.online ? "在线" : "离线" }}</p>
          </div>
        </div>
        <div class="device-detail-grid wide">
          <span>IP 地址</span><strong>{{ selectedPeer.address }}</strong>
          <span>端口</span><strong>{{ selectedPeer.port }}</strong>
          <span>MAC 地址</span><strong>{{ selectedPeer.device_id }}</strong>
          <span>昵称</span><strong>{{ selectedPeer.nickname }}</strong>
          <span>昵称限制</span><strong>{{ selectedPeer.nickname_locked ? "禁止本地修改" : "允许本地修改" }}</strong>
          <span>客户端</span><strong>{{ peerClientKindLabel(selectedPeer) }}</strong>
          <span>软件版本</span><strong>{{ peerBuildVersionLabel(selectedPeer) }}</strong>
          <span>构建时间</span><strong>{{ peerBuildTimeLabel(selectedPeer) }}</strong>
          <span>支持能力</span><strong>{{ peerSupportsFullFeatures(selectedPeer) ? "告警、聊天、频道、游戏、文件" : "桌宠告警" }}</strong>
          <span>最近在线</span><strong>{{ peerLastSeenLabel(selectedPeer) }}</strong>
        </div>
        <div class="device-note-editor">
          <NFormItem label="设备备注" :show-feedback="false">
            <NInput v-model:value="peerNoteDraft" maxlength="32" clearable placeholder="仅保存在本机，用于识别设备" @keyup.enter="emit('savePeerNote')" />
          </NFormItem>
          <NButton secondary type="primary" @click="emit('savePeerNote')">保存备注</NButton>
        </div>
        <div v-if="superAdminEnabled" class="admin-rename-box">
          <NFormItem label="超管修改设备昵称">
            <NInput v-model:value="adminNicknameDraft" maxlength="24" clearable />
          </NFormItem>
          <NCheckbox v-model:checked="adminNicknameLockAfterIssue">下发后禁止对方本地修改昵称</NCheckbox>
          <NButton block type="warning" :disabled="!selectedPeer.online || !adminNicknameDraft.trim()" @click="emit('adminRename')">下发昵称修改</NButton>
          <NButton block secondary type="primary" :disabled="!selectedPeer.online" @click="emit('adminUseSystemUsername')">改为电脑登录用户名</NButton>
          <NButton block secondary type="warning" :disabled="!selectedPeer.online || !adminNicknameDraft.trim()" @click="emit('adminUnlockNickname')">解除昵称修改限制</NButton>
          <NButton block type="warning" @click="emit('openSimulation')">超管模拟发送</NButton>
          <NText depth="3">目标设备在线时会立即更新本机昵称，并通过在线广播同步给局域网。</NText>
        </div>
        <div class="device-detail-actions">
          <NButton type="primary" :disabled="!selectedPeer.online && peerSupportsFullFeatures(selectedPeer)" @click="emit('startDirectChat', selectedPeer)">
            {{ peerSupportsFullFeatures(selectedPeer) ? "发起单聊" : "查看历史" }}
          </NButton>
          <NButton secondary type="error" @click="emit('deletePeer')">删除设备</NButton>
        </div>
      </div>
      <div v-else-if="selectedChannel" class="device-profile-panel">
        <div class="device-detail-head large">
          <NAvatar :size="56" class="conversation-avatar">{{ selectedChannel.is_private ? "私" : "局" }}</NAvatar>
          <div>
            <h3>{{ selectedChannel.title }}</h3>
            <p>{{ selectedChannel.is_private ? "私有加密频道" : "局域网公开频道" }}</p>
          </div>
        </div>
        <div class="device-detail-grid wide">
          <span>频道类型</span><strong>{{ selectedChannel.is_private ? "私有频道" : "公开频道" }}</strong>
          <span>创建人</span><strong>{{ channelOwnerName }}</strong>
          <span>频道 ID</span><strong>{{ selectedChannel.id }}</strong>
          <span>成员数量</span><strong>{{ channelMembers.length }}</strong>
          <span>更新时间</span><strong>{{ formatTime(selectedChannel.updated_at) }}</strong>
        </div>
        <div class="channel-detail-members">
          <div class="channel-detail-title"><strong>频道成员</strong><span>{{ channelMembers.length }} 人</span></div>
          <NEmpty v-if="channelMembers.length === 0" description="暂无成员" class="list-empty compact" />
          <NList v-else hoverable clickable class="channel-member-list embedded">
            <NListItem v-for="member in channelMembers" :key="member.device_id" class="device-item" :class="{ 'is-offline': !sameDeviceId(member.device_id, profileDeviceId) && !member.online }" @click="emit('openMember', member)">
              <NThing :title="memberDisplayName(member)" :description="memberSubtitle(member)">
                <template #avatar>
                  <img v-if="avatarImage(member.avatar)" class="avatar-image peer-avatar" :src="avatarImage(member.avatar)" alt="成员头像" />
                  <NAvatar v-else class="peer-avatar">{{ firstLetter(memberDisplayName(member)) }}</NAvatar>
                </template>
                <template #header-extra><NTag v-if="'is_owner' in member && member.is_owner" size="small" :bordered="false" type="warning">群主</NTag></template>
              </NThing>
            </NListItem>
          </NList>
        </div>
        <div class="device-detail-actions">
          <NButton secondary @click="emit('enterChannel')">进入频道</NButton>
          <NButton v-if="canManageChannel" type="primary" @click="emit('inviteMembers')">邀请成员</NButton>
          <NButton v-if="canManageChannel" secondary type="error" @click="emit('dissolveChannel')">解散频道</NButton>
        </div>
      </div>
      <NEmpty v-else description="从左侧选择设备或频道" class="device-detail-empty">
        <template #extra><NText depth="3">设备会自动发现；频道包含局域网公开频道和私有频道。</NText></template>
      </NEmpty>
    </div>
  </section>
</template>
