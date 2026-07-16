const { chromium } = require("playwright");
const fs = require("node:fs");
const path = require("node:path");

async function capture(browser, width, height) {
  const page = await browser.newPage({ viewport: { width, height } });
  await page.goto("http://127.0.0.1:1421/", { waitUntil: "networkidle" });
  await page.locator('button[title="设置"]').click();
  const basicSettingsButton = page.getByRole("button", { name: "基础设置", exact: true });
  const petSettingsButton = page.getByRole("button", { name: "桌宠设置", exact: true });
  await basicSettingsButton.waitFor();
  if (await page.locator(".desktop-pet-settings-card").count()) {
    throw new Error("基础设置中不应展示桌宠设置卡片");
  }
  await petSettingsButton.click();
  await page.getByText("桌宠与青蛙告警器", { exact: true }).waitFor();
  const metrics = await page.locator(".desktop-pet-settings-card").evaluate((element) => ({
    width: element.getBoundingClientRect().width,
    right: element.getBoundingClientRect().right,
    viewport: document.documentElement.clientWidth,
    bodyOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
  }));
  if (metrics.bodyOverflow || metrics.right > metrics.viewport + 1 || metrics.width < 240) {
    throw new Error(`桌宠设置布局溢出：${JSON.stringify(metrics)}`);
  }
  const output = path.join("artifacts", `desktop-pet-settings-subnav-${width}x${height}.png`);
  await page.screenshot({ path: output, fullPage: false });
  await page.close();
  return { output, metrics };
}

async function main() {
  fs.mkdirSync("artifacts", { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    executablePath: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  });
  try {
    const results = [];
    results.push(await capture(browser, 1280, 800));
    results.push(await capture(browser, 1024, 768));
    console.log(JSON.stringify(results, null, 2));
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
