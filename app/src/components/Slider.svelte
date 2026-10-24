<script lang="ts">
  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    unit?: string;
    digits?: number;
    /** Value restored on double-click. */
    reset?: number;
    disabled?: boolean;
    onchange: (v: number) => void;
  }
  let { label, value, min, max, step = 1, unit = '', digits = 1, reset, disabled = false, onchange }: Props = $props();
  const id = $props.id();
</script>

<div class="slider" class:disabled>
  <label for={id}>{label}</label>
  <input
    {id}
    type="range"
    {min}
    {max}
    {step}
    {value}
    {disabled}
    oninput={(e) => onchange(+e.currentTarget.value)}
    ondblclick={() => reset !== undefined && onchange(reset)}
  />
  <output class="num">{value.toFixed(digits)}{unit}</output>
</div>

<style>
  .slider {
    display: grid;
    grid-template-columns: 78px 1fr 62px;
    align-items: center;
    gap: 8px;
    min-height: 26px;
  }

  label {
    color: var(--muted);
    font-size: 12px;
  }

  input {
    width: 100%;
    margin: 0;
  }

  output {
    text-align: right;
    font-size: 12px;
  }

  .disabled {
    opacity: 0.45;
  }
</style>
