// Minimal PNG reader for screenshot baselines: 8-bit RGB/RGBA, non-interlaced,
// which is what the bridge captures. Kept dependency-free so the harness runs
// from a plain Node install.
import { inflateSync } from 'node:zlib';

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

const paeth = (a, b, c) => {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  return pb <= pc ? b : c;
};

/// Decode one PNG into `{ width, height, channels, pixels }`.
export function decodePng(buffer) {
  if (!buffer.subarray(0, 8).equals(SIGNATURE)) throw new Error('not a PNG');
  let offset = 8;
  let header = null;
  const chunks = [];
  while (offset + 8 <= buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const type = buffer.toString('ascii', offset + 4, offset + 8);
    const data = buffer.subarray(offset + 8, offset + 8 + length);
    offset += 12 + length;
    if (type === 'IHDR') {
      header = {
        width: data.readUInt32BE(0),
        height: data.readUInt32BE(4),
        bitDepth: data[8],
        colorType: data[9],
        interlace: data[12],
      };
    } else if (type === 'IDAT') {
      chunks.push(data);
    } else if (type === 'IEND') {
      break;
    }
  }
  if (!header) throw new Error('PNG has no IHDR chunk');
  const { width, height, bitDepth, colorType, interlace } = header;
  const channels = colorType === 6 ? 4 : colorType === 2 ? 3 : 0;
  if (bitDepth !== 8 || interlace !== 0 || channels === 0) {
    throw new Error(`unsupported PNG (depth ${bitDepth}, color ${colorType}, interlace ${interlace})`);
  }
  const stride = width * channels;
  const raw = inflateSync(Buffer.concat(chunks));
  if (raw.length !== (stride + 1) * height) throw new Error('PNG data length does not match its header');
  const pixels = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y += 1) {
    const filter = raw[y * (stride + 1)];
    const row = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const out = pixels.subarray(y * stride, (y + 1) * stride);
    const prev = y > 0 ? pixels.subarray((y - 1) * stride, y * stride) : null;
    for (let x = 0; x < stride; x += 1) {
      const left = x >= channels ? out[x - channels] : 0;
      const up = prev ? prev[x] : 0;
      const upLeft = prev && x >= channels ? prev[x - channels] : 0;
      const value = row[x];
      switch (filter) {
        case 0: out[x] = value; break;
        case 1: out[x] = value + left; break;
        case 2: out[x] = value + up; break;
        case 3: out[x] = value + ((left + up) >> 1); break;
        case 4: out[x] = value + paeth(left, up, upLeft); break;
        default: throw new Error(`unknown PNG filter ${filter}`);
      }
    }
  }
  return { width, height, channels, pixels };
}

/// Compare two captures over their overlapping area. `tolerance` is the largest
/// per-channel delta treated as equal; a window can shift by a few pixels between
/// runs, so a small size delta and a mismatch budget are allowed. A real layout
/// change moves far more pixels than anti-aliasing jitter ever does.
export function comparePng(leftBuffer, rightBuffer, {
  tolerance = 2,
  mismatchRatio = 0.002,
  maxSizeDelta = 8,
} = {}) {
  const left = decodePng(leftBuffer);
  const right = decodePng(rightBuffer);
  if (left.channels !== right.channels) {
    return {
      same: false,
      reason: `channel layout ${left.channels} -> ${right.channels}`,
      mismatched: 0,
      total: 0,
      ratio: 1,
      maxDelta: 255,
    };
  }
  const width = Math.min(left.width, right.width);
  const height = Math.min(left.height, right.height);
  const sizeDelta = Math.max(
    Math.abs(left.width - right.width),
    Math.abs(left.height - right.height),
  );
  const total = width * height;
  let mismatched = 0;
  let maxDelta = 0;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      let delta = 0;
      for (let channel = 0; channel < left.channels; channel += 1) {
        const leftIndex = (y * left.width + x) * left.channels + channel;
        const rightIndex = (y * right.width + x) * right.channels + channel;
        delta = Math.max(delta, Math.abs(left.pixels[leftIndex] - right.pixels[rightIndex]));
      }
      if (delta > maxDelta) maxDelta = delta;
      if (delta > tolerance) mismatched += 1;
    }
  }
  const ratio = total === 0 ? 0 : mismatched / total;
  const same = sizeDelta <= maxSizeDelta && ratio <= mismatchRatio;
  return {
    same,
    reason: same
      ? null
      : `${mismatched}/${total} pixels differ (${(ratio * 100).toFixed(2)}%), size delta ${sizeDelta}`,
    mismatched,
    total,
    ratio,
    maxDelta,
    sizeDelta,
  };
}
