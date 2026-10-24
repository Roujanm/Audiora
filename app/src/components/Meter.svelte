<script lang="ts">
  interface Props {
    /** Displayed levels in dBFS, one per channel. */
    levels: number[];
    holds?: number[];
    floor?: number;
  }
  let { levels, holds = [], floor = -60 }: Props = $props();

  const pos = (db: number) => Math.min(Math.max((db - floor) / -floor, 0), 1);
  const ticks = [0, -6, -12, -24, -48];
</script>

<div class="meter" role="img" aria-label="Level meter">
  {#each levels as db, i (i)}
    <div class="bar">
      <div class="mask" style:height="{(1 - pos(db)) * 100}%"></div>
      {#if holds[i] !== undefined && holds[i] > floor}
        <div class="hold" class:clip={holds[i] > -0.5} style:bottom="{pos(holds[i]) * 100}%"></div>
      {/if}
    </div>
  {/each}
  <div class="scale">
    {#each ticks as t (t)}
      <span style:bottom="{pos(t) * 100}%">{Math.abs(t)}</span>
    {/each}
  </div>
</div>

<style>
  .meter {
    position: relative;
    display: flex;
    gap: 3px;
    height: 100%;
    min-height: 120px;
    padding-right: 18px;
  }

  .bar {
    position: relative;
    width: 9px;
    border-radius: 2px;
    overflow: hidden;
    background: linear-gradient(
      to top,
      var(--ok) 0%,
      var(--ok) 62%,
      #c9e060 78%,
      var(--warn) 88%,
      var(--bad) 100%
    );
  }

  .mask {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    background: #0a0e13;
    opacity: 0.93;
  }

  .hold {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: #fff;
    transform: translateY(1px);
  }

  .hold.clip {
    background: var(--bad);
    box-shadow: 0 0 6px var(--bad);
  }

  .scale {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 16px;
    font-size: 9px;
    color: var(--faint);
    pointer-events: none;
  }

  .scale span {
    position: absolute;
    right: 0;
    transform: translateY(50%);
  }
</style>
