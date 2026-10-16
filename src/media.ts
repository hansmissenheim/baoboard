// Formats the backend can't decode, turned into frames by the webview.

export type Frames = { width: number; height: number; delays: number[]; pixels: Uint8Array };

// GIF delays tick in 10 ms, so 50 ms frames stay exact.
const VIDEO_FPS = 20;
// ponytail: raw RGBA crosses the IPC, so long or large clips are capped; stream frames if that bites.
const MAX_FRAMES = 10 * VIDEO_FPS;
const MAX_SIDE = 512;

/** Which webview-decoded format the bytes hold, from their ISO BMFF `ftyp` box. */
export function kind(bytes: Uint8Array): "avif" | "mp4" | undefined {
  const box = String.fromCharCode(...bytes.subarray(4, 12));
  if (!box.startsWith("ftyp")) return undefined;
  return box === "ftypavif" || box === "ftypavis" ? "avif" : "mp4";
}

/** The frames of an MP4 or AVIF, or `undefined` when the backend reads the bytes itself. */
export async function toFrames(bytes: Uint8Array<ArrayBuffer>): Promise<Frames | undefined> {
  const type = kind(bytes);
  if (type === "mp4") return videoFrames(new Blob([bytes]));
  // WKWebView has no ImageDecoder, and on macOS the backend reads AVIF through ImageIO.
  if (type === "avif" && "ImageDecoder" in window) return imageFrames(bytes);
  return undefined;
}

async function collect(
  width: number,
  height: number,
  count: number,
  draw: (ctx: CanvasRenderingContext2D, i: number, w: number, h: number) => Promise<number>,
): Promise<Frames> {
  const scale = Math.min(1, MAX_SIDE / Math.max(width, height));
  const [w, h] = [Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))];
  const canvas = document.createElement("canvas");
  [canvas.width, canvas.height] = [w, h];
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  const frames = Math.min(count, MAX_FRAMES);
  const pixels = new Uint8Array(w * h * 4 * frames);
  const delays = [];
  for (let i = 0; i < frames; i++) {
    ctx.clearRect(0, 0, w, h);
    delays.push(Math.round(await draw(ctx, i, w, h)));
    pixels.set(ctx.getImageData(0, 0, w, h).data, w * h * 4 * i);
  }
  return { width: w, height: h, delays, pixels };
}

function once(target: EventTarget, event: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const done = new AbortController();
    const settle = (error?: Error) => {
      done.abort();
      if (error) reject(error);
      else resolve();
    };
    target.addEventListener(event, () => settle(), { signal: done.signal });
    target.addEventListener("error", () => settle(new Error("the video could not be read")), { signal: done.signal });
  });
}

async function videoFrames(blob: Blob): Promise<Frames> {
  const video = document.createElement("video");
  video.muted = true;
  video.src = URL.createObjectURL(blob);
  try {
    await once(video, "loadeddata");
    // WebKit's first draw from a video comes out blank, so throw one away,
    // away from the first sample: a seek to the same time paints nothing new.
    video.currentTime = Math.min(video.duration, 1);
    await once(video, "seeked");
    document.createElement("canvas").getContext("2d")!.drawImage(video, 0, 0, 1, 1);
    // Streams without a known length report Infinity, which MAX_FRAMES caps.
    const count = Math.max(1, Math.round(video.duration * VIDEO_FPS));
    return await collect(video.videoWidth, video.videoHeight, count, async (ctx, i, w, h) => {
      video.currentTime = (i + 0.5) / VIDEO_FPS;
      await once(video, "seeked");
      ctx.drawImage(video, 0, 0, w, h);
      return 1000 / VIDEO_FPS;
    });
  } finally {
    URL.revokeObjectURL(video.src);
  }
}

async function imageFrames(bytes: Uint8Array<ArrayBuffer>): Promise<Frames> {
  // Whole bytes rather than a stream, so frameCount is final once the track is ready.
  const decoder = new ImageDecoder({ data: bytes, type: "image/avif" });
  try {
    await decoder.tracks.ready;
    const first = (await decoder.decode()).image;
    const [width, height] = [first.displayWidth, first.displayHeight];
    first.close();
    return await collect(width, height, decoder.tracks.selectedTrack!.frameCount, async (ctx, i, w, h) => {
      const { image } = await decoder.decode({ frameIndex: i });
      ctx.drawImage(image, 0, 0, w, h);
      image.close();
      // VideoFrame durations are in microseconds; stills have none.
      return (image.duration ?? 100_000) / 1000;
    });
  } finally {
    decoder.close();
  }
}
