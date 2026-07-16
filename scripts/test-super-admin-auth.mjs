import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const appVue = await readFile("src/App.vue", "utf8");
const pkg = await readFile("package.json", "utf8");

assert.match(pkg, /"crypto-js"/, "应使用开源 crypto-js 处理 MD5");
assert.match(appVue, /import CryptoJS from "crypto-js"/, "App 应调用 crypto-js 而不是自写 MD5");
assert.match(appVue, /SUPER_ADMIN_PASSWORD_MD5\s*=\s*"D7B9AF919901FA1598BDC21465E3EB3F"/, "超管密码应使用给定 MD5 值");
assert.match(appVue, /CryptoJS\.MD5\(superAdminPasswordDraft\.value\)\.toString\(\)\.toUpperCase\(\)/, "密码输入应先用 crypto-js MD5 再忽略大小写比较");
assert.match(appVue, /superAdminAuthOpen = ref\(false\)/, "连续点击后应打开密码验证弹窗");
assert.match(appVue, /NModal v-model:show="superAdminAuthOpen"/, "应有超管密码验证弹窗");
assert.match(appVue, /type="password"/, "超管验证输入框应使用密码类型");
assert.match(appVue, /disableSuperAdmin/, "超管开启后应支持关闭");
assert.match(appVue, /if \(superAdminEnabled\.value\) \{\s*disableSuperAdmin\(\);/, "已开启超管时再次点击入口应关闭");
assert.doesNotMatch(appVue, /superAdminEnabled\.value = true;\s*if \(typeof window !== "undefined"\) window\.localStorage\.setItem\("lanchat-super-admin-enabled", "true"\);/, "连续点击不应再绕过密码直接开启");

console.log("super admin auth checks passed");
