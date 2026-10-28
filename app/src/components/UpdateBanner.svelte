<script lang="ts">
  import { desktop } from '../lib/desktop.svelte.ts';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  const busy = $derived(desktop.phase === 'downloading' || desktop.phase === 'installing');
  const open = $derived(!!desktop.update && (!desktop.dismissed || busy));
  const notes = $derived(
    (desktop.update?.notes ?? '')
      .split('\n')
      .map((l) => l.replace(/^[-*•]\s*/, '').trim())
      .filter(Boolean)
      .slice(0, 6),
  );
</script>

{#if open && desktop.update}
  <div class="scrim" role="presentation"></div>
  <div class="dialog panel" role="dialog" aria-modal="true" aria-labelledby="upd-title">
    <div class="art" aria-hidden="true"><i></i><i></i></div>
    <span class="logo"><Logo variant="glyph" size={34} /></span>
    <p class="eyebrow">Update available</p>
    <h2 id="upd-title">Audiora {desktop.update.version}</h2>
    <p class="muted from">You have {desktop.version}. Your settings and presets stay as they are.</p>

    {#if notes.length}
      <ul class="notes">
        {#each notes as n, i (i)}<li><Icon name="check" size={13} />{n}</li>{/each}
      </ul>
    {/if}

    {#if busy}
      <div class="progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(desktop.progress * 100)}>
        <i style:width="{desktop.phase === 'installing' ? 100 : desktop.progress * 100}%"></i>
      </div>
      <p class="muted state">
        {desktop.phase === 'installing' ? 'Installing… Audiora restarts by itself in a moment.' : `Downloading… ${Math.round(desktop.progress * 100)}%`}
      </p>
    {:else}
      <div class="actions">
        <button class="btn ghost" onclick={() => (desktop.dismissed = true)}>Later</button>
        <button class="btn primary big" onclick={() => void desktop.install()}><Icon name="bolt" size={15} /> Update now</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 300;
    background: rgba(2, 4, 8, 0.6);
    backdrop-filter: blur(6px);
    animation: fade 0.25s var(--ease);
  }

  .dialog {
    position: fixed;
    left: 50%;
    top: 50%;
    z-index: 301;
    width: min(440px, calc(100vw - 40px));
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 30px 30px 26px;
    overflow: hidden;
    box-shadow: var(--shadow-pop), 0 30px 80px -20px rgba(0, 0, 0, 0.9);
    animation: pop 0.32s var(--ease);
  }

  .art {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .art i {
    position: absolute;
    width: 320px;
    height: 320px;
    border-radius: 50%;
    filter: blur(50px);
    opacity: 0.45;
  }

  .art i:first-child {
    left: -120px;
    top: -170px;
    background: radial-gradient(circle, #4d9bff, transparent 65%);
  }

  .art i:last-child {
    right: -140px;
    top: -120px;
    background: radial-gradient(circle, #2ed3e6, transparent 65%);
  }

  .logo {
    position: relative;
    width: 64px;
    height: 64px;
    display: grid;
    place-items: center;
    border-radius: 18px;
    background: #000;
    color: #fff;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.14), 0 14px 34px -10px rgba(0, 0, 0, 0.9);
    margin-bottom: 16px;
  }

  .eyebrow,
  h2,
  .from,
  .notes,
  .actions,
  .progress,
  .state {
    position: relative;
  }

  h2 {
    margin-top: 4px;
    font-size: 26px;
    font-weight: 750;
    letter-spacing: -0.02em;
    color: var(--text);
    text-transform: none;
  }

  .from {
    margin-top: 6px;
    font-size: 13px;
  }

  .notes {
    width: 100%;
    margin: 18px 0 0;
    padding: 14px 16px;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    text-align: left;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--glass-line);
    font-size: 13px;
    color: var(--text-2);
  }

  .notes li {
    display: flex;
    gap: 9px;
  }

  .notes :global(svg) {
    flex: none;
    margin-top: 3px;
    color: var(--ok);
  }

  .actions {
    display: flex;
    gap: 10px;
    width: 100%;
    margin-top: 22px;
  }

  .actions .btn {
    flex: 1;
    height: 42px;
    justify-content: center;
  }

  .big {
    font-size: 14px;
    font-weight: 650;
  }

  .progress {
    width: 100%;
    height: 8px;
    margin-top: 22px;
    border-radius: 4px;
    background: var(--bg-0);
    overflow: hidden;
  }

  .progress i {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, var(--accent-lo), var(--accent));
    box-shadow: 0 0 14px var(--accent-glow);
    transition: width 0.2s;
  }

  .state {
    margin-top: 10px;
    font-size: 12.5px;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translate(-50%, -46%) scale(0.97);
    }
  }
</style>
