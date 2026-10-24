<script lang="ts">
  interface Props {
    checked: boolean;
    label: string;
    disabled?: boolean;
    onchange: (v: boolean) => void;
  }
  let { checked, label, disabled = false, onchange }: Props = $props();
</script>

<button
  class="switch"
  class:on={checked}
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    position: relative;
    flex: none;
    width: 38px;
    height: 22px;
    border-radius: 11px;
    border: 1px solid var(--line-2);
    background: var(--bg-0);
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.7);
    padding: 0;
    transition: background 0.18s var(--ease), border-color 0.18s;
  }

  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: linear-gradient(#f2f5fa, #aeb8c7);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.7);
    transition: transform 0.18s var(--ease);
  }

  .on {
    background: linear-gradient(180deg, var(--accent-hi), var(--accent));
    border-color: transparent;
    box-shadow: 0 0 14px -2px var(--accent-glow);
  }

  .on .knob {
    transform: translateX(16px);
  }

  .switch:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
