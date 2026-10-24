import { chromium } from "playwright";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const FPS = 30;
const DURATION = 10;
const WIDTH = 1760;
const HEIGHT = 520;
const KEYFRAME_TIMES = [0, 1.8, 3.8, 5.2, 7.0, 8.5];

const here = import.meta.dir;
const docsDir = resolve(here, "..");

function run(cmd: string[]) {
  const proc = Bun.spawnSync(cmd, { stdout: "inherit", stderr: "inherit" });
  if (proc.exitCode !== 0) throw new Error(`${cmd[0]} exited with ${proc.exitCode}`);
}

const keyframeFlag = process.argv.indexOf("--keyframes");
const keyframeDir = keyframeFlag >= 0 ? process.argv[keyframeFlag + 1] : undefined;
if (keyframeFlag >= 0 && !keyframeDir) throw new Error("--keyframes needs an output directory");

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({
    viewport: { width: WIDTH, height: HEIGHT },
    deviceScaleFactor: 2,
  });
  await page.goto(pathToFileURL(join(here, "index.html")).href);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForFunction(() => typeof (window as any).seek === "function");

  async function shoot(t: number, path: string) {
    await page.evaluate((time) => (window as any).seek(time), t);
    await page.screenshot({ path, type: "png" });
  }

  if (keyframeDir) {
    mkdirSync(keyframeDir, { recursive: true });
    for (const t of KEYFRAME_TIMES) await shoot(t, join(keyframeDir, `t${t.toFixed(1)}.png`));
    console.log(`wrote ${KEYFRAME_TIMES.length} keyframes to ${keyframeDir}`);
  } else {
    const frames = mkdtempSync(join(tmpdir(), "tsbars-hero-"));
    try {
      const total = FPS * DURATION;
      for (let frame = 0; frame < total; frame++) {
        await shoot(frame / FPS, join(frames, `${String(frame).padStart(4, "0")}.png`));
      }
      const input = ["-framerate", String(FPS), "-i", join(frames, "%04d.png")];
      run([
        "ffmpeg", "-y", "-loglevel", "error", ...input,
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart",
        join(docsDir, "hero.mp4"),
      ]);
      run([
        "ffmpeg", "-y", "-loglevel", "error", ...input,
        "-vf", "fps=20,scale=880:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
        "-loop", "0",
        join(docsDir, "hero.gif"),
      ]);
    } finally {
      rmSync(frames, { recursive: true, force: true });
    }
  }
} finally {
  await browser.close();
}
