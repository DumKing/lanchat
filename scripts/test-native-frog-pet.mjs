import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";

const native = fs.readFileSync("src-tauri/src/native_frog_pet.rs", "utf8");
const lib = fs.readFileSync("src-tauri/src/lib.rs", "utf8");
const cargo = fs.readFileSync("src-tauri/Cargo.toml", "utf8");
const refreshedAssets = [
  "public/pet-assets/preview-candidates/33-angry-green.png",
  "public/pet-assets/preview-candidates/34-idle-look-up.png",
  "public/pet-assets/preview-candidates/35-idle-look-down.png",
  "public/pet-assets/preview-candidates/36-idle-sway.png",
];

for (const asset of refreshedAssets) {
  assert.ok(fs.existsSync(asset), `缺少新动作素材：${asset}`);
}

const imageDigest = (path) => crypto.createHash("sha256").update(fs.readFileSync(path)).digest("hex");
assert.notEqual(
  imageDigest("public/pet-assets/preview-candidates/33-angry-green.png"),
  imageDigest("public/pet-assets/preview-candidates/32-alert-pulse-original-style.png"),
  "普通报警的生气帧必须是独立素材，不能复用告警帧",
);

assert.match(cargo, /eframe\s*=\s*(?:\{\s*version\s*=\s*)?"0\.28/,
  "Rust 桌宠应使用 eframe/egui 原生绘制");
assert.match(lib, /mod native_frog_pet;/, "Tauri 应注册原生桌宠模块");
assert.match(lib, /NativeFrogPetController/, "Tauri 应持有原生桌宠控制器");
assert.match(native, /eframe::run_native/, "桌宠应通过 eframe 创建原生窗口");
assert.match(native, /with_transparent\(true\)/, "原生桌宠窗口应透明");
assert.match(native, /FROG_SHEET_BYTES[\s\S]{0,120}frog-3d-actions-alpha\.png/, "桌宠应内嵌包含完整动作池的 3D 青蛙透明 PNG 图集");
assert.match(native, /FROG_ACTION_COLUMNS: u32 = 4[\s\S]{0,80}FROG_ACTION_ROWS: u32 = 4/, "桌宠主动作图集应按 4x4 切分");
assert.match(native, /load_frog_textures/, "桌宠应从 3D 青蛙图集加载姿态纹理");
assert.match(native, /ctx\.load_texture/, "3D 青蛙姿态应加载为 egui 纹理");
assert.match(native, /painter\.image/, "青蛙本体应通过图片纹理绘制");
assert.match(native, /frog_pose_index/, "桌宠应按状态选择不同青蛙姿态");
assert.match(native, /Wheel|scroll_delta|MouseWheel/, "桌宠应支持滚轮缩放本体");
assert.match(native, /StartDrag/, "无边框桌宠应支持拖动窗口");
assert.match(native, /with_any_thread\(true\)/, "Windows 子线程原生窗口应允许事件循环运行");
assert.match(native, /latest_sender|pending_count|temperature/, "桌宠状态应能显示告警信息");
assert.match(native, /latest_sender_address/, "告警详情应显示发送人 IP");
assert.match(native, /latest_created_at/, "告警详情应显示消息时间戳");
assert.match(native, /theme_accent/, "原生桌宠状态应接收当前主题色");
assert.match(native, /native_frog_action/, "桌宠交互应回传主应用处理反馈和告警");
assert.match(native, /request_repaint/, "状态更新后应主动唤醒原生桌宠重绘");
assert.match(native, /FontDefinitions|set_fonts/, "桌宠应加载中文字体，避免详情文字乱码");
assert.doesNotMatch(native, /InnerSize\(Vec2::new\(420\.0, 320\.0\)\)/, "展开详情不应自动放大桌宠窗口");
assert.doesNotMatch(native, /Color32::from_rgba_unmultiplied\(24, 27, 31, 205\)/, "详情面板不应使用近黑背景");
assert.doesNotMatch(native, /Pos2::new\(available\.x \* 0\.5, 148\.0\)/, "温度坐标不应写死在窗口底部");
assert.match(native, /state\.disco[\s\S]{0,240}elapsed/, "蹦迪模式应有随时间变化的全屏位置");
assert.match(native, /FROG_DISCO_SHEET_BYTES[\s\S]{0,160}frog-disco-side-hop-v2-alpha\.png/, "蹦迪模式应加载修正后的侧身蹲跳青蛙图集");
assert.match(native, /draw_disco_frog_image/, "蹦迪模式应按移动方向绘制青蛙图片");
assert.match(native, /const DISCO_HOP_SECONDS: f32 = 0\.72/, "蹦迪移动应放慢到可看清跃起动作的节奏");
assert.match(native, /DISCO_HOPS_PER_LEG/, "蹦迪移动应分段向左或向右跳");
assert.match(native, /moving_right[\s\S]{0,180}disco_direction/, "蹦迪青蛙图片方向应跟随移动方向");
assert.match(native, /jump_arc[\s\S]{0,260}OuterPosition/, "蹦迪窗口移动应带跳跃弧线");
assert.match(native, /DISCO_CROUCH_OUT_END/, "蹦迪应包含侧身蹲下蓄力阶段");
assert.match(native, /DISCO_LEAP_END/, "蹦迪应包含跃出落地阶段");
assert.match(native, /movement_progress[\s\S]{0,320}DISCO_CROUCH_OUT_END/, "蹦迪移动应先蹲住再跃出");
assert.match(native, /direction if direction < 0 => if crouching \{ 0 \} else \{ 1 \}/, "向左蹦迪应使用左蹲和左跃两帧");
assert.match(native, /direction if direction > 0 => if crouching \{ 2 \} else \{ 3 \}/, "向右蹦迪应使用右蹲和右跃两帧");
assert.match(native, /draw_disco_frog_image\(ui\.painter\(\), frog_rect, disco_hop_progress\)/, "蹦迪绘制应按 hop 阶段切换蹲姿和跃出");
assert.match(native, /const DETAIL_WIDTH: f32 = 236\.0/, "详情面板宽度应更窄");
assert.match(native, /const DETAIL_HEIGHT: f32 = 88\.0/, "详情面板高度应略微增加，避免文案和按钮重叠");
assert.match(native, /detail_last_open: bool/, "详情应记录展开状态，用于窗口位置补偿");
assert.match(native, /detail_side: i8/, "详情应记录显示在青蛙左侧或右侧");
assert.match(native, /let detail_space = if detail_is_open \{ DETAIL_WIDTH \+ DETAIL_GAP \} else \{ 0\.0 \}/, "详情打开时应扩展透明窗口为弹窗预留空间");
assert.match(native, /OuterPosition\(Pos2::new\(\s*outer_rect\.min\.x - detail_transition_space/, "详情在左侧展开时应左移窗口抵消扩宽");
assert.match(native, /pet_origin_x = if detail_is_open && self\.detail_side < 0 \{ detail_space \} else \{ 0\.0 \}/, "左侧详情展开时青蛙应在扩展画布内右移以保持屏幕位置");
assert.match(native, /draw_alert_details/, "详情应绘制在主透明窗口内，避免独立窗口黑角");
assert.match(native, /timestamp_millis_opt/, "详情时间戳应格式化为本地时间");
assert.doesNotMatch(native, /show_viewport_immediate/, "详情不应再使用独立 viewport，避免圆角黑底");
assert.doesNotMatch(native, /with_title\("告警详情"\)/, "详情不应再创建独立标题窗口");
assert.match(native, /ALERT_POSES: \[usize; 3\] = \[POSE_SURPRISE, POSE_ANGRY, POSE_ALERT\]/, "普通报警应使用惊讶、生气、告警三张图循环");
assert.match(native, /POSE_LOOK_RIGHT[\s\S]{0,260}POSE_LOOK_LEFT/, "待机状态应包含左右偏头观察动作");
assert.match(native, /POSE_BREATHE_A[\s\S]{0,240}POSE_BREATHE_B[\s\S]{0,240}POSE_BREATHE_C/, "待机状态应包含呼吸动作池");
assert.doesNotMatch(native, /POSE_SOFT_SMILE|POSE_CHEER|POSE_COOL_DANCE|POSE_NERVOUS|POSE_HOP_UP|POSE_HOP_FALL|POSE_DANCE_POINT/, "待机循环不应混入异风格或生硬跳跃动作");
assert.doesNotMatch(native, /POSE_HAPPY|POSE_SHY|POSE_PARTY/, "待机循环不应再包含开心、害羞或轻蹦迪动作");
assert.match(native, /POSE_LOOK_UP[\s\S]{0,260}POSE_LOOK_DOWN[\s\S]{0,260}POSE_SWAY/, "待机循环应包含抬头、低头和轻晃微动作");
assert.match(native, /17\.0\.\.18\.0[\s\S]{0,100}POSE_LOOK_UP/, "待机循环应在独立时间段播放抬头动作");
assert.match(native, /18\.5\.\.19\.5[\s\S]{0,100}POSE_LOOK_DOWN/, "待机循环应在独立时间段播放低头动作");
assert.match(native, /21\.0\.\.22\.0[\s\S]{0,100}POSE_SWAY/, "待机循环应在独立时间段播放轻晃动作");
assert.match(native, /fn fitted_texture_rect/, "桌宠图片绘制应按原图比例居中，避免不同动作被拉伸变形");
assert.doesNotMatch(native, /draw_thumbs_up_paw|draw_middle_finger_paw|thumbs-up-paw|thumbs-down-paw/, "不应保留爪子快捷反馈");
assert.doesNotMatch(native, /draw_frog_image_with_tint|alert_flash_color/, "普通报警应使用青蛙原始图片颜色，不应整图染红");
assert.match(native, /initial_positioned: bool/, "桌宠应记录首次定位状态");
assert.match(native, /ViewportCommand::OuterPosition\(Pos2::new\(x, y\)\)/, "桌宠应能主动定位到屏幕右下角");
assert.match(native, /Color32::from_rgba_unmultiplied[\s\S]{0,220}232/, "详情面板背景应为半透明主题浅色");
assert.match(native, /alert_detail_background/, "详情面板背景应基于主题色计算");
assert.match(native, /rect_filled\(panel, 12\.0/, "详情面板应使用圆角");
assert.match(native, /\{\}：\{\}|latest_created_at/, "详情信息应按昵称 IP、消息标题、反馈与时间戳布局");
assert.match(native, /sender_line[\s\S]{0,220}title[\s\S]{0,260}created_at/, "详情应按第一行昵称 IP、第二行提示语、第三行按钮和时间戳布局");
assert.match(native, /detail_panel_rect[\s\S]{0,260}frog_rect/, "详情弹窗应以青蛙本体为基准悬浮定位");
assert.doesNotMatch(native, /时间错|昵称：/, "详情中不应再显示时间错或昵称前缀");
assert.doesNotMatch(native, /告警详情[\s\S]{0,900}Color32::from_rgba_unmultiplied\(0, 0, 0/, "详情窗口透明区域不应绘制黑色背景");
assert.doesNotMatch(native, /center \+ Vec2::new\(0\.0, 14\.0 \* scale\)[\s\S]{0,180}circle_filled/, "告警青蛙不应再额外绘制红色圆形背景");
assert.match(native, /draw_bold_temperature/, "温度文字应使用加粗绘制");
assert.match(native, /pending_count == 0[\s\S]{0,120}details_open = false/, "未处理告警归零后应关闭详情展示");
assert.match(native, /ViewportCommand::Visible\(enabled\)/, "桌宠开关重新打开时应主动显示原生窗口");
assert.match(native, /modifiers\.ctrl[\s\S]{0,220}broadcast_disco_alert/, "Ctrl 双击青蛙应触发全员蹦迪告警");
console.log("native frog pet checks passed");
