<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';

  const SECONDS = 6;
  let phase = $state<'idle' | 'recording' | 'playing'>('idle');
  let seconds = $state(0);
  let length = $state(0);
  let timer = 0;

  async function poll() {
    try {
      const r = await engine.call('mic_test_status');
      phase = r.state as typeof phase;
      seconds = r.seconds as number;
      length = r.length as number;
      if (phase === 'idle') stopPolling();
    } catch {
      stopPolling();
    }
  }

  function stopPolling() {
    clearInterval(timer);
    timer = 0;
  }

  async function act(action: 'record' | 'play' | 'stop') {
    try {
      const r = await engine.call('mic_test', { action, seconds: SECONDS });
      phase = action === 'record' ? 'recording' : action === 'play' ? 'playing' : 'idle';
      length = r.length as number;
      if (!timer) timer = window.setInterval(() => void poll(), 120);
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  $effect(() => stopPolling);

  const progress = $derived(phase === 'recording' ? seconds / SECONDS : phase === 'playing' && length > 0 ? seconds / length : length > 0 ? 1 : 0);
</script>

<section class="check panel" aria-label="Microphone check">
  <span class="ico"><Icon name="record" size={17} /></span>
  <div class="txt">
    <h3>Mic check</h3>
    <p class="muted">
      {#if phase === 'recording'}Speak now… {seconds.toFixed(1)} s
      {:else if phase === 'playing'}Playing back exactly what others hear
      {:else if length > 0}Recorded {length.toFixed(1)} s with all processing — play it back
      {:else}Record {SECONDS} seconds and hear yourself the way others do{/if}
    </p>
    <div class="bar"><i class:rec={phase === 'recording'} style:width="{Math.min(1, progress) * 100}%"></i></div>
  </div>
  {#if phase === 'idle'}
    <button class="btn rec" onclick={() => act('record')}><Icon name="record" size={14} /> Record</button>
    <button class="btn" disabled={length <= 0} onclick={() => act('play')}><Icon name="play" size={14} /> Play</button>
  {:else}
    <button class="btn" onclick={() => act('stop')}><Icon name="stop" size={14} /> Stop</button>
  {/if}
</section>

<style>
  .check {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 18px;
  }

  .ico {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    color: var(--rec);
    background: color-mix(in oklab, var(--rec) 14%, transparent);
  }

  .txt {
    flex: 1;
    min-width: 0;
  }

  h3 {
    font-size: 13.5px;
    font-weight: 650;
  }

  p {
    font-size: 12px;
    margin-top: 2px;
  }

  .bar {
    margin-top: 8px;
    height: 4px;
    border-radius: 2px;
    background: var(--bg-0);
    overflow: hidden;
  }

  .bar i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.12s linear;
  }

  .bar i.rec {
    background: var(--rec);
  }

  .btn.rec {
    color: var(--rec);
  }
</style>
