<script lang="ts">
  interface Props {
    /** Output level in dB for an input level in dB. */
    fn: (inDb: number) => number;
    thresholdDb: number;
    /** Optional live input level marker in dB. */
    liveDb?: number | null;
    width?: number;
    height?: number;
    color?: string;
    active?: boolean;
    label: string;
  }
  let { fn, thresholdDb, liveDb = null, width = 190, height = 150, color = 'var(--accent)', active = true, label }: Props = $props();

  const MIN = -60;
  const MAX = 0;

  let canvas: HTMLCanvasElement;

  $effect(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    const ctx = canvas.getContext('2d')!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);

    const tint = getComputedStyle(canvas).getPropertyValue('--g').trim() || '#4d9bff';
    const pad = 6;
    const px = (db: number) => pad + ((db - MIN) / (MAX - MIN)) * (width - pad * 2);
    const py = (db: number) => height - pad - ((Math.min(Math.max(db, MIN), MAX) - MIN) / (MAX - MIN)) * (height - pad * 2);

    ctx.lineWidth = 1;
    ctx.strokeStyle = 'rgba(255,255,255,0.05)';
    for (let db = MIN; db <= MAX; db += 12) {
      ctx.beginPath();
      ctx.moveTo(Math.round(px(db)) + 0.5, pad);
      ctx.lineTo(Math.round(px(db)) + 0.5, height - pad);
      ctx.moveTo(pad, Math.round(py(db)) + 0.5);
      ctx.lineTo(width - pad, Math.round(py(db)) + 0.5);
      ctx.stroke();
    }

    // unity line
    ctx.strokeStyle = 'rgba(255,255,255,0.16)';
    ctx.setLineDash([3, 4]);
    ctx.beginPath();
    ctx.moveTo(px(MIN), py(MIN));
    ctx.lineTo(px(MAX), py(MAX));
    ctx.stroke();
    ctx.setLineDash([]);

    // threshold
    ctx.strokeStyle = 'rgba(255,255,255,0.3)';
    ctx.beginPath();
    ctx.moveTo(Math.round(px(thresholdDb)) + 0.5, pad);
    ctx.lineTo(Math.round(px(thresholdDb)) + 0.5, height - pad);
    ctx.stroke();

    // curve
    const path = new Path2D();
    for (let db = MIN; db <= MAX; db += 0.5) {
      const X = px(db);
      const Y = py(fn(db));
      db === MIN ? path.moveTo(X, Y) : path.lineTo(X, Y);
    }
    ctx.globalAlpha = active ? 1 : 0.35;
    ctx.shadowColor = tint;
    ctx.shadowBlur = active ? 10 : 0;
    ctx.strokeStyle = tint;
    ctx.lineWidth = 2.4;
    ctx.lineJoin = 'round';
    ctx.stroke(path);
    ctx.shadowBlur = 0;

    if (liveDb !== null && active) {
      ctx.beginPath();
      ctx.arc(px(liveDb), py(fn(liveDb)), 4.5, 0, Math.PI * 2);
      ctx.fillStyle = '#fff';
      ctx.shadowColor = '#fff';
      ctx.shadowBlur = 8;
      ctx.fill();
      ctx.shadowBlur = 0;
    }
    ctx.globalAlpha = 1;
  });
</script>

<canvas
  bind:this={canvas}
  class="graph"
  style:width="{width}px"
  style:height="{height}px"
  style:--g={color}
  aria-label={label}
></canvas>

<style>
  .graph {
    display: block;
    border-radius: var(--r-sm);
    background: var(--bg-0);
    border: 1px solid var(--line);
    box-shadow: inset 0 2px 6px rgba(0, 0, 0, 0.55);
  }
</style>
